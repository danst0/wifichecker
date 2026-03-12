use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, DropDown, FileDialog, Orientation,
    StringList, ToggleButton,
};
use gtk4::glib;
use libadwaita::prelude::*;
use libadwaita::{ApplicationWindow, HeaderBar, Toast, ToastOverlay};
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::{Floor, Measurement, Project};
use crate::persistence::JsonStore;
use crate::services::{WifiInfo, WifiScanner};
use crate::widgets::{FloorPlanView, MeasurementPanel};

pub struct Window {
    pub window: ApplicationWindow,
}

impl Window {
    pub fn new(app: &libadwaita::Application) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("WiFi Checker")
            .default_width(1100)
            .default_height(750)
            .build();

        let state = Rc::new(RefCell::new(AppState {
            project: Project::new("New Project"),
            current_floor: 0,
            save_path: None,
            iperf_server: None,
            smb_config: None,
        }));

        let content = build_ui(&window, state.clone());
        window.set_content(Some(&content));

        Self { window }
    }
}

struct AppState {
    project: Project,
    current_floor: usize,
    save_path: Option<std::path::PathBuf>,
    iperf_server: Option<String>,
    smb_config: Option<(String, String)>, // (server, share)
}

fn build_ui(window: &ApplicationWindow, state: Rc<RefCell<AppState>>) -> ToastOverlay {
    let overlay = ToastOverlay::new();

    let main_box = GtkBox::new(Orientation::Vertical, 0);

    // ── Header bar ────────────────────────────────────────────────────────────
    let header = HeaderBar::new();

    // Floor selector
    let floor_model = StringList::new(&[]);
    let floor_dropdown = DropDown::new(Some(floor_model.clone()), gtk4::Expression::NONE);
    floor_dropdown.set_tooltip_text(Some("Select floor"));
    header.pack_start(&floor_dropdown);

    // Add floor button
    let add_floor_btn = Button::from_icon_name("list-add-symbolic");
    add_floor_btn.set_tooltip_text(Some("Add floor"));
    header.pack_start(&add_floor_btn);

    // Heatmap toggle
    let heatmap_toggle = ToggleButton::new();
    heatmap_toggle.set_icon_name("view-grid-symbolic");
    heatmap_toggle.set_tooltip_text(Some("Toggle heatmap"));
    heatmap_toggle.set_active(true);
    header.pack_end(&heatmap_toggle);

    // Save/Load buttons
    let save_btn = Button::from_icon_name("document-save-symbolic");
    save_btn.set_tooltip_text(Some("Save project"));
    header.pack_end(&save_btn);

    let open_btn = Button::from_icon_name("document-open-symbolic");
    open_btn.set_tooltip_text(Some("Open project"));
    header.pack_end(&open_btn);

    // Settings button (iperf/smb)
    let settings_btn = Button::from_icon_name("preferences-system-symbolic");
    settings_btn.set_tooltip_text(Some("Speed test settings"));
    header.pack_end(&settings_btn);

    main_box.append(&header);

    // ── Body: floor plan + sidebar ────────────────────────────────────────────
    let body = GtkBox::new(Orientation::Horizontal, 0);
    body.set_vexpand(true);

    let floor_plan = FloorPlanView::new();
    body.append(&floor_plan.widget);

    // Right sidebar
    let sidebar = GtkBox::new(Orientation::Vertical, 6);
    sidebar.set_width_request(290);

    let panel = MeasurementPanel::new();

    // Measure button
    let measure_btn = Button::with_label("📡 Measure here");
    measure_btn.add_css_class("suggested-action");
    measure_btn.set_margin_start(6);
    measure_btn.set_margin_end(6);
    measure_btn.set_margin_top(6);

    // Import floor plan button
    let import_btn = Button::with_label("🗺 Import floor plan");
    import_btn.set_margin_start(6);
    import_btn.set_margin_end(6);

    sidebar.append(&measure_btn);
    sidebar.append(&import_btn);
    sidebar.append(&panel.widget);
    body.append(&sidebar);

    main_box.append(&body);
    overlay.set_child(Some(&main_box));

    // ── Wire up callbacks ──────────────────────────────────────────────────────

    // Heatmap toggle
    {
        let fp = floor_plan.clone();
        heatmap_toggle.connect_toggled(move |btn| {
            fp.set_show_heatmap(btn.is_active());
        });
    }

    // Click on floor plan → record position for next measurement
    let click_pos: Rc<RefCell<Option<(f64, f64)>>> = Rc::new(RefCell::new(None));
    {
        let pos = click_pos.clone();
        floor_plan.set_on_click(move |rx, ry| {
            *pos.borrow_mut() = Some((rx, ry));
        });
    }

    // Measure button
    {
        let state = state.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        let overlay_ref = overlay.clone();
        let pos = click_pos.clone();

        measure_btn.connect_clicked(move |_| {
            let Some((rx_pos, ry_pos)) = *pos.borrow() else {
                overlay_ref.add_toast(Toast::new("Click on the floor plan first to set your position"));
                return;
            };

            let (tx, rx) = async_channel::bounded::<anyhow::Result<Option<WifiInfo>>>(1);

            std::thread::spawn(move || {
                tx.send_blocking(WifiScanner::scan()).ok();
            });

            let state2 = state.clone();
            let fp2 = fp.clone();
            let panel2 = panel.clone();
            let overlay2 = overlay_ref.clone();

            glib::spawn_future_local(async move {
                if let Ok(result) = rx.recv().await {
                    match result {
                        Ok(Some(info)) => {
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
                        Ok(None) => overlay2.add_toast(Toast::new("No active WiFi connection found")),
                        Err(e) => overlay2.add_toast(Toast::new(&format!("WiFi scan error: {e}"))),
                    }
                }
            });
        });
    }

    // Delete measurement callback
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

    // Add floor button
    {
        let state = state.clone();
        let floor_model = floor_model.clone();
        let fp = floor_plan.clone();
        let panel = panel.clone();
        let overlay_ref = overlay.clone();
        add_floor_btn.connect_clicked(move |_| {
            let mut s = state.borrow_mut();
            let name = format!("Floor {}", s.project.floors.len() + 1);
            s.project.add_floor(Floor::new(&name));
            floor_model.append(&name);
            let idx = s.project.floors.len() - 1;
            s.current_floor = idx;
            fp.set_measurements(vec![]);
            fp.set_image(""); // clear
            panel.set_measurements(vec![]);
            let toast = Toast::new(&format!("Added: {}", name));
            overlay_ref.add_toast(toast);
        });
    }

    // Floor dropdown change
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
                drop(s);
                if let Some(path) = image_path {
                    fp.set_image(&path);
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
            let dialog = FileDialog::builder()
                .title("Import Floor Plan")
                .modal(true)
                .build();

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
                        let toast = Toast::new("Floor plan imported");
                        overlay2.add_toast(toast);
                    }
                }
            });
        });
    }

    // Save project
    {
        let state = state.clone();
        let overlay_ref = overlay.clone();
        let window_ref = window.clone();
        save_btn.connect_clicked(move |_| {
            let s = state.borrow();
            if let Some(ref path) = s.save_path {
                match JsonStore::save(&s.project, path) {
                    Ok(_) => {
                        overlay_ref.add_toast(Toast::new("Project saved"));
                    }
                    Err(e) => {
                        overlay_ref.add_toast(Toast::new(&format!("Save error: {}", e)));
                    }
                }
                return;
            }
            drop(s);

            let dialog = FileDialog::builder()
                .title("Save Project")
                .initial_name("project.wifichecker.json")
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
                            Err(e) => {
                                overlay2.add_toast(Toast::new(&format!("Save error: {}", e)));
                            }
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
            let dialog = FileDialog::builder()
                .title("Open Project")
                .modal(true)
                .build();

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
                                // Rebuild floor dropdown
                                while floor_model2.n_items() > 0 {
                                    floor_model2.remove(0);
                                }
                                for f in &project.floors {
                                    floor_model2.append(&f.name);
                                }

                                let mut s = state2.borrow_mut();
                                s.save_path = Some(path);
                                s.current_floor = 0;
                                s.project = project;

                                // Load first floor
                                if let Some(floor) = s.project.floors.first() {
                                    let m = floor.measurements.clone();
                                    let img = floor.image_path.clone();
                                    drop(s);
                                    if let Some(img_path) = img {
                                        fp2.set_image(&img_path);
                                    }
                                    fp2.set_measurements(m.clone());
                                    panel2.set_measurements(m);
                                    floor_dd2.set_selected(0);
                                }

                                overlay2.add_toast(Toast::new("Project loaded"));
                            }
                            Err(e) => {
                                overlay2.add_toast(Toast::new(&format!("Load error: {}", e)));
                            }
                        }
                    }
                }
            });
        });
    }

    overlay
}
