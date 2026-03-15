use gtk4::prelude::*;
use gtk4::DrawingArea;
use cairo::Context;
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::Measurement;

#[derive(Clone, Copy)]
enum ColorMetric { SmbMbps, IperfMbps, SignalDbm }

fn value_color(val: f64, min: f64, max: f64) -> (f64, f64, f64) {
    let t = if max > min { ((val - min) / (max - min)).clamp(0.0, 1.0) } else { 0.5 };
    if t >= 0.5 { (1.0 - (t - 0.5) * 2.0, 1.0, 0.0) } else { (1.0, t * 2.0, 0.0) }
}

fn active_metric(measurements: &[Measurement]) -> ColorMetric {
    if measurements.iter().any(|m| m.smb_mbps.is_some())   { return ColorMetric::SmbMbps; }
    if measurements.iter().any(|m| m.iperf_mbps.is_some()) { return ColorMetric::IperfMbps; }
    ColorMetric::SignalDbm
}

fn metric_value(m: &Measurement, metric: ColorMetric) -> Option<f64> {
    match metric {
        ColorMetric::SmbMbps   => m.smb_mbps,
        ColorMetric::IperfMbps => m.iperf_mbps,
        ColorMetric::SignalDbm => Some(m.signal_dbm as f64),
    }
}

#[derive(Clone)]
pub struct LegendBar {
    pub widget: DrawingArea,
    state: Rc<RefCell<LegendState>>,
}

struct LegendState {
    metric: ColorMetric,
    min: f64,
    max: f64,
    active: bool,
}

impl LegendBar {
    pub fn new() -> Self {
        let area = DrawingArea::new();
        area.set_height_request(46);
        area.set_hexpand(true);
        area.set_visible(false);

        let state = Rc::new(RefCell::new(LegendState {
            metric: ColorMetric::SignalDbm,
            min: -90.0,
            max: -30.0,
            active: false,
        }));

        {
            let state = state.clone();
            area.set_draw_func(move |_area, ctx, w, _h| {
                let s = state.borrow();
                if !s.active { return; }
                draw_legend(ctx, w as f64, s.metric, s.min, s.max);
            });
        }

        Self { widget: area, state }
    }

    pub fn set_measurements(&self, measurements: &[Measurement]) {
        if measurements.is_empty() {
            self.state.borrow_mut().active = false;
            self.widget.set_visible(false);
            return;
        }
        let metric = active_metric(measurements);
        let values: Vec<f64> = measurements.iter()
            .filter_map(|m| metric_value(m, metric))
            .collect();
        let (min, max) = if values.is_empty() {
            (-90.0, -30.0)
        } else {
            let lo = values.iter().cloned().fold(f64::MAX, f64::min);
            let hi = values.iter().cloned().fold(f64::MIN, f64::max);
            if (hi - lo).abs() < 1.0 { (lo - 5.0, hi + 5.0) } else { (lo, hi) }
        };
        {
            let mut s = self.state.borrow_mut();
            s.metric = metric;
            s.min = min;
            s.max = max;
            s.active = true;
        }
        self.widget.set_visible(true);
        self.widget.queue_draw();
    }
}

fn draw_legend(ctx: &Context, w: f64, metric: ColorMetric, min: f64, max: f64) {
    const MARGIN: f64 = 8.0;
    const BAR_Y: f64 = 4.0;
    const BAR_H: f64 = 20.0;

    let bar_w = w - MARGIN * 2.0;
    if bar_w <= 0.0 { return; }

    // Gradient bar — one 1-px column per pixel
    for xi in 0..(bar_w as i32) {
        let t = xi as f64 / bar_w;
        let val = min + (max - min) * t;
        let (r, g, b) = value_color(val, min, max);
        ctx.set_source_rgb(r, g, b);
        ctx.rectangle(MARGIN + xi as f64, BAR_Y, 1.0, BAR_H);
        ctx.fill().unwrap();
    }

    // Border
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.3);
    ctx.set_line_width(1.0);
    ctx.rectangle(MARGIN, BAR_Y, bar_w, BAR_H);
    ctx.stroke().unwrap();

    // Labels
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.set_font_size(11.0);
    let unit = match metric {
        ColorMetric::SignalDbm => " dBm",
        _ => " Mbit/s",
    };
    let metric_name = match metric {
        ColorMetric::SmbMbps   => "Samba",
        ColorMetric::IperfMbps => "iperf",
        ColorMetric::SignalDbm => "Signal",
    };
    let mid = (min + max) / 2.0;
    let label_y = BAR_Y + BAR_H + 14.0;

    // Metric name — leftmost
    ctx.move_to(MARGIN, label_y);
    let _ = ctx.show_text(metric_name);

    // Min label (after metric name, with some gap)
    let min_label = format!("{:.0}{}", min, unit);
    ctx.move_to(MARGIN + 46.0, label_y);
    let _ = ctx.show_text(&min_label);

    // Mid label — centred
    let mid_label = format!("{:.0}{}", mid, unit);
    if let Ok(ext) = ctx.text_extents(&mid_label) {
        ctx.move_to(MARGIN + bar_w / 2.0 - ext.width() / 2.0, label_y);
        let _ = ctx.show_text(&mid_label);
    }

    // Max label — right-aligned
    let max_label = format!("{:.0}{}", max, unit);
    if let Ok(ext) = ctx.text_extents(&max_label) {
        ctx.move_to(MARGIN + bar_w - ext.width(), label_y);
        let _ = ctx.show_text(&max_label);
    }
}
