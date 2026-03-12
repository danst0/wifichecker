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
use crate::services::{WifiInfo, WifiScanner};
use crate::widgets::{FloorPlanView, MeasurementPanel, SettingsDialog};
use crate::widgets::floor_plan_view::DrawMode;

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

        let state = Rc::new(RefCell::new(AppState {
            project: Project::new("New Project"),
            current_floor: 0,
            save_path: None,
        }));

        let content = build_ui(&window, state.clone(), settings.clone());
        window.set_content(Some(&content));

        Self { window }
    }
}

struct AppState {
    project: Project,
    current_floor: usize,
    save_path: Option<std::path::PathBuf>,
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

    let save_btn = Button::from_icon_name("document-save-symbolic");
    save_btn.set_tooltip_text(Some("Save project"));
    header.pack_end(&save_btn);

    let open_btn = Button::from_icon_name("document-open-symbolic");
    open_btn.set_tooltip_text(Some("Open project"));
    header.pack_end(&open_btn);

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

    let measure_btn = Button::with_label("📡 Measure here");
    measure_btn.add_css_class("suggested-action");
    measure_btn.set_margin_start(6);
    measure_btn.set_margin_end(6);
    measure_btn.set_margin_top(6);

    sidebar.append(&measure_btn);
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
            // Remove drawing file from floor model
            let mut s = state.borrow_mut();
            let idx = s.current_floor;
            if let Some(floor) = s.project.floors.get_mut(idx) {
                floor.drawing_path = None;
            }
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
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        let overlay_ref = overlay.clone();

        measure_btn.connect_clicked(move |_| {
            let (tx, rx) = async_channel::bounded::<anyhow::Result<Option<WifiInfo>>>(1);
            std::thread::spawn(move || { tx.send_blocking(WifiScanner::scan()).ok(); });

            let state2 = state.clone();
            let fp2 = fp.clone();
            let panel2 = panel.clone();
            let overlay2 = overlay_ref.clone();

            glib::spawn_future_local(async move {
                if let Ok(result) = rx.recv().await {
                    match result {
                        Ok(Some(info)) => {
                            let pos = LAST_CLICK_POS.with(|c| *c.borrow());
                            let Some((rx_pos, ry_pos)) = pos else {
                                overlay2.add_toast(Toast::new("Click on the floor plan first"));
                                return;
                            };

                            let m = Measurement::new(
                                rx_pos, ry_pos,
                                info.ssid.clone(), info.bssid.clone(),
                                info.frequency_mhz, info.channel, info.signal_dbm,
                            );
                            let mut s = state2.borrow_mut();
                            let idx = s.current_floor;
                            if let Some(floor) = s.project.floors.get_mut(idx) {
                                floor.add_measurement(m);
                                let measurements = floor.measurements.clone();
                                drop(s);
                                fp2.set_measurements(measurements.clone());
                                panel2.set_measurements(measurements);
                                panel2.update_current_wifi(
                                    &info.ssid, &info.bssid,
                                    info.signal_dbm, info.frequency_mhz, info.channel,
                                );
                                overlay2.add_toast(Toast::new(&format!(
                                    "Measured: {} dBm on {}", info.signal_dbm, info.ssid
                                )));
                            }
                        }
                        Ok(None) => overlay2.add_toast(Toast::new("No active WiFi connection")),
                        Err(e) => overlay2.add_toast(Toast::new(&format!("WiFi error: {e}"))),
                    }
                }
            });
        });
    }

    // Measure click position tracking via thread-local
    {
        floor_plan.set_on_measure_click(move |rx, ry| {
            LAST_CLICK_POS.with(|c| *c.borrow_mut() = Some((rx, ry)));
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

            // Entry for distance
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

                let mut s = state2.borrow_mut();
                let idx = s.current_floor;
                if let Some(floor) = s.project.floors.get_mut(idx) {
                    floor.scale_px_per_m = Some(scale);
                    floor.calib_point_a = Some((ax, ay));
                    floor.calib_point_b = Some((bx, by));
                }
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
            let mut s = state.borrow_mut();
            let idx = s.current_floor;
            if let Some(floor) = s.project.floors.get_mut(idx) {
                floor.remove_measurement(&id);
                let m = floor.measurements.clone();
                drop(s);
                fp.set_measurements(m.clone());
                panel2.set_measurements(m);
            }
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
            // Compute name + add to project, then DROP the borrow before touching the model
            let (name, new_idx) = {
                let mut s = state.borrow_mut();
                let name = format!("Floor {}", s.project.floors.len() + 1);
                s.project.add_floor(Floor::new(&name));
                s.current_floor = s.project.floors.len() - 1;
                (name, s.current_floor)
            }; // borrow released here

            floor_model.append(&name);
            // Explicitly select the new floor; this fires connect_selected_notify
            // which will load measurements/canvas. If already at this index
            // (first floor: 0→0), also update manually.
            if floor_dropdown.selected() as usize == new_idx {
                fp.set_measurements(vec![]);
                fp.set_image("");
                panel.set_measurements(vec![]);
            } else {
                floor_dropdown.set_selected(new_idx as u32);
            }
            overlay_ref.add_toast(Toast::new(&format!("Added: {name}")));
        });
    }

    // Floor dropdown
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        floor_dropdown.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            let mut s = state.borrow_mut();
            if idx < s.project.floors.len() {
                s.current_floor = idx;
                let floor = &s.project.floors[idx];
                let measurements = floor.measurements.clone();
                let image_path = floor.image_path.clone();
                let drawing_path = floor.drawing_path.clone();
                let scale = floor.scale_px_per_m;
                let calib_a = floor.calib_point_a;
                let calib_b = floor.calib_point_b;
                drop(s);
                if let Some(p) = image_path { fp.set_image(&p); }
                if let Some(p) = drawing_path { fp.load_canvas(std::path::Path::new(&p)); }
                if let (Some(s), Some(a), Some(b)) = (scale, calib_a, calib_b) {
                    fp.set_scale(s, a, b);
                } else {
                    fp.set_scale_px_per_m(None);
                }
                fp.set_measurements(measurements.clone());
                panel.set_measurements(measurements);
            }
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
                        let mut s = state2.borrow_mut();
                        let idx = s.current_floor;
                        if let Some(floor) = s.project.floors.get_mut(idx) {
                            floor.image_path = Some(path_str.clone());
                        }
                        drop(s);
                        fp2.set_image(&path_str);
                        overlay2.add_toast(Toast::new("Floor plan imported"));
                    }
                }
            });
        });
    }

    // Save — auto-save canvas before saving project
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let overlay_ref = overlay.clone();
        let window_ref = window.clone();
        save_btn.connect_clicked(move |_| {
            // Save canvas first
            save_current_canvas(&fp, &state);

            let s = state.borrow();
            if let Some(ref path) = s.save_path {
                let path = path.clone();
                let project = s.project.clone();
                drop(s);
                match JsonStore::save(&project, &path) {
                    Ok(_) => overlay_ref.add_toast(Toast::new("Project saved")),
                    Err(e) => overlay_ref.add_toast(Toast::new(&format!("Save error: {e}"))),
                }
                return;
            }
            drop(s);

            let dialog = FileDialog::builder()
                .title("Save Project")
                .initial_name("project.json")
                .modal(true)
                .build();
            let state2 = state.clone();
            let overlay2 = overlay_ref.clone();
            dialog.save(Some(&window_ref), gtk4::gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let s = state2.borrow();
                        match JsonStore::save(&s.project, &path) {
                            Ok(_) => {
                                drop(s);
                                state2.borrow_mut().save_path = Some(path);
                                overlay2.add_toast(Toast::new("Project saved"));
                            }
                            Err(e) => overlay2.add_toast(Toast::new(&format!("Save error: {e}"))),
                        }
                    }
                }
            });
        });
    }

    // Open project
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        let floor_model = floor_model.clone();
        let floor_dropdown = floor_dropdown.clone();
        let overlay_ref = overlay.clone();
        let window_ref = window.clone();
        open_btn.connect_clicked(move |_| {
            let dialog = FileDialog::builder().title("Open Project").modal(true).build();
            let filter = gtk4::FileFilter::new();
            filter.add_pattern("*.json");
            filter.set_name(Some("WiFi Checker Project (*.json)"));
            let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&filter);
            dialog.set_filters(Some(&filters));

            let state2 = state.clone();
            let fp2 = fp.clone();
            let panel2 = panel.clone();
            let floor_model2 = floor_model.clone();
            let floor_dd2 = floor_dropdown.clone();
            let overlay2 = overlay_ref.clone();
            dialog.open(Some(&window_ref), gtk4::gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        match JsonStore::load(&path) {
                            Ok(project) => {
                                while floor_model2.n_items() > 0 { floor_model2.remove(0); }
                                for f in &project.floors { floor_model2.append(&f.name); }

                                let mut s = state2.borrow_mut();
                                s.save_path = Some(path);
                                s.current_floor = 0;
                                s.project = project;

                                if let Some(floor) = s.project.floors.first() {
                                    let m = floor.measurements.clone();
                                    let img = floor.image_path.clone();
                                    let drawing = floor.drawing_path.clone();
                                    let scale = floor.scale_px_per_m;
                                    let ca = floor.calib_point_a;
                                    let cb = floor.calib_point_b;
                                    drop(s);
                                    if let Some(p) = img { fp2.set_image(&p); }
                                    if let Some(p) = drawing { fp2.load_canvas(std::path::Path::new(&p)); }
                                    if let (Some(sc), Some(a), Some(b)) = (scale, ca, cb) {
                                        fp2.set_scale(sc, a, b);
                                    }
                                    fp2.set_measurements(m.clone());
                                    panel2.set_measurements(m);
                                    floor_dd2.set_selected(0);
                                }

                                overlay2.add_toast(Toast::new("Project loaded"));
                            }
                            Err(e) => overlay2.add_toast(Toast::new(&format!("Load error: {e}"))),
                        }
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
            // Sync grid state back when dialog closes
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

    // ── Initialize with one default floor ─────────────────────────────────────
    {
        let mut s = state.borrow_mut();
        s.project.add_floor(Floor::new("Floor 1"));
        s.current_floor = 0;
        drop(s);
        floor_model.append("Floor 1");
        // selected_notify fires for index 0 → 0 (no change), so update manually
        floor_plan.set_measurements(vec![]);
        panel.set_measurements(vec![]);
    }

    overlay
}

/// Save the current floor's drawing canvas to the drawings directory.
fn save_current_canvas(fp: &FloorPlanView, state: &Rc<RefCell<AppState>>) {
    let idx = state.borrow().current_floor;
    let drawings = drawings_dir();
    let canvas_path = drawings.join(format!("floor_{idx}.png"));
    if fp.save_canvas(&canvas_path).is_ok() {
        let mut s = state.borrow_mut();
        if let Some(floor) = s.project.floors.get_mut(idx) {
            floor.drawing_path = Some(canvas_path.to_string_lossy().to_string());
        }
    }
}

thread_local! {
    static LAST_CLICK_POS: RefCell<Option<(f64, f64)>> = RefCell::new(None);
}
