use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, EntryRow, PasswordEntryRow, PreferencesGroup,
    PreferencesPage, PreferencesWindow, SpinRow, SwitchRow,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::{AppSettings, ThroughputUnit};
use crate::persistence::SettingsStore;

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
        grid_switch.set_title("Show grid");
        grid_switch.set_active(settings.borrow().show_grid);

        // Grid spacing selector
        let spacing_row = ActionRow::new();
        spacing_row.set_title("Grid spacing");
        spacing_row.set_subtitle("Meters per grid cell");

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

        grid_group.add(&grid_switch);
        grid_group.add(&spacing_row);

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
        page.add(&smb_group);
        page.add(&grid_group);
        page.add(&display_group);
        win.add(&page);

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
            let unit_switch = unit_switch.clone();

            win.connect_close_request(move |_| {
                let spacing_m = spacing_values[spacing_drop.selected() as usize];
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
