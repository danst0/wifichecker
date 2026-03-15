use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, DropDown, FileDialog,
    Orientation, Separator, StringList, ToggleButton,
};
use gtk4::glib;
use libadwaita::prelude::*;
use libadwaita::{ApplicationWindow, HeaderBar, MessageDialog, Toast, ToastOverlay};
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::{AppSettings, Floor, Measurement, Project};
use crate::persistence::{JsonStore, SettingsStore};
use crate::persistence::json_store::{drawings_dir, ensure_config_dirs};
use crate::services::{IperfClient, SmbTester, WifiInfo, WifiScanner};
use crate::widgets::{FloorPlanView, LegendBar, MeasurementPanel, SettingsDialog};
use crate::widgets::floor_plan_view::DrawMode;

struct MeasureResult {
    rx: f64,
    ry: f64,
    wifi: Option<WifiInfo>,
    iperf_mbps: Option<f64>,
    iperf_error: Option<String>,
    smb_mbps: Option<f64>,
    smb_error: Option<String>,
}

pub struct Window {
    pub window: ApplicationWindow,
}

impl Window {
    pub fn new(app: &libadwaita::Application) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("WiFi Checker")
            .default_width(1200)
            .default_height(780)
            .build();

        let _ = ensure_config_dirs();
        let settings = Rc::new(RefCell::new(SettingsStore::load()));

        let project = JsonStore::load(&JsonStore::default_path())
            .unwrap_or_else(|_| Project::new("My Project"));

        let state = Rc::new(RefCell::new(AppState {
            project,
            current_floor: 0,
        }));

        let content = build_ui(&window, state.clone(), settings.clone());
        window.set_content(Some(&content));

        // Ctrl+Q / Ctrl+W → close window
        let key_ctrl = gtk4::EventControllerKey::new();
        {
            let win = window.clone();
            key_ctrl.connect_key_pressed(move |_, key, _, mods| {
                if mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK)
                    && (key == gtk4::gdk::Key::q || key == gtk4::gdk::Key::w)
                {
                    win.close();
                    return gtk4::glib::Propagation::Stop;
                }
                gtk4::glib::Propagation::Proceed
            });
        }
        window.add_controller(key_ctrl);

        Self { window }
    }
}

struct AppState {
    project: Project,
    current_floor: usize,
}

/// Save the current floor's canvas PNG and persist the whole project to disk.
fn auto_save(fp: &FloorPlanView, state: &Rc<RefCell<AppState>>) {
    let idx = state.borrow().current_floor;
    let canvas_path = drawings_dir().join(format!("floor_{idx}.png"));
    if fp.save_canvas(&canvas_path).is_ok() {
        let mut s = state.borrow_mut();
        if let Some(floor) = s.project.floors.get_mut(idx) {
            floor.drawing_path = Some(canvas_path.to_string_lossy().to_string());
            floor.origin = fp.get_origin();
        }
    }
    let project = state.borrow().project.clone();
    let _ = JsonStore::save(&project, &JsonStore::default_path());
}

fn build_ui(
    window: &ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    settings: Rc<RefCell<AppSettings>>,
) -> ToastOverlay {
    let overlay = ToastOverlay::new();
    let main_box = GtkBox::new(Orientation::Vertical, 0);

    // ── Header bar ────────────────────────────────────────────────────────────
    let header = HeaderBar::new();

    let floor_model = StringList::new(&[]);
    let floor_dropdown = DropDown::new(Some(floor_model.clone()), gtk4::Expression::NONE);
    floor_dropdown.set_tooltip_text(Some("Select floor"));
    header.pack_start(&floor_dropdown);

    let add_floor_btn = Button::from_icon_name("list-add-symbolic");
    add_floor_btn.set_tooltip_text(Some("Add floor"));
    header.pack_start(&add_floor_btn);

    let edit_floor_btn = Button::from_icon_name("document-edit-symbolic");
    edit_floor_btn.set_tooltip_text(Some("Rename or delete current floor"));
    header.pack_start(&edit_floor_btn);

    let heatmap_toggle = ToggleButton::new();
    heatmap_toggle.set_icon_name("view-grid-symbolic");
    heatmap_toggle.set_tooltip_text(Some("Toggle heatmap"));
    heatmap_toggle.set_active(true);
    header.pack_end(&heatmap_toggle);

    let settings_btn = Button::from_icon_name("preferences-system-symbolic");
    settings_btn.set_tooltip_text(Some("Settings (iperf / Samba)"));
    header.pack_end(&settings_btn);

    main_box.append(&header);

    // ── Drawing toolbar ───────────────────────────────────────────────────────
    let draw_bar = GtkBox::new(Orientation::Horizontal, 4);
    draw_bar.set_margin_start(6);
    draw_bar.set_margin_end(6);
    draw_bar.set_margin_top(4);
    draw_bar.set_margin_bottom(4);

    let mode_measure = ToggleButton::builder()
        .icon_name("find-location-symbolic")
        .tooltip_text("Measure mode (click to record WiFi point)")
        .active(true)
        .build();
    let mode_draw = ToggleButton::builder()
        .icon_name("edit-symbolic")
        .tooltip_text("Draw mode (freehand floor plan)")
        .group(&mode_measure)
        .build();
    let mode_calib = ToggleButton::builder()
        .icon_name("zoom-fit-best-symbolic")
        .tooltip_text("Calibrate scale (click two points)")
        .group(&mode_measure)
        .build();
    let mode_origin = ToggleButton::builder()
        .icon_name("mark-location-symbolic")
        .tooltip_text("Set origin (0, 0) — click to place")
        .group(&mode_measure)
        .build();

    let clear_canvas_btn = Button::builder()
        .icon_name("edit-clear-symbolic")
        .tooltip_text("Clear drawing")
        .build();

    let grid_toggle = ToggleButton::builder()
        .icon_name("view-grid-symbolic")
        .tooltip_text("Toggle grid")
        .active(settings.borrow().show_grid)
        .build();

    // Grid spacing selector
    let spacing_model = StringList::new(&["0.5 m", "1 m", "2 m", "5 m"]);
    let spacing_values = [0.5f64, 1.0, 2.0, 5.0];
    let cur_spacing = settings.borrow().grid_spacing_m;
    let spacing_idx = spacing_values.iter().position(|&v| (v - cur_spacing).abs() < 0.01).unwrap_or(1) as u32;
    let grid_spacing_dd = DropDown::new(Some(spacing_model), gtk4::Expression::NONE);
    grid_spacing_dd.set_selected(spacing_idx);
    grid_spacing_dd.set_tooltip_text(Some("Grid spacing"));

    let import_btn = Button::builder()
        .icon_name("insert-image-symbolic")
        .tooltip_text("Import floor plan image")
        .build();

    let zoom_in_btn = Button::builder()
        .icon_name("zoom-in-symbolic")
        .tooltip_text("Zoom in")
        .build();
    let zoom_out_btn = Button::builder()
        .icon_name("zoom-out-symbolic")
        .tooltip_text("Zoom out")
        .build();
    let zoom_reset_btn = Button::builder()
        .icon_name("zoom-original-symbolic")
        .tooltip_text("Reset zoom")
        .build();

    draw_bar.append(&mode_measure);
    draw_bar.append(&mode_draw);
    draw_bar.append(&mode_calib);
    draw_bar.append(&mode_origin);
    draw_bar.append(&Separator::new(Orientation::Vertical));
    draw_bar.append(&clear_canvas_btn);
    draw_bar.append(&Separator::new(Orientation::Vertical));
    draw_bar.append(&grid_toggle);
    draw_bar.append(&grid_spacing_dd);
    draw_bar.append(&Separator::new(Orientation::Vertical));
    draw_bar.append(&import_btn);
    draw_bar.append(&Separator::new(Orientation::Vertical));
    draw_bar.append(&zoom_in_btn);
    draw_bar.append(&zoom_out_btn);
    draw_bar.append(&zoom_reset_btn);

    main_box.append(&draw_bar);

    // ── Body ──────────────────────────────────────────────────────────────────
    let body = GtkBox::new(Orientation::Horizontal, 0);
    body.set_vexpand(true);

    let floor_plan = FloorPlanView::new();
    floor_plan.set_show_grid(settings.borrow().show_grid);
    floor_plan.set_grid_spacing(settings.borrow().grid_spacing_m);
    floor_plan.set_measurement_grid_spacing(settings.borrow().measurement_grid_spacing_m);
    floor_plan.set_snap_to_grid(settings.borrow().snap_to_grid);

    let legend = LegendBar::new();
    let fp_col = GtkBox::new(Orientation::Vertical, 0);
    fp_col.set_hexpand(true);
    fp_col.set_vexpand(true);
    fp_col.append(&floor_plan.widget);
    fp_col.append(&legend.widget);
    body.append(&fp_col);

    let sidebar = GtkBox::new(Orientation::Vertical, 6);
    sidebar.set_width_request(290);

    let panel = MeasurementPanel::new();

    sidebar.append(&panel.widget);
    body.append(&sidebar);
    main_box.append(&body);
    overlay.set_child(Some(&main_box));

    // ── Wire up callbacks ──────────────────────────────────────────────────────

    // Heatmap toggle
    {
        let fp = floor_plan.clone();
        heatmap_toggle.connect_toggled(move |btn| fp.set_show_heatmap(btn.is_active()));
    }

    // Draw mode buttons
    {
        let fp = floor_plan.clone();
        mode_measure.connect_toggled(move |btn| {
            if btn.is_active() { fp.set_draw_mode(DrawMode::Measure); }
        });
    }
    {
        let fp = floor_plan.clone();
        mode_draw.connect_toggled(move |btn| {
            if btn.is_active() { fp.set_draw_mode(DrawMode::Draw); }
        });
    }
    {
        let fp = floor_plan.clone();
        mode_calib.connect_toggled(move |btn| {
            if btn.is_active() { fp.set_draw_mode(DrawMode::Calibrate); }
        });
    }
    {
        let fp = floor_plan.clone();
        mode_origin.connect_toggled(move |btn| {
            if btn.is_active() { fp.set_draw_mode(DrawMode::SetOrigin); }
        });
    }

    // Clear canvas
    {
        let fp = floor_plan.clone();
        let state = state.clone();
        let overlay_ref = overlay.clone();
        clear_canvas_btn.connect_clicked(move |_| {
            fp.clear_canvas();
            {
                let mut s = state.borrow_mut();
                let idx = s.current_floor;
                if let Some(floor) = s.project.floors.get_mut(idx) {
                    floor.drawing_path = None;
                }
            }
            auto_save(&fp, &state);
            overlay_ref.add_toast(Toast::new("Drawing cleared"));
        });
    }

    // Grid toggle
    {
        let fp = floor_plan.clone();
        let settings = settings.clone();
        grid_toggle.connect_toggled(move |btn| {
            fp.set_show_grid(btn.is_active());
            settings.borrow_mut().show_grid = btn.is_active();
            let _ = SettingsStore::save(&settings.borrow());
        });
    }

    // Grid spacing dropdown
    {
        let fp = floor_plan.clone();
        let settings = settings.clone();
        grid_spacing_dd.connect_selected_notify(move |dd| {
            let m = spacing_values[dd.selected() as usize];
            fp.set_grid_spacing(m);
            settings.borrow_mut().grid_spacing_m = m;
            let _ = SettingsStore::save(&settings.borrow());
        });
    }

    // Measure button — WiFi scan in background thread
    // ── on_measure_click: click on map = immediate full measurement ───────────
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        let overlay_ref = overlay.clone();
        let settings = settings.clone();
        let legend = legend.clone();

        floor_plan.set_on_measure_click(move |rx, ry| {
            let (iperf_enabled, iperf_server, iperf_port, iperf_dur, iperf_streams,
                 smb_enabled, smb_server, smb_share, smb_user, smb_pass,
                 unit) = {
                let s = settings.borrow();
                (
                    s.iperf_enabled, s.iperf_server.clone(), s.iperf_port, s.iperf_duration_secs,
                    s.iperf_parallel_streams,
                    s.smb_enabled, s.smb_server.clone(), s.smb_share.clone(),
                    s.smb_username.clone(), s.smb_password.clone(),
                    s.throughput_unit,
                )
            };

            let panel2 = panel.clone();
            let status_msg = match (iperf_enabled && !iperf_server.is_empty(),
                                    smb_enabled   && !smb_server.is_empty()) {
                (true,  true)  => "Scanning + iperf3 + Samba…",
                (true,  false) => "Scanning + iperf3…",
                (false, true)  => "Scanning + Samba…",
                (false, false) => "Scanning WiFi…",
            };
            panel2.set_measuring(true, status_msg);

            let (tx, recv) = async_channel::bounded::<MeasureResult>(1);
            std::thread::spawn(move || {
                let wifi = WifiScanner::scan().ok().flatten();

                let (iperf_mbps, iperf_error) = if iperf_enabled && !iperf_server.is_empty() {
                    match IperfClient::new(&iperf_server, iperf_port, iperf_dur, iperf_streams).run_test() {
                        Ok(mbps) => (Some(mbps), None),
                        Err(e)   => (None, Some(e.to_string())),
                    }
                } else {
                    (None, None)
                };

                let (smb_mbps, smb_error) = if smb_enabled && !smb_server.is_empty() {
                    let mut tester = SmbTester::new(&smb_server, &smb_share);
                    tester.username = if smb_user.is_empty() { None } else { Some(smb_user) };
                    tester.password = if smb_pass.is_empty() { None } else { Some(smb_pass) };
                    match tester.run_test() {
                        Ok(mbps) => (Some(mbps), None),
                        Err(e)   => (None, Some(e.to_string())),
                    }
                } else {
                    (None, None)
                };

                tx.send_blocking(MeasureResult { rx, ry, wifi, iperf_mbps, iperf_error, smb_mbps, smb_error }).ok();
            });

            let state2 = state.clone();
            let fp2 = fp.clone();
            let panel3 = panel.clone();
            let overlay2 = overlay_ref.clone();
            let legend2 = legend.clone();

            glib::spawn_future_local(async move {
                let Ok(result) = recv.recv().await else { return; };
                panel3.set_measuring(false, "");

                let Some(info) = result.wifi else {
                    overlay2.add_toast(Toast::new("No active WiFi connection"));
                    // Still surface speed-test errors even without WiFi
                    if let Some(ref e) = result.iperf_error {
                        overlay2.add_toast(Toast::new(&format!("iperf3 error: {e}")));
                    }
                    if let Some(ref e) = result.smb_error {
                        overlay2.add_toast(Toast::new(&format!("Samba error: {e}")));
                    }
                    return;
                };

                let mut m = Measurement::new(
                    result.rx, result.ry,
                    info.ssid.clone(), info.bssid.clone(),
                    info.frequency_mhz, info.channel, info.signal_dbm,
                );
                m.iperf_mbps = result.iperf_mbps;
                m.smb_mbps = result.smb_mbps;

                // Surface speed-test errors as toasts
                if let Some(ref e) = result.iperf_error {
                    overlay2.add_toast(Toast::new(&format!("iperf3 error: {e}")));
                }
                if let Some(ref e) = result.smb_error {
                    overlay2.add_toast(Toast::new(&format!("Samba error: {e}")));
                }

                let (measurements, panel_measurements) = {
                    let mut s = state2.borrow_mut();
                    let idx = s.current_floor;
                    if let Some(floor) = s.project.floors.get_mut(idx) {
                        floor.add_measurement(m);
                        let measurements = floor.measurements.clone();
                        (measurements.clone(), measurements)
                    } else {
                        return;
                    }
                };

                fp2.set_measurements(measurements.clone());
                legend2.set_measurements(&measurements);
                panel3.set_measurements(panel_measurements);
                panel3.set_throughput_unit(unit);
                panel3.update_current_wifi(
                    &info.ssid, &info.bssid,
                    info.signal_dbm, info.frequency_mhz, info.channel,
                    result.iperf_mbps, result.smb_mbps, unit,
                );
                auto_save(&fp2, &state2);
                let mut toast_msg = format!("{} dBm | {}", info.signal_dbm, info.ssid);
                if let Some(mbps) = result.iperf_mbps {
                    toast_msg.push_str(&format!(" | ⚡{}", unit.format_short(mbps)));
                }
                overlay2.add_toast(Toast::new(&toast_msg));
            });
        });
    }

    // Calibration callback → show distance dialog
    {
        let fp = floor_plan.clone();
        let state = state.clone();
        let window_ref = window.clone();
        floor_plan.set_on_calibration_complete(move |ax, ay, bx, by| {
            let dialog = MessageDialog::builder()
                .heading("Set real distance")
                .body("Enter the real-world distance between points A and B (in meters):")
                .default_response("ok")
                .close_response("cancel")
                .transient_for(&window_ref)
                .modal(true)
                .build();
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("ok", "Set Scale");
            dialog.set_response_appearance("ok", libadwaita::ResponseAppearance::Suggested);

            let entry = gtk4::Entry::builder()
                .placeholder_text("e.g. 3.5")
                .input_purpose(gtk4::InputPurpose::Number)
                .build();
            dialog.set_extra_child(Some(&entry));

            let fp2 = fp.clone();
            let state2 = state.clone();
            let entry2 = entry.clone();
            dialog.choose(gtk4::gio::Cancellable::NONE, move |response| {
                if response.as_str() != "ok" { return; }
                let text = entry2.text();
                let Ok(real_m) = text.trim().parse::<f64>() else { return; };
                if real_m <= 0.0 { return; }

                let w = fp2.widget.width() as f64;
                let h = fp2.widget.height() as f64;
                let dx = (bx - ax) * w;
                let dy = (by - ay) * h;
                let px_dist = (dx * dx + dy * dy).sqrt();
                let scale = px_dist / real_m;

                fp2.set_scale(scale, (ax, ay), (bx, by));

                {
                    let mut s = state2.borrow_mut();
                    let idx = s.current_floor;
                    if let Some(floor) = s.project.floors.get_mut(idx) {
                        floor.scale_px_per_m = Some(scale);
                        floor.calib_point_a = Some((ax, ay));
                        floor.calib_point_b = Some((bx, by));
                    }
                }
                auto_save(&fp2, &state2);
            });
        });
    }

    // Persist drawing strokes when the user finishes a draw gesture
    {
        let fp = floor_plan.clone();
        let state = state.clone();
        floor_plan.set_on_draw_complete(move || {
            auto_save(&fp, &state);
        });
    }

    // Delete measurement
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        let panel2 = panel.clone();
        let legend = legend.clone();
        panel.set_on_delete(move |id| {
            let measurements = {
                let mut s = state.borrow_mut();
                let idx = s.current_floor;
                if let Some(floor) = s.project.floors.get_mut(idx) {
                    floor.remove_measurement(&id);
                    floor.measurements.clone()
                } else {
                    return;
                }
            };
            fp.set_measurements(measurements.clone());
            legend.set_measurements(&measurements);
            panel2.set_measurements(measurements);
            auto_save(&fp, &state);
        });
    }

    // Delete all measurements (triggered from panel's trash button)
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        let overlay_ref = overlay.clone();
        let window_ref = window.clone();
        let panel_ref = panel.clone();
        let legend = legend.clone();
        panel_ref.set_on_delete_all(move || {
            let n_floors = state.borrow().project.floors.len();
            let dialog = MessageDialog::builder()
                .heading("Delete All Measurements")
                .body(if n_floors > 1 {
                    "Delete measurements on the current floor, or on all floors?"
                } else {
                    "Delete all measurements on this floor?"
                })
                .default_response("cancel")
                .close_response("cancel")
                .transient_for(&window_ref)
                .modal(true)
                .build();
            dialog.add_response("cancel", "Cancel");
            if n_floors > 1 {
                dialog.add_response("all", "All Floors");
                dialog.set_response_appearance("all", libadwaita::ResponseAppearance::Destructive);
            }
            dialog.add_response("current", "This Floor");
            dialog.set_response_appearance("current", libadwaita::ResponseAppearance::Destructive);

            let state2 = state.clone();
            let fp2 = fp.clone();
            let panel2 = panel.clone();
            let overlay2 = overlay_ref.clone();
            let legend2 = legend.clone();
            dialog.choose(gtk4::gio::Cancellable::NONE, move |response| {
                match response.as_str() {
                    "current" => {
                        let idx = state2.borrow().current_floor;
                        {
                            let mut s = state2.borrow_mut();
                            if let Some(floor) = s.project.floors.get_mut(idx) {
                                floor.measurements.clear();
                            }
                        }
                        fp2.set_measurements(vec![]);
                        legend2.set_measurements(&[]);
                        panel2.set_measurements(vec![]);
                        auto_save(&fp2, &state2);
                        overlay2.add_toast(Toast::new("Measurements deleted"));
                    }
                    "all" => {
                        {
                            let mut s = state2.borrow_mut();
                            for floor in s.project.floors.iter_mut() {
                                floor.measurements.clear();
                            }
                        }
                        fp2.set_measurements(vec![]);
                        legend2.set_measurements(&[]);
                        panel2.set_measurements(vec![]);
                        auto_save(&fp2, &state2);
                        overlay2.add_toast(Toast::new("All measurements deleted"));
                    }
                    _ => {}
                }
            });
        });
    }

    // Add floor
    {
        let state = state.clone();
        let floor_model = floor_model.clone();
        let floor_dropdown = floor_dropdown.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        let overlay_ref = overlay.clone();
        add_floor_btn.connect_clicked(move |_| {
            // Save current floor before switching
            auto_save(&fp, &state);

            let (name, new_idx) = {
                let mut s = state.borrow_mut();
                let name = format!("Floor {}", s.project.floors.len() + 1);
                s.project.add_floor(Floor::new(&name));
                s.current_floor = s.project.floors.len() - 1;
                (name, s.current_floor)
            };

            floor_model.append(&name);
            if floor_dropdown.selected() as usize == new_idx {
                fp.set_measurements(vec![]);
                fp.set_image("");
                panel.set_measurements(vec![]);
            } else {
                floor_dropdown.set_selected(new_idx as u32);
            }
            auto_save(&fp, &state);
            overlay_ref.add_toast(Toast::new(&format!("Added: {name}")));
        });
    }

    // Shared flag: suppress floor_dropdown's selected_notify during programmatic changes
    let suppress_floor_change = Rc::new(std::cell::Cell::new(false));

    // Edit floor (rename / delete)
    {
        let state = state.clone();
        let floor_model = floor_model.clone();
        let floor_dropdown = floor_dropdown.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        let overlay_ref = overlay.clone();
        let window_ref = window.clone();
        let suppress = suppress_floor_change.clone();
        let legend = legend.clone();
        edit_floor_btn.connect_clicked(move |_| {
            let (current_name, n_floors, current_idx) = {
                let s = state.borrow();
                (
                    s.project.floors[s.current_floor].name.clone(),
                    s.project.floors.len(),
                    s.current_floor,
                )
            };

            let dialog = MessageDialog::builder()
                .heading("Edit Floor")
                .body(if n_floors > 1 { "Rename this floor or delete it." }
                      else            { "Rename this floor." })
                .default_response("rename")
                .close_response("cancel")
                .transient_for(&window_ref)
                .modal(true)
                .build();
            dialog.add_response("cancel", "Cancel");
            if n_floors > 1 {
                dialog.add_response("delete", "Delete Floor");
                dialog.set_response_appearance("delete", libadwaita::ResponseAppearance::Destructive);
            }
            dialog.add_response("rename", "Rename");
            dialog.set_response_appearance("rename", libadwaita::ResponseAppearance::Suggested);

            let entry = gtk4::Entry::builder()
                .text(&current_name)
                .activates_default(true)
                .build();
            dialog.set_extra_child(Some(&entry));

            let state2        = state.clone();
            let floor_model2  = floor_model.clone();
            let floor_dd2     = floor_dropdown.clone();
            let fp2           = fp.clone();
            let panel2        = panel.clone();
            let overlay2      = overlay_ref.clone();
            let entry2        = entry.clone();
            let suppress2     = suppress.clone();
            let legend2       = legend.clone();

            dialog.choose(gtk4::gio::Cancellable::NONE, move |response| {
                match response.as_str() {
                    "rename" => {
                        let new_name = entry2.text().trim().to_string();
                        if new_name.is_empty() { return; }
                        state2.borrow_mut().project.floors[current_idx].name = new_name.clone();
                        floor_model2.splice(current_idx as u32, 1, &[new_name.as_str()]);
                        auto_save(&fp2, &state2);
                        overlay2.add_toast(Toast::new(&format!("Renamed to \"{new_name}\"")));
                    }
                    "delete" => {
                        // Which floor to show after deletion
                        let new_idx = if current_idx + 1 < n_floors { current_idx } else { current_idx - 1 };

                        // Remove drawing file from disk
                        if let Some(path) = state2.borrow().project.floors[current_idx].drawing_path.clone() {
                            let _ = std::fs::remove_file(&path);
                        }

                        // Update model state
                        {
                            let mut s = state2.borrow_mut();
                            s.project.floors.remove(current_idx);
                            s.current_floor = new_idx;
                        }

                        // Update the dropdown without triggering selected_notify side-effects
                        suppress2.set(true);
                        floor_model2.remove(current_idx as u32);
                        floor_dd2.set_selected(new_idx as u32);
                        suppress2.set(false);

                        // Load the replacement floor into the UI
                        let (measurements, image_path, drawing_path, scale, calib_a, calib_b, pdf_page, origin) = {
                            let s = state2.borrow();
                            let floor = &s.project.floors[new_idx];
                            (
                                floor.measurements.clone(),
                                floor.image_path.clone(),
                                floor.drawing_path.clone(),
                                floor.scale_px_per_m,
                                floor.calib_point_a,
                                floor.calib_point_b,
                                floor.pdf_page,
                                floor.origin,
                            )
                        };

                        fp2.set_image("");
                        fp2.clear_canvas();
                        fp2.clear_calibration();
                        if let Some(ref p) = image_path {
                            if p.to_lowercase().ends_with(".pdf") {
                                fp2.set_pdf(p, pdf_page.unwrap_or(0));
                            } else {
                                fp2.set_image(p);
                            }
                        }
                        if let Some(p) = drawing_path { fp2.load_canvas(std::path::Path::new(&p)); }
                        if let (Some(sc), Some(a), Some(b)) = (scale, calib_a, calib_b) {
                            fp2.set_scale(sc, a, b);
                        }
                        fp2.set_origin(origin);
                        fp2.set_measurements(measurements.clone());
                        legend2.set_measurements(&measurements);
                        panel2.set_measurements(measurements);

                        auto_save(&fp2, &state2);
                        overlay2.add_toast(Toast::new("Floor deleted"));
                    }
                    _ => {}
                }
            });
        });
    }

    // Floor dropdown
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        let suppress = suppress_floor_change.clone();
        let legend = legend.clone();
        let settings = settings.clone();
        floor_dropdown.connect_selected_notify(move |dd| {
            if suppress.get() { return; }
            let new_idx = dd.selected() as usize;
            // Auto-save the floor being left
            auto_save(&fp, &state);

            let (measurements, image_path, drawing_path, scale, calib_a, calib_b, pdf_page, origin) = {
                let mut s = state.borrow_mut();
                if new_idx >= s.project.floors.len() { return; }
                s.current_floor = new_idx;
                settings.borrow_mut().last_floor_index = new_idx;
                let _ = SettingsStore::save(&settings.borrow());
                let floor = &s.project.floors[new_idx];
                (
                    floor.measurements.clone(),
                    floor.image_path.clone(),
                    floor.drawing_path.clone(),
                    floor.scale_px_per_m,
                    floor.calib_point_a,
                    floor.calib_point_b,
                    floor.pdf_page,
                    floor.origin,
                )
            };

            fp.set_image("");
            fp.clear_canvas();
            fp.clear_calibration();
            if let Some(ref p) = image_path {
                if p.to_lowercase().ends_with(".pdf") {
                    fp.set_pdf(p, pdf_page.unwrap_or(0));
                } else {
                    fp.set_image(p);
                }
            }
            if let Some(p) = drawing_path { fp.load_canvas(std::path::Path::new(&p)); }
            if let (Some(s), Some(a), Some(b)) = (scale, calib_a, calib_b) {
                fp.set_scale(s, a, b);
            }
            fp.set_origin(origin);
            fp.set_measurements(measurements.clone());
            legend.set_measurements(&measurements);
            panel.set_measurements(measurements);
        });
    }

    // Import floor plan image
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let overlay_ref = overlay.clone();
        let window_ref = window.clone();
        import_btn.connect_clicked(move |_| {
            let dialog = FileDialog::builder().title("Import Floor Plan").modal(true).build();
            let filter = gtk4::FileFilter::new();
            filter.add_mime_type("image/png");
            filter.add_mime_type("image/jpeg");
            filter.add_mime_type("application/pdf");
            filter.set_name(Some("Images & PDF (PNG, JPG, PDF)"));
            let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&filter);
            dialog.set_filters(Some(&filters));

            let state2 = state.clone();
            let fp2 = fp.clone();
            let overlay2 = overlay_ref.clone();
            let window_ref2 = window_ref.clone();
            dialog.open(Some(&window_ref), gtk4::gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let path_str = path.to_string_lossy().to_string();
                        if path_str.to_lowercase().ends_with(".pdf") {
                            // Count pages; if > 1 show page-picker dialog
                            match poppler::PopplerDocument::new_from_file(&path_str, None) {
                                Err(e) => {
                                    log::warn!("Cannot open PDF {path_str}: {e}");
                                    overlay2.add_toast(Toast::new("Failed to open PDF"));
                                }
                                Ok(doc) => {
                                    let n_pages = doc.get_n_pages();
                                    if n_pages <= 1 {
                                        import_pdf_page(&state2, &fp2, &overlay2, path_str, 0);
                                    } else {
                                        show_pdf_page_picker(&window_ref2, &state2, &fp2, &overlay2, path_str, n_pages);
                                    }
                                }
                            }
                        } else {
                            let mut s = state2.borrow_mut();
                            let idx = s.current_floor;
                            if let Some(floor) = s.project.floors.get_mut(idx) {
                                floor.image_path = Some(path_str.clone());
                                floor.pdf_page = None;
                            }
                            drop(s);
                            fp2.set_image(&path_str);
                            auto_save(&fp2, &state2);
                            overlay2.add_toast(Toast::new("Floor plan imported"));
                        }
                    }
                }
            });
        });
    }

    // Zoom buttons
    {
        let fp = floor_plan.clone();
        zoom_in_btn.connect_clicked(move |_| { fp.zoom_in(); });
    }
    {
        let fp = floor_plan.clone();
        zoom_out_btn.connect_clicked(move |_| { fp.zoom_out(); });
    }
    {
        let fp = floor_plan.clone();
        zoom_reset_btn.connect_clicked(move |_| { fp.reset_zoom(); });
    }

    // Settings button
    {
        let settings = settings.clone();
        let window_ref = window.clone();
        let fp = floor_plan.clone();
        let grid_toggle = grid_toggle.clone();
        let panel_ref = panel.clone();
        settings_btn.connect_clicked(move |_| {
            let dlg = SettingsDialog::new(&window_ref, settings.clone());
            let fp2 = fp.clone();
            let grid_toggle2 = grid_toggle.clone();
            let settings2 = settings.clone();
            let panel2 = panel_ref.clone();
            dlg.window.connect_close_request(move |_| {
                let s = settings2.borrow();
                fp2.set_show_grid(s.show_grid);
                fp2.set_grid_spacing(s.grid_spacing_m);
                fp2.set_measurement_grid_spacing(s.measurement_grid_spacing_m);
                fp2.set_snap_to_grid(s.snap_to_grid);
                grid_toggle2.set_active(s.show_grid);
                panel2.set_throughput_unit(s.throughput_unit);
                gtk4::glib::Propagation::Proceed
            });
            dlg.window.present();
        });
    }

    // ── Initialize from loaded project ────────────────────────────────────────
    {
        // Collect all data needed, then release borrow before touching the model
        let (floor_names, start_floor_data, start_idx) = {
            let mut s = state.borrow_mut();
            if s.project.floors.is_empty() {
                s.project.add_floor(Floor::new("Floor 1"));
            }
            let last_idx = settings.borrow().last_floor_index;
            let start_idx = if last_idx < s.project.floors.len() { last_idx } else { 0 };
            s.current_floor = start_idx;
            let names: Vec<String> = s.project.floors.iter().map(|f| f.name.clone()).collect();
            let first = s.project.floors.get(start_idx).map(|f| (
                f.measurements.clone(),
                f.image_path.clone(),
                f.drawing_path.clone(),
                f.scale_px_per_m,
                f.calib_point_a,
                f.calib_point_b,
                f.pdf_page,
                f.origin,
            ));
            (names, first, start_idx)
        }; // state borrow fully released here

        // Suppress all notifications during bulk initialization to prevent premature
        // selected_notify firing (GTK auto-selects index 0 on first append).
        suppress_floor_change.set(true);
        for name in &floor_names {
            floor_model.append(name);
        }
        floor_dropdown.set_selected(start_idx as u32);
        suppress_floor_change.set(false);

        // Load restored floor into view
        if let Some((measurements, image_path, drawing_path, scale, calib_a, calib_b, pdf_page, origin)) = start_floor_data {
            if let Some(ref p) = image_path {
                if p.to_lowercase().ends_with(".pdf") {
                    floor_plan.set_pdf(p, pdf_page.unwrap_or(0));
                } else {
                    floor_plan.set_image(p);
                }
            }
            if let Some(p) = drawing_path { floor_plan.load_canvas(std::path::Path::new(&p)); }
            if let (Some(sc), Some(a), Some(b)) = (scale, calib_a, calib_b) {
                floor_plan.set_scale(sc, a, b);
            }
            floor_plan.set_origin(origin);
            floor_plan.set_measurements(measurements.clone());
            legend.set_measurements(&measurements);
            panel.set_measurements(measurements);
        } else {
            floor_plan.set_measurements(vec![]);
            legend.set_measurements(&[]);
            panel.set_measurements(vec![]);
        }
    }

    // Auto-save if we just created the default floor
    auto_save(&floor_plan, &state);
    // Apply saved settings to panel
    panel.set_throughput_unit(settings.borrow().throughput_unit);

    overlay
}

fn import_pdf_page(
    state: &Rc<RefCell<AppState>>,
    fp: &FloorPlanView,
    overlay: &libadwaita::ToastOverlay,
    path_str: String,
    page_idx: u32,
) {
    {
        let mut s = state.borrow_mut();
        let idx = s.current_floor;
        if let Some(floor) = s.project.floors.get_mut(idx) {
            floor.image_path = Some(path_str.clone());
            floor.pdf_page = Some(page_idx);
        }
    }
    fp.set_pdf(&path_str, page_idx);
    auto_save(fp, state);
    overlay.add_toast(libadwaita::Toast::new("Floor plan imported"));
}

fn show_pdf_page_picker(
    parent: &libadwaita::ApplicationWindow,
    state: &Rc<RefCell<AppState>>,
    fp: &FloorPlanView,
    overlay: &libadwaita::ToastOverlay,
    path_str: String,
    n_pages: usize,
) {
    use gtk4::prelude::*;

    let dialog = gtk4::Window::builder()
        .title("Select PDF Page")
        .transient_for(parent)
        .modal(true)
        .default_width(320)
        .resizable(false)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    vbox.set_margin_top(20);
    vbox.set_margin_bottom(20);
    vbox.set_margin_start(20);
    vbox.set_margin_end(20);

    let label = gtk4::Label::new(Some(&format!(
        "This PDF has {n_pages} pages.\nSelect which page to use as floor plan:"
    )));
    label.set_halign(gtk4::Align::Start);
    vbox.append(&label);

    let spin = gtk4::SpinButton::with_range(1.0, n_pages as f64, 1.0);
    spin.set_value(1.0);
    vbox.append(&spin);

    let btn_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btn_row.set_halign(gtk4::Align::End);

    let cancel_btn = gtk4::Button::with_label("Cancel");
    let ok_btn = gtk4::Button::with_label("Import");
    ok_btn.add_css_class("suggested-action");

    btn_row.append(&cancel_btn);
    btn_row.append(&ok_btn);
    vbox.append(&btn_row);

    dialog.set_child(Some(&vbox));

    // Cancel
    {
        let dialog_weak = dialog.downgrade();
        cancel_btn.connect_clicked(move |_| {
            if let Some(d) = dialog_weak.upgrade() { d.close(); }
        });
    }

    // OK
    {
        let dialog_weak = dialog.downgrade();
        let state = state.clone();
        let fp = fp.clone();
        let overlay = overlay.clone();
        ok_btn.connect_clicked(move |_| {
            let page_idx = (spin.value() as u32).saturating_sub(1);
            import_pdf_page(&state, &fp, &overlay, path_str.clone(), page_idx);
            if let Some(d) = dialog_weak.upgrade() { d.close(); }
        });
    }

    dialog.present();
}
