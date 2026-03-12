use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow};
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use crate::models::Measurement;

#[derive(Clone)]
pub struct MeasurementPanel {
    pub widget: GtkBox,
    current_label: Label,
    list: ListBox,
    measurements: Rc<RefCell<Vec<Measurement>>>,
    on_delete: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
}

impl MeasurementPanel {
    pub fn new() -> Self {
        let vbox = GtkBox::new(Orientation::Vertical, 6);
        vbox.set_width_request(280);
        vbox.set_margin_start(6);
        vbox.set_margin_end(6);
        vbox.set_margin_top(6);
        vbox.set_margin_bottom(6);

        // Current WiFi info section
        let current_label = Label::new(Some("No WiFi data"));
        current_label.set_xalign(0.0);
        current_label.set_wrap(true);
        current_label.add_css_class("caption");

        let current_group = libadwaita::PreferencesGroup::new();
        current_group.set_title("Current Signal");
        current_group.add(&current_label);
        vbox.append(&current_group);

        // Measurements list
        let list_group = libadwaita::PreferencesGroup::new();
        list_group.set_title("Measurements");

        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::Single);
        list.add_css_class("boxed-list");

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scroll.set_child(Some(&list));

        vbox.append(&list_group);
        vbox.append(&scroll);

        let measurements = Rc::new(RefCell::new(Vec::<Measurement>::new()));
        let on_delete: Rc<RefCell<Option<Box<dyn Fn(String)>>>> = Rc::new(RefCell::new(None));

        Self {
            widget: vbox,
            current_label,
            list,
            measurements,
            on_delete,
        }
    }

    pub fn update_current_wifi(&self, ssid: &str, bssid: &str, dbm: i32, freq: u32, channel: u8) {
        let band = if freq >= 5000 { "5 GHz" } else { "2.4 GHz" };
        let quality = signal_quality_str(dbm);
        let text = format!(
            "SSID: {}\nBSSID: {}\nSignal: {} dBm ({})\nBand: {} | Ch: {}",
            ssid, bssid, dbm, quality, band, channel
        );
        self.current_label.set_label(&text);
    }

    pub fn set_no_wifi(&self) {
        self.current_label.set_label("No WiFi connection detected");
    }

    pub fn set_measurements(&self, measurements: Vec<Measurement>) {
        *self.measurements.borrow_mut() = measurements.clone();
        self.rebuild_list(&measurements);
    }

    pub fn set_on_delete<F: Fn(String) + 'static>(&self, cb: F) {
        *self.on_delete.borrow_mut() = Some(Box::new(cb));
    }

    fn rebuild_list(&self, measurements: &[Measurement]) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }

        for m in measurements.iter().rev() {
            let row = self.make_row(m);
            self.list.append(&row);
        }
    }

    fn make_row(&self, m: &Measurement) -> ListBoxRow {
        let hbox = GtkBox::new(Orientation::Horizontal, 6);
        hbox.set_margin_start(6);
        hbox.set_margin_end(6);
        hbox.set_margin_top(4);
        hbox.set_margin_bottom(4);

        let info = Label::new(Some(&format!(
            "{} | {} dBm | {}",
            m.ssid,
            m.signal_dbm,
            m.timestamp.format("%H:%M:%S")
        )));
        info.set_hexpand(true);
        info.set_xalign(0.0);
        info.set_ellipsize(gtk4::pango::EllipsizeMode::End);

        let del_btn = Button::from_icon_name("edit-delete-symbolic");
        del_btn.add_css_class("flat");
        del_btn.set_tooltip_text(Some("Delete measurement"));

        let id = m.id.clone();
        let on_delete = self.on_delete.clone();
        del_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *on_delete.borrow() {
                cb(id.clone());
            }
        });

        hbox.append(&info);
        hbox.append(&del_btn);

        let row = ListBoxRow::new();
        row.set_child(Some(&hbox));
        row
    }
}

fn signal_quality_str(dbm: i32) -> &'static str {
    match dbm {
        -50..=0 => "Excellent",
        -60..=-51 => "Good",
        -70..=-61 => "Fair",
        -80..=-71 => "Poor",
        _ => "No signal",
    }
}
