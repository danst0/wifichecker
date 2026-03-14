use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, EntryRow, PasswordEntryRow, PreferencesGroup,
    PreferencesPage, PreferencesWindow, SpinRow, SwitchRow,
};

const APP_VERSION: &str = env!("APP_VERSION");
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::{AppSettings, ThroughputUnit};
use crate::persistence::SettingsStore;
use crate::utils::flatpak::is_flatpak;

pub struct SettingsDialog {
    pub window: PreferencesWindow,
}

impl SettingsDialog {
    pub fn new(parent: &impl gtk4::prelude::IsA<gtk4::Window>, settings: Rc<RefCell<AppSettings>>) -> Self {
        let win = PreferencesWindow::builder()
            .title("Settings")
            .transient_for(parent)
            .modal(true)
            .build();

        let page = PreferencesPage::new();
        page.set_title("Speed Tests");
        page.set_icon_name(Some("network-wired-symbolic"));

        // ── iperf3 group ──────────────────────────────────────────────────
        let iperf_group = PreferencesGroup::new();
        iperf_group.set_title("iperf3 Speed Test");
        iperf_group.set_description(Some("Test real throughput with an iperf3 server"));

        let s = settings.borrow();

        let iperf_switch = SwitchRow::new();
        iperf_switch.set_title("Enable iperf3 test");
        iperf_switch.set_active(s.iperf_enabled);

        let iperf_server = EntryRow::new();
        iperf_server.set_title("Server address");
        iperf_server.set_text(&s.iperf_server);

        let iperf_port = SpinRow::with_range(1.0, 65535.0, 1.0);
        iperf_port.set_title("Port");
        iperf_port.set_value(s.iperf_port as f64);

        let iperf_dur = SpinRow::with_range(1.0, 60.0, 1.0);
        iperf_dur.set_title("Duration (seconds)");
        iperf_dur.set_value(s.iperf_duration_secs as f64);

        iperf_group.add(&iperf_switch);
        iperf_group.add(&iperf_server);
        iperf_group.add(&iperf_port);
        iperf_group.add(&iperf_dur);

        // ── Samba group ───────────────────────────────────────────────────
        let smb_group = PreferencesGroup::new();
        smb_group.set_title("Samba Speed Test");
        smb_group.set_description(Some("Test throughput by uploading a file to a Samba share"));

        let smb_switch = SwitchRow::new();
        smb_switch.set_title("Enable Samba test");
        smb_switch.set_active(s.smb_enabled);

        let smb_server = EntryRow::new();
        smb_server.set_title("Server address");
        smb_server.set_text(&s.smb_server);

        let smb_share = EntryRow::new();
        smb_share.set_title("Share name");
        smb_share.set_text(&s.smb_share);

        let smb_user = EntryRow::new();
        smb_user.set_title("Username");
        smb_user.set_text(&s.smb_username);

        let smb_pass = PasswordEntryRow::new();
        smb_pass.set_title("Password");
        smb_pass.set_text(&s.smb_password);

        smb_group.add(&smb_switch);
        smb_group.add(&smb_server);
        smb_group.add(&smb_share);
        smb_group.add(&smb_user);
        smb_group.add(&smb_pass);

        drop(s);

        // ── Grid group ────────────────────────────────────────────────────
        let grid_group = PreferencesGroup::new();
        grid_group.set_title("Grid Overlay");
        grid_group.set_description(Some("Background measurement grid"));

        let grid_switch = SwitchRow::new();
        grid_switch.set_title("Show visual grid");
        grid_switch.set_active(settings.borrow().show_grid);

        // Visual grid spacing selector
        let spacing_row = ActionRow::new();
        spacing_row.set_title("Visual grid spacing");
        spacing_row.set_subtitle("Background grid line spacing");

        let spacing_model = gtk4::StringList::new(&["0.5 m", "1 m", "2 m", "5 m"]);
        let spacing_values = [0.5f64, 1.0, 2.0, 5.0];
        let current_spacing = settings.borrow().grid_spacing_m;
        let selected_idx = spacing_values
            .iter()
            .position(|&v| (v - current_spacing).abs() < 0.01)
            .unwrap_or(1) as u32;
        let spacing_drop = gtk4::DropDown::new(Some(spacing_model), gtk4::Expression::NONE);
        spacing_drop.set_selected(selected_idx);
        spacing_drop.set_valign(gtk4::Align::Center);
        spacing_row.add_suffix(&spacing_drop);

        // Measurement grid spacing selector
        let meas_spacing_row = ActionRow::new();
        meas_spacing_row.set_title("Measurement grid spacing");
        meas_spacing_row.set_subtitle("Cell size for snapping and coloring");

        let meas_spacing_model = gtk4::StringList::new(&["0.5 m", "1 m", "2 m", "5 m"]);
        let meas_spacing_values = [0.5f64, 1.0, 2.0, 5.0];
        let current_meas_spacing = settings.borrow().measurement_grid_spacing_m;
        let meas_selected_idx = meas_spacing_values
            .iter()
            .position(|&v| (v - current_meas_spacing).abs() < 0.01)
            .unwrap_or(1) as u32;
        let meas_spacing_drop = gtk4::DropDown::new(Some(meas_spacing_model), gtk4::Expression::NONE);
        meas_spacing_drop.set_selected(meas_selected_idx);
        meas_spacing_drop.set_valign(gtk4::Align::Center);
        meas_spacing_row.add_suffix(&meas_spacing_drop);

        let snap_switch = SwitchRow::new();
        snap_switch.set_title("Snap to measurement grid");
        snap_switch.set_subtitle("Place measurements at cell centers");
        snap_switch.set_active(settings.borrow().snap_to_grid);

        grid_group.add(&grid_switch);
        grid_group.add(&spacing_row);
        grid_group.add(&meas_spacing_row);
        grid_group.add(&snap_switch);

        // ── Display group ─────────────────────────────────────────────────
        let display_group = PreferencesGroup::new();
        display_group.set_title("Display");
        display_group.set_description(Some("How throughput values are shown"));

        let unit_switch = SwitchRow::new();
        unit_switch.set_title("Show throughput as MByte/s");
        unit_switch.set_subtitle("Off = Mbit/s  |  On = MByte/s");
        unit_switch.set_active(settings.borrow().throughput_unit == ThroughputUnit::MByte);
        display_group.add(&unit_switch);

        page.add(&iperf_group);
        if !is_flatpak() {
            page.add(&smb_group);
        }
        page.add(&grid_group);
        page.add(&display_group);
        win.add(&page);

        // ── About page ────────────────────────────────────────────────────
        let about_page = PreferencesPage::new();
        about_page.set_title("About");
        about_page.set_icon_name(Some("help-about-symbolic"));

        // App info group
        let info_group = PreferencesGroup::new();
        info_group.set_title("WiFi Checker");
        info_group.set_description(Some("WiFi signal strength analyzer and floor plan mapper"));

        let version_row = ActionRow::new();
        version_row.set_title("Version");
        version_row.set_subtitle(APP_VERSION);
        let version_label = gtk4::Label::new(Some(APP_VERSION));
        version_label.add_css_class("dim-label");
        version_label.set_valign(gtk4::Align::Center);

        let author_row = ActionRow::new();
        author_row.set_title("Author");
        author_row.set_subtitle("Dr. Daniel Dumke");

        let license_row = ActionRow::new();
        license_row.set_title("License");
        license_row.set_subtitle("MIT");

        info_group.add(&version_row);
        info_group.add(&author_row);
        info_group.add(&license_row);

        // Links group
        let links_group = PreferencesGroup::new();
        links_group.set_title("Links");

        let source_row = ActionRow::new();
        source_row.set_title("Source Code");
        source_row.set_subtitle("github.com/danst0/wifichecker");
        source_row.set_activatable(true);
        source_row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
        source_row.connect_activated(|_| {
            gtk4::UriLauncher::new("https://github.com/danst0/wifichecker")
                .launch(gtk4::Window::NONE, gtk4::gio::Cancellable::NONE, |_| {});
        });

        let issues_row = ActionRow::new();
        issues_row.set_title("Report an Issue");
        issues_row.set_subtitle("github.com/danst0/wifichecker/issues");
        issues_row.set_activatable(true);
        issues_row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
        issues_row.connect_activated(|_| {
            gtk4::UriLauncher::new("https://github.com/danst0/wifichecker/issues")
                .launch(gtk4::Window::NONE, gtk4::gio::Cancellable::NONE, |_| {});
        });

        links_group.add(&source_row);
        links_group.add(&issues_row);

        // System tools group
        let tools_group = PreferencesGroup::new();
        tools_group.set_title("System Dependencies");
        tools_group.set_description(Some("External tools used at runtime"));

        let system_tools: &[(&str, &str)] = if is_flatpak() {
            &[
                ("nmcli", "WiFi scanning via NetworkManager"),
                ("iperf3 / iperf2", "Network throughput testing"),
            ]
        } else {
            &[
                ("nmcli", "WiFi scanning via NetworkManager"),
                ("iperf3 / iperf2", "Network throughput testing"),
                ("smbclient", "Samba share speed testing"),
            ]
        };
        for (tool, purpose) in system_tools {
            let row = ActionRow::new();
            row.set_title(tool);
            row.set_subtitle(purpose);
            tools_group.add(&row);
        }

        about_page.add(&info_group);
        about_page.add(&links_group);
        about_page.add(&tools_group);
        win.add(&about_page);

        // ── Save on close ─────────────────────────────────────────────────
        {
            let settings = settings.clone();
            let iperf_switch = iperf_switch.clone();
            let iperf_server = iperf_server.clone();
            let iperf_port = iperf_port.clone();
            let iperf_dur = iperf_dur.clone();
            let smb_switch = smb_switch.clone();
            let smb_server = smb_server.clone();
            let smb_share = smb_share.clone();
            let smb_user = smb_user.clone();
            let smb_pass = smb_pass.clone();
            let grid_switch = grid_switch.clone();
            let snap_switch = snap_switch.clone();
            let unit_switch = unit_switch.clone();

            win.connect_close_request(move |_| {
                let spacing_m = spacing_values[spacing_drop.selected() as usize];
                let meas_spacing_m = meas_spacing_values[meas_spacing_drop.selected() as usize];
                let mut s = settings.borrow_mut();
                s.iperf_enabled = iperf_switch.is_active();
                s.iperf_server = iperf_server.text().to_string();
                s.iperf_port = iperf_port.value() as u16;
                s.iperf_duration_secs = iperf_dur.value() as u32;
                s.smb_enabled = smb_switch.is_active();
                s.smb_server = smb_server.text().to_string();
                s.smb_share = smb_share.text().to_string();
                s.smb_username = smb_user.text().to_string();
                s.smb_password = smb_pass.text().to_string();
                s.show_grid = grid_switch.is_active();
                s.grid_spacing_m = spacing_m;
                s.measurement_grid_spacing_m = meas_spacing_m;
                s.snap_to_grid = snap_switch.is_active();
                s.throughput_unit = if unit_switch.is_active() {
                    ThroughputUnit::MByte
                } else {
                    ThroughputUnit::Mbit
                };
                let _ = SettingsStore::save(&s);
                gtk4::glib::Propagation::Proceed
            });
        }

        Self { window: win }
    }
}
