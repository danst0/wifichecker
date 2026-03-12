use gtk4::prelude::*;
use gtk4::{DrawingArea, GestureClick};
use gdk_pixbuf::Pixbuf;
use cairo::{Context, ImageSurface, Format};
use std::cell::RefCell;
use std::rc::Rc;
use crate::models::Measurement;
use crate::heatmap::HeatmapRenderer;

#[derive(Clone)]
pub struct FloorPlanView {
    pub widget: DrawingArea,
    state: Rc<RefCell<FloorPlanState>>,
}

struct FloorPlanState {
    image: Option<ImageSurface>,
    measurements: Vec<Measurement>,
    show_heatmap: bool,
    heatmap_alpha: f64,
    on_click: Option<Box<dyn Fn(f64, f64)>>,
}

impl FloorPlanView {
    pub fn new() -> Self {
        let area = DrawingArea::new();
        area.set_hexpand(true);
        area.set_vexpand(true);
        area.set_content_width(600);
        area.set_content_height(400);

        let state = Rc::new(RefCell::new(FloorPlanState {
            image: None,
            measurements: Vec::new(),
            show_heatmap: true,
            heatmap_alpha: 0.55,
            on_click: None,
        }));

        {
            let state = state.clone();
            area.set_draw_func(move |_area, ctx, w, h| {
                draw_floor_plan(&state.borrow(), ctx, w, h);
            });
        }

        let gesture = GestureClick::new();
        {
            let state = state.clone();
            let area_weak = area.downgrade();
            gesture.connect_released(move |_, _, x, y| {
                if let Some(area) = area_weak.upgrade() {
                    let w = area.width() as f64;
                    let h = area.height() as f64;
                    if w > 0.0 && h > 0.0 {
                        if let Some(ref cb) = state.borrow().on_click {
                            cb(x / w, y / h);
                        }
                    }
                }
            });
        }
        area.add_controller(gesture);

        Self { widget: area, state }
    }

    pub fn set_image(&self, path: &str) {
        if path.is_empty() {
            self.state.borrow_mut().image = None;
            self.widget.queue_draw();
            return;
        }
        match Pixbuf::from_file(path) {
            Ok(pb) => {
                self.state.borrow_mut().image = pixbuf_to_surface(&pb);
                self.widget.queue_draw();
            }
            Err(e) => log::warn!("Failed to load floor plan image {path}: {e}"),
        }
    }

    pub fn set_measurements(&self, measurements: Vec<Measurement>) {
        self.state.borrow_mut().measurements = measurements;
        self.widget.queue_draw();
    }

    pub fn set_show_heatmap(&self, show: bool) {
        self.state.borrow_mut().show_heatmap = show;
        self.widget.queue_draw();
    }

    pub fn set_on_click<F: Fn(f64, f64) + 'static>(&self, cb: F) {
        self.state.borrow_mut().on_click = Some(Box::new(cb));
    }
}

fn draw_floor_plan(state: &FloorPlanState, ctx: &Context, w: i32, h: i32) {
    let wf = w as f64;
    let hf = h as f64;

    ctx.set_source_rgb(0.15, 0.15, 0.15);
    let _ = ctx.paint();

    if let Some(ref surface) = state.image {
        let img_w = surface.width() as f64;
        let img_h = surface.height() as f64;
        let scale = (wf / img_w).min(hf / img_h);
        let ox = (wf - img_w * scale) / 2.0;
        let oy = (hf - img_h * scale) / 2.0;

        ctx.save().unwrap();
        ctx.translate(ox, oy);
        ctx.scale(scale, scale);
        ctx.set_source_surface(surface, 0.0, 0.0).unwrap();
        ctx.paint().unwrap();
        ctx.restore().unwrap();
    }

    if state.show_heatmap && !state.measurements.is_empty() {
        if let Some(hm) = HeatmapRenderer::render(&state.measurements, w, h, state.heatmap_alpha) {
            ctx.set_source_surface(&hm, 0.0, 0.0).unwrap();
            ctx.paint().unwrap();
        }
    }

    for m in &state.measurements {
        let px = m.x * wf;
        let py = m.y * hf;
        let (r, g, b) = signal_color(m.signal_dbm);

        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.6);
        ctx.arc(px, py, 8.0, 0.0, std::f64::consts::TAU);
        ctx.fill().unwrap();

        ctx.set_source_rgb(r, g, b);
        ctx.arc(px, py, 6.0, 0.0, std::f64::consts::TAU);
        ctx.fill().unwrap();
    }
}

/// Convert a gdk_pixbuf::Pixbuf to a cairo::ImageSurface (ARgb32 format).
fn pixbuf_to_surface(pixbuf: &Pixbuf) -> Option<ImageSurface> {
    let w = pixbuf.width() as usize;
    let h = pixbuf.height() as usize;
    let has_alpha = pixbuf.has_alpha();
    let n_ch = pixbuf.n_channels() as usize;
    let src_stride = pixbuf.rowstride() as usize;

    let bytes = pixbuf.read_pixel_bytes();
    let src: &[u8] = &bytes;

    let mut surface = ImageSurface::create(Format::ARgb32, w as i32, h as i32).ok()?;
    let dst_stride = surface.stride() as usize;
    {
        let mut data = surface.data().ok()?;
        for y in 0..h {
            for x in 0..w {
                let si = y * src_stride + x * n_ch;
                let di = y * dst_stride + x * 4;
                if si + n_ch <= src.len() && di + 3 < data.len() {
                    let r = src[si];
                    let g = src[si + 1];
                    let b = src[si + 2];
                    let a = if has_alpha && n_ch >= 4 { src[si + 3] } else { 255 };
                    // Cairo ARgb32 in memory (little-endian): B G R A
                    data[di]     = b;
                    data[di + 1] = g;
                    data[di + 2] = r;
                    data[di + 3] = a;
                }
            }
        }
    }
    Some(surface)
}

fn signal_color(dbm: i32) -> (f64, f64, f64) {
    let t = ((dbm + 90) as f64 / 60.0).clamp(0.0, 1.0);
    if t >= 0.5 {
        let s = (t - 0.5) * 2.0;
        (1.0 - s, 1.0, 0.0)
    } else {
        (1.0, t * 2.0, 0.0)
    }
}

