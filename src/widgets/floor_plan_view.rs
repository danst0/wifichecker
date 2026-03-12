use gtk4::prelude::*;
use gtk4::{DrawingArea, GestureDrag};
use gdk_pixbuf::Pixbuf;
use cairo::{Context, ImageSurface, Format};
use std::cell::RefCell;
use std::rc::Rc;
use crate::models::Measurement;
use crate::heatmap::HeatmapRenderer;

#[derive(Debug, Clone, PartialEq)]
pub enum DrawMode {
    /// Click to place a WiFi measurement point
    Measure,
    /// Freehand drawing with a pen/brush
    Draw,
    /// Click two points to calibrate the scale
    Calibrate,
}

#[derive(Clone)]
pub struct FloorPlanView {
    pub widget: DrawingArea,
    state: Rc<RefCell<FloorPlanState>>,
}

struct FloorPlanState {
    // Display
    image: Option<ImageSurface>,
    measurements: Vec<Measurement>,
    show_heatmap: bool,
    heatmap_alpha: f64,

    // Draw mode
    mode: DrawMode,
    canvas: Option<ImageSurface>,
    last_draw_pos: Option<(f64, f64)>,
    stroke_color: (f64, f64, f64),
    stroke_width: f64,

    // Calibration
    calib_a: Option<(f64, f64)>,  // relative coords
    calib_b: Option<(f64, f64)>,
    scale_px_per_m: Option<f64>,

    // Grid
    show_grid: bool,
    grid_spacing_m: f64,

    // Callbacks
    on_measure_click: Option<Box<dyn Fn(f64, f64)>>,
    on_calibration_complete: Option<Box<dyn Fn(f64, f64, f64, f64)>>,
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
            mode: DrawMode::Measure,
            canvas: None,
            last_draw_pos: None,
            stroke_color: (0.0, 0.0, 0.0),
            stroke_width: 3.0,
            calib_a: None,
            calib_b: None,
            scale_px_per_m: None,
            show_grid: true,
            grid_spacing_m: 1.0,
            on_measure_click: None,
            on_calibration_complete: None,
        }));

        // Draw function
        {
            let state = state.clone();
            area.set_draw_func(move |_area, ctx, w, h| {
                draw_all(&state.borrow(), ctx, w, h);
            });
        }

        // Drag gesture handles both freehand drawing and click-based modes
        let drag = GestureDrag::new();
        {
            let state = state.clone();
            let area_weak = area.downgrade();
            drag.connect_drag_begin(move |_, x, y| {
                let Some(area) = area_weak.upgrade() else { return };
                let w = area.width() as f64;
                let h = area.height() as f64;
                if w <= 0.0 || h <= 0.0 { return; }

                let mut s = state.borrow_mut();
                match s.mode {
                    DrawMode::Draw => {
                        ensure_canvas(&mut s, area.width(), area.height());
                        s.last_draw_pos = Some((x, y));
                    }
                    DrawMode::Measure => {
                        let rx = x / w;
                        let ry = y / h;
                        if let Some(ref cb) = s.on_measure_click {
                            cb(rx, ry);
                        }
                    }
                    DrawMode::Calibrate => {
                        let rx = x / w;
                        let ry = y / h;
                        if s.calib_a.is_none() {
                            s.calib_a = Some((rx, ry));
                            s.calib_b = None;
                        } else if s.calib_b.is_none() {
                            s.calib_b = Some((rx, ry));
                            let a = s.calib_a.unwrap();
                            let b = (rx, ry);
                            let cb = s.on_calibration_complete.as_ref().map(|_f| {
                                // collect arguments for calling outside borrow
                                (a.0, a.1, b.0, b.1)
                            });
                            drop(s);
                            if let Some((ax, ay, bx, by)) = cb {
                                // caller will read these via get_calib_points()
                                // and show the dialog
                                let s2 = state.borrow();
                                if let Some(ref cb) = s2.on_calibration_complete {
                                    cb(ax, ay, bx, by);
                                }
                            }
                        } else {
                            // Reset and start again
                            state.borrow_mut().calib_a = Some((rx, ry));
                            state.borrow_mut().calib_b = None;
                        }
                    }
                }
                if let Some(area) = area_weak.upgrade() {
                    area.queue_draw();
                }
            });
        }

        {
            let state = state.clone();
            let area_weak = area.downgrade();
            drag.connect_drag_update(move |drag, dx, dy| {
                let Some(area) = area_weak.upgrade() else { return };
                let mut s = state.borrow_mut();
                if s.mode != DrawMode::Draw { return; }

                let start = drag.start_point().unwrap_or((0.0, 0.0));
                let current = (start.0 + dx, start.1 + dy);

                let Some(last) = s.last_draw_pos else {
                    s.last_draw_pos = Some(current);
                    return;
                };

                if let Some(ref canvas) = s.canvas {
                    if let Ok(ctx) = Context::new(canvas) {
                        let (r, g, b) = s.stroke_color;
                        ctx.set_source_rgb(r, g, b);
                        ctx.set_line_width(s.stroke_width);
                        ctx.set_line_cap(cairo::LineCap::Round);
                        ctx.set_line_join(cairo::LineJoin::Round);
                        ctx.move_to(last.0, last.1);
                        ctx.line_to(current.0, current.1);
                        let _ = ctx.stroke();
                    }
                }
                s.last_draw_pos = Some(current);
                drop(s);
                area.queue_draw();
            });
        }

        {
            let state = state.clone();
            drag.connect_drag_end(move |_, _, _| {
                state.borrow_mut().last_draw_pos = None;
            });
        }

        area.add_controller(drag);

        Self { widget: area, state }
    }

    // ── Image & measurements ───────────────────────────────────────────────

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

    // ── Draw mode ──────────────────────────────────────────────────────────

    pub fn set_draw_mode(&self, mode: DrawMode) {
        self.state.borrow_mut().mode = mode;
    }

    pub fn set_stroke_color(&self, r: f64, g: f64, b: f64) {
        self.state.borrow_mut().stroke_color = (r, g, b);
    }

    pub fn set_stroke_width(&self, w: f64) {
        self.state.borrow_mut().stroke_width = w;
    }

    pub fn clear_canvas(&self) {
        let mut s = self.state.borrow_mut();
        s.canvas = None;
        drop(s);
        self.widget.queue_draw();
    }

    /// Save the drawing canvas as a PNG file. Returns `None` if nothing was drawn.
    pub fn save_canvas(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let s = self.state.borrow();
        let Some(ref canvas) = s.canvas else {
            anyhow::bail!("No canvas to save");
        };
        let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
        canvas.write_to_png(&mut file)?;
        Ok(())
    }

    /// Load a drawing canvas from a PNG file.
    pub fn load_canvas(&self, path: &std::path::Path) {
        match std::fs::File::open(path) {
            Ok(f) => {
                let mut reader = std::io::BufReader::new(f);
                match ImageSurface::create_from_png(&mut reader) {
                    Ok(surface) => {
                        self.state.borrow_mut().canvas = Some(surface);
                        self.widget.queue_draw();
                    }
                    Err(e) => log::warn!("Failed to load canvas PNG {}: {e}", path.display()),
                }
            }
            Err(e) => log::warn!("Cannot open canvas PNG {}: {e}", path.display()),
        }
    }

    // ── Calibration ────────────────────────────────────────────────────────

    pub fn set_scale(&self, px_per_m: f64, a: (f64, f64), b: (f64, f64)) {
        let mut s = self.state.borrow_mut();
        s.scale_px_per_m = Some(px_per_m);
        s.calib_a = Some(a);
        s.calib_b = Some(b);
        drop(s);
        self.widget.queue_draw();
    }

    pub fn get_calib_points(&self) -> (Option<(f64, f64)>, Option<(f64, f64)>) {
        let s = self.state.borrow();
        (s.calib_a, s.calib_b)
    }

    // ── Grid ───────────────────────────────────────────────────────────────

    pub fn set_show_grid(&self, show: bool) {
        self.state.borrow_mut().show_grid = show;
        self.widget.queue_draw();
    }

    pub fn set_grid_spacing(&self, spacing_m: f64) {
        self.state.borrow_mut().grid_spacing_m = spacing_m;
        self.widget.queue_draw();
    }

    pub fn set_scale_px_per_m(&self, px_per_m: Option<f64>) {
        self.state.borrow_mut().scale_px_per_m = px_per_m;
        self.widget.queue_draw();
    }

    // ── Callbacks ──────────────────────────────────────────────────────────

    pub fn set_on_measure_click<F: Fn(f64, f64) + 'static>(&self, cb: F) {
        self.state.borrow_mut().on_measure_click = Some(Box::new(cb));
    }

    pub fn set_on_calibration_complete<F: Fn(f64, f64, f64, f64) + 'static>(&self, cb: F) {
        self.state.borrow_mut().on_calibration_complete = Some(Box::new(cb));
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn draw_all(state: &FloorPlanState, ctx: &Context, w: i32, h: i32) {
    let wf = w as f64;
    let hf = h as f64;

    // 1. Background
    ctx.set_source_rgb(0.12, 0.12, 0.12);
    let _ = ctx.paint();

    // 2. Grid (behind the floor plan image)
    if state.show_grid {
        draw_grid(ctx, wf, hf, state.scale_px_per_m, state.grid_spacing_m);
    }

    // 3. Floor plan image
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

    // 4. Freehand drawing canvas
    if let Some(ref canvas) = state.canvas {
        ctx.set_source_surface(canvas, 0.0, 0.0).unwrap();
        ctx.paint().unwrap();
    }

    // 5. Heatmap overlay
    if state.show_heatmap && !state.measurements.is_empty() {
        if let Some(hm) = HeatmapRenderer::render(&state.measurements, w, h, state.heatmap_alpha) {
            ctx.set_source_surface(&hm, 0.0, 0.0).unwrap();
            ctx.paint().unwrap();
        }
    }

    // 6. Measurement dots
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

    // 7. Calibration visualization
    draw_calibration(ctx, wf, hf, state);
}

fn draw_grid(ctx: &Context, w: f64, h: f64, scale_px_per_m: Option<f64>, spacing_m: f64) {
    let px_step = match scale_px_per_m {
        Some(s) if s > 0.0 => s * spacing_m,
        _ => 60.0, // default: 60px per cell when uncalibrated
    };
    if px_step < 8.0 { return; }

    ctx.save().unwrap();
    ctx.set_source_rgba(0.5, 0.7, 1.0, 0.18);
    ctx.set_line_width(0.5);

    let mut x = 0.0f64;
    while x <= w {
        ctx.move_to(x, 0.0);
        ctx.line_to(x, h);
        let _ = ctx.stroke();
        x += px_step;
    }
    let mut y = 0.0f64;
    while y <= h {
        ctx.move_to(0.0, y);
        ctx.line_to(w, y);
        let _ = ctx.stroke();
        y += px_step;
    }

    // Axis labels
    ctx.set_source_rgba(0.7, 0.9, 1.0, 0.55);
    ctx.set_font_size(10.0);
    let mut col = 0u32;
    let mut x = 0.0f64;
    while x <= w {
        if col > 0 {
            let label = format!("{} m", col as f64 * spacing_m);
            ctx.move_to(x + 2.0, 12.0);
            let _ = ctx.show_text(&label);
        }
        col += 1;
        x += px_step;
    }
    let mut row = 0u32;
    let mut y = 0.0f64;
    while y <= h {
        if row > 0 {
            let label = format!("{} m", row as f64 * spacing_m);
            ctx.move_to(2.0, y - 2.0);
            let _ = ctx.show_text(&label);
        }
        row += 1;
        y += px_step;
    }

    ctx.restore().unwrap();
}

fn draw_calibration(ctx: &Context, w: f64, h: f64, state: &FloorPlanState) {
    if state.mode != DrawMode::Calibrate
        && state.calib_a.is_none()
        && state.calib_b.is_none()
    {
        return;
    }

    if let Some((ax, ay)) = state.calib_a {
        let px = ax * w;
        let py = ay * h;
        ctx.set_source_rgba(1.0, 0.2, 0.2, 0.9);
        ctx.arc(px, py, 6.0, 0.0, std::f64::consts::TAU);
        ctx.fill().unwrap();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.8);
        ctx.set_font_size(11.0);
        ctx.move_to(px + 8.0, py - 4.0);
        let _ = ctx.show_text("A");
    }

    if let Some((bx, by)) = state.calib_b {
        let px = bx * w;
        let py = by * h;
        ctx.set_source_rgba(0.2, 0.5, 1.0, 0.9);
        ctx.arc(px, py, 6.0, 0.0, std::f64::consts::TAU);
        ctx.fill().unwrap();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.8);
        ctx.set_font_size(11.0);
        ctx.move_to(px + 8.0, py - 4.0);
        let _ = ctx.show_text("B");
    }

    // Line between A and B
    if let (Some((ax, ay)), Some((bx, by))) = (state.calib_a, state.calib_b) {
        ctx.set_source_rgba(1.0, 0.9, 0.0, 0.7);
        ctx.set_line_width(1.5);
        ctx.set_dash(&[4.0, 4.0], 0.0);
        ctx.move_to(ax * w, ay * h);
        ctx.line_to(bx * w, by * h);
        let _ = ctx.stroke();
        ctx.set_dash(&[], 0.0);

        // Distance label
        if let Some(scale) = state.scale_px_per_m {
            let dx = (bx - ax) * w;
            let dy = (by - ay) * h;
            let px_dist = (dx * dx + dy * dy).sqrt();
            let meters = px_dist / scale;
            let label = format!("{:.1} m", meters);
            let mx = (ax + bx) / 2.0 * w;
            let my = (ay + by) / 2.0 * h - 6.0;
            ctx.set_source_rgba(1.0, 0.9, 0.0, 0.9);
            ctx.set_font_size(11.0);
            ctx.move_to(mx, my);
            let _ = ctx.show_text(&label);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn ensure_canvas(state: &mut FloorPlanState, w: i32, h: i32) {
    if state.canvas.is_none() {
        if let Ok(surface) = ImageSurface::create(Format::ARgb32, w, h) {
            state.canvas = Some(surface);
        }
    }
}

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
    if t >= 0.5 { (1.0 - (t - 0.5) * 2.0, 1.0, 0.0) } else { (1.0, t * 2.0, 0.0) }
}
