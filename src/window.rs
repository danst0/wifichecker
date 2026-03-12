use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, DropDown, FileDialog, Label,
    Orientation, Separator, SpinButton, StringList, ToggleButton,
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
use crate::widgets::{FloorPlanView, MeasurementPanel, SettingsDialog};
use crate::widgets::floor_plan_view::DrawMode;

struct MeasureResult {
    rx: f64,
    ry: f64,
    wifi: Option<WifiInfo>,
    iperf_mbps: Option<f64>,
    smb_mbps: Option<f64>,
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

    let clear_canvas_btn = Button::builder()
        .icon_name("edit-clear-symbolic")
        .tooltip_text("Clear drawing")
        .build();

    let stroke_label = Label::new(Some("Width:"));
    let stroke_spin = SpinButton::with_range(1.0, 20.0, 1.0);
    stroke_spin.set_value(3.0);
    stroke_spin.set_tooltip_text(Some("Stroke width"));

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

    draw_bar.append(&mode_measure);
    draw_bar.append(&mode_draw);
    draw_bar.append(&mode_calib);
    draw_bar.append(&Separator::new(Orientation::Vertical));
    draw_bar.append(&clear_canvas_btn);
    draw_bar.append(&stroke_label);
    draw_bar.append(&stroke_spin);
    draw_bar.append(&Separator::new(Orientation::Vertical));
    draw_bar.append(&grid_toggle);
    draw_bar.append(&grid_spacing_dd);
    draw_bar.append(&Separator::new(Orientation::Vertical));
    draw_bar.append(&import_btn);

    main_box.append(&draw_bar);

    // ── Body ──────────────────────────────────────────────────────────────────
    let body = GtkBox::new(Orientation::Horizontal, 0);
    body.set_vexpand(true);

    let floor_plan = FloorPlanView::new();
    floor_plan.set_show_grid(settings.borrow().show_grid);
    floor_plan.set_grid_spacing(settings.borrow().grid_spacing_m);
    body.append(&floor_plan.widget);

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

    // Stroke width
    {
        let fp = floor_plan.clone();
        stroke_spin.connect_value_changed(move |spin| fp.set_stroke_width(spin.value()));
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

        floor_plan.set_on_measure_click(move |rx, ry| {
            let (iperf_enabled, iperf_server, iperf_port, iperf_dur,
                 smb_enabled, smb_server, smb_share, smb_user, smb_pass) = {
                let s = settings.borrow();
                (
                    s.iperf_enabled, s.iperf_server.clone(), s.iperf_port, s.iperf_duration_secs,
                    s.smb_enabled, s.smb_server.clone(), s.smb_share.clone(),
                    s.smb_username.clone(), s.smb_password.clone(),
                )
            };

            let panel2 = panel.clone();
            panel2.set_measuring(true, "Scanning WiFi…");

            let (tx, recv) = async_channel::bounded::<MeasureResult>(1);
            std::thread::spawn(move || {
                let wifi = WifiScanner::scan().ok().flatten();

                let iperf_mbps = if iperf_enabled && !iperf_server.is_empty() {
                    IperfClient::new(&iperf_server, iperf_port, iperf_dur)
                        .run_test().ok()
                } else {
                    None
                };

                let smb_mbps = if smb_enabled && !smb_server.is_empty() {
                    let mut tester = SmbTester::new(&smb_server, &smb_share);
                    tester.username = if smb_user.is_empty() { None } else { Some(smb_user) };
                    tester.password = if smb_pass.is_empty() { None } else { Some(smb_pass) };
                    tester.run_test().ok()
                } else {
                    None
                };

                tx.send_blocking(MeasureResult { rx, ry, wifi, iperf_mbps, smb_mbps }).ok();
            });

            let state2 = state.clone();
            let fp2 = fp.clone();
            let panel3 = panel.clone();
            let overlay2 = overlay_ref.clone();

            glib::spawn_future_local(async move {
                let Ok(result) = recv.recv().await else { return; };
                panel3.set_measuring(false, "");

                let Some(info) = result.wifi else {
                    overlay2.add_toast(Toast::new("No active WiFi connection"));
                    return;
                };

                let mut m = Measurement::new(
                    result.rx, result.ry,
                    info.ssid.clone(), info.bssid.clone(),
                    info.frequency_mhz, info.channel, info.signal_dbm,
                );
                m.iperf_mbps = result.iperf_mbps;
                m.smb_mbps = result.smb_mbps;

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
                panel3.set_measurements(panel_measurements);
                panel3.update_current_wifi(
                    &info.ssid, &info.bssid,
                    info.signal_dbm, info.frequency_mhz, info.channel,
                    result.iperf_mbps, result.smb_mbps,
                );
                auto_save(&fp2, &state2);
                let mut toast_msg = format!("{} dBm | {}", info.signal_dbm, info.ssid);
                if let Some(mbps) = result.iperf_mbps {
                    toast_msg.push_str(&format!(" | ⚡{:.1} Mbps", mbps));
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

                let dx = bx - ax;
                let dy = by - ay;
                let rel_dist = (dx * dx + dy * dy).sqrt();
                let scale = rel_dist / real_m;

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

    // Delete measurement
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        let panel2 = panel.clone();
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
            panel2.set_measurements(measurements);
            auto_save(&fp, &state);
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

    // Floor dropdown
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        floor_dropdown.connect_selected_notify(move |dd| {
            let new_idx = dd.selected() as usize;
            // Auto-save the floor being left
            auto_save(&fp, &state);

            let (measurements, image_path, drawing_path, scale, calib_a, calib_b) = {
                let mut s = state.borrow_mut();
                if new_idx >= s.project.floors.len() { return; }
                s.current_floor = new_idx;
                let floor = &s.project.floors[new_idx];
                (
                    floor.measurements.clone(),
                    floor.image_path.clone(),
                    floor.drawing_path.clone(),
                    floor.scale_px_per_m,
                    floor.calib_point_a,
                    floor.calib_point_b,
                )
            };

            if let Some(p) = image_path { fp.set_image(&p); }
            if let Some(p) = drawing_path { fp.load_canvas(std::path::Path::new(&p)); }
            if let (Some(s), Some(a), Some(b)) = (scale, calib_a, calib_b) {
                fp.set_scale(s, a, b);
            } else {
                fp.set_scale_px_per_m(None);
            }
            fp.set_measurements(measurements.clone());
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
            filter.set_name(Some("Images (PNG, JPG)"));
            let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&filter);
            dialog.set_filters(Some(&filters));

            let state2 = state.clone();
            let fp2 = fp.clone();
            let overlay2 = overlay_ref.clone();
            dialog.open(Some(&window_ref), gtk4::gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let path_str = path.to_string_lossy().to_string();
                        {
                            let mut s = state2.borrow_mut();
                            let idx = s.current_floor;
                            if let Some(floor) = s.project.floors.get_mut(idx) {
                                floor.image_path = Some(path_str.clone());
                            }
                        }
                        fp2.set_image(&path_str);
                        auto_save(&fp2, &state2);
                        overlay2.add_toast(Toast::new("Floor plan imported"));
                    }
                }
            });
        });
    }

    // Settings button
    {
        let settings = settings.clone();
        let window_ref = window.clone();
        let fp = floor_plan.clone();
        let grid_toggle = grid_toggle.clone();
        settings_btn.connect_clicked(move |_| {
            let dlg = SettingsDialog::new(&window_ref, settings.clone());
            let fp2 = fp.clone();
            let grid_toggle2 = grid_toggle.clone();
            let settings2 = settings.clone();
            dlg.window.connect_close_request(move |_| {
                let s = settings2.borrow();
                fp2.set_show_grid(s.show_grid);
                fp2.set_grid_spacing(s.grid_spacing_m);
                grid_toggle2.set_active(s.show_grid);
                gtk4::glib::Propagation::Proceed
            });
            dlg.window.present();
        });
    }

    // ── Initialize from loaded project ────────────────────────────────────────
    {
        let mut s = state.borrow_mut();
        if s.project.floors.is_empty() {
            s.project.add_floor(Floor::new("Floor 1"));
        }
        s.current_floor = 0;

        for f in &s.project.floors {
            floor_model.append(&f.name);
        }

        // Load first floor data
        if let Some(floor) = s.project.floors.first() {
            let measurements = floor.measurements.clone();
            let image_path = floor.image_path.clone();
            let drawing_path = floor.drawing_path.clone();
            let scale = floor.scale_px_per_m;
            let calib_a = floor.calib_point_a;
            let calib_b = floor.calib_point_b;
            drop(s);

            if let Some(p) = image_path { floor_plan.set_image(&p); }
            if let Some(p) = drawing_path { floor_plan.load_canvas(std::path::Path::new(&p)); }
            if let (Some(sc), Some(a), Some(b)) = (scale, calib_a, calib_b) {
                floor_plan.set_scale(sc, a, b);
            }
            floor_plan.set_measurements(measurements.clone());
            panel.set_measurements(measurements);
        } else {
            drop(s);
            floor_plan.set_measurements(vec![]);
            panel.set_measurements(vec![]);
        }
    }

    // Auto-save if we just created the default floor
    auto_save(&floor_plan, &state);

    overlay
}
