use gtk4::prelude::*;
use gtk4::{DrawingArea, EventControllerMotion, EventControllerScroll, GestureDrag};
use gdk_pixbuf::Pixbuf;
use cairo::{Context, ImageSurface, Format};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use crate::models::Measurement;

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
    /// Snapped grid corner where the current drag started (used in snap mode)
    draw_start_pos: Option<(f64, f64)>,
    /// In-progress segment to preview before committing to canvas (snap mode)
    preview_line: Option<((f64, f64), (f64, f64))>,

    // Calibration
    calib_a: Option<(f64, f64)>,  // relative coords
    calib_b: Option<(f64, f64)>,
    scale_px_per_m: Option<f64>,

    // Grid
    show_grid: bool,
    grid_spacing_m: f64,             // visual grid line spacing
    measurement_grid_spacing_m: f64, // cell size for snapping & coloring
    snap_to_grid: bool,

    // Zoom / pan
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    /// Stored pan at the start of a middle-button pan drag.
    pan_origin: Option<(f64, f64)>,
    /// Last known mouse position in widget space (used for scroll-wheel zoom centre).
    last_mouse_pos: Option<(f64, f64)>,

    // Hover / tooltip
    hover_pos: Option<(f64, f64)>,
    hover_cell: Option<(i64, i64)>,
    tooltip_visible: bool,
    hover_generation: u32,

    // Callbacks
    on_measure_click: Option<Box<dyn Fn(f64, f64)>>,
    on_calibration_complete: Option<Box<dyn Fn(f64, f64, f64, f64)>>,
    on_draw_complete: Option<Box<dyn Fn()>>,
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
            draw_start_pos: None,
            preview_line: None,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            pan_origin: None,
            last_mouse_pos: None,
            calib_a: None,
            calib_b: None,
            scale_px_per_m: None,
            show_grid: true,
            grid_spacing_m: 1.0,
            measurement_grid_spacing_m: 1.0,
            snap_to_grid: false,
            hover_pos: None,
            hover_cell: None,
            tooltip_visible: false,
            hover_generation: 0,
            on_measure_click: None,
            on_calibration_complete: None,
            on_draw_complete: None,
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
                        // Always snap to the visual grid corners
                        let (cx, cy) = widget_to_canvas(&s, x, y);
                        let px_step = grid_px_step(s.scale_px_per_m, s.grid_spacing_m);
                        let snap_x = (cx / px_step).round() * px_step;
                        let snap_y = (cy / px_step).round() * px_step;
                        s.draw_start_pos = Some((snap_x, snap_y));
                        s.last_draw_pos = None;
                    }
                    DrawMode::Measure => {
                        let (cx, cy) = widget_to_canvas(&s, x, y);
                        let mut rx = cx / w;
                        let mut ry = cy / h;
                        if s.snap_to_grid {
                            let px_step = grid_px_step(s.scale_px_per_m, s.measurement_grid_spacing_m);
                            rx = ((cx / px_step).floor() + 0.5) * px_step / w;
                            ry = ((cy / px_step).floor() + 0.5) * px_step / h;
                        }
                        if let Some(ref cb) = s.on_measure_click {
                            cb(rx, ry);
                        }
                    }
                    DrawMode::Calibrate => {
                        let (cx, cy) = widget_to_canvas(&s, x, y);
                        let rx = cx / w;
                        let ry = cy / h;
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
                let current_w = (start.0 + dx, start.1 + dy);
                let current = widget_to_canvas(&s, current_w.0, current_w.1);

                // Always snap to the visual grid and constrain to horizontal/vertical
                if let Some(seg_start) = s.draw_start_pos {
                    let px_step = grid_px_step(s.scale_px_per_m, s.grid_spacing_m);
                    let snap_x = (current.0 / px_step).round() * px_step;
                    let snap_y = (current.1 / px_step).round() * px_step;
                    let end = if (snap_x - seg_start.0).abs() >= (snap_y - seg_start.1).abs() {
                        (snap_x, seg_start.1) // horizontal
                    } else {
                        (seg_start.0, snap_y) // vertical
                    };
                    s.preview_line = Some((seg_start, end));
                    drop(s);
                    area.queue_draw();
                }
            });
        }

        {
            let state = state.clone();
            let area_weak = area.downgrade();
            drag.connect_drag_end(move |_, _, _| {
                let mut s = state.borrow_mut();
                if s.mode == DrawMode::Draw {
                    // Commit the previewed grid-snapped segment to the canvas
                    if let Some((a, b)) = s.preview_line.take() {
                        if let Some(ref canvas) = s.canvas {
                            if let Ok(ctx) = Context::new(canvas) {
                                let (r, g, b_ch) = s.stroke_color;
                                ctx.set_source_rgb(r, g, b_ch);
                                ctx.set_line_width(3.0);
                                ctx.set_line_cap(cairo::LineCap::Round);
                                ctx.move_to(a.0, a.1);
                                ctx.line_to(b.0, b.1);
                                let _ = ctx.stroke();
                            }
                        }
                    }
                    s.draw_start_pos = None;
                    let should_notify = s.on_draw_complete.is_some();
                    drop(s);
                    if should_notify {
                        let s2 = state.borrow();
                        if let Some(ref cb) = s2.on_draw_complete { cb(); }
                    }
                    if let Some(area) = area_weak.upgrade() { area.queue_draw(); }
                } else {
                    s.last_draw_pos = None;
                }
            });
        }

        area.add_controller(drag);

        // Motion controller for hover highlight and tooltip
        let motion = EventControllerMotion::new();
        {
            let state = state.clone();
            let area_weak = area.downgrade();
            motion.connect_motion(move |_, x, y| {
                let Some(area) = area_weak.upgrade() else { return };
                let w = area.width() as f64;
                let h = area.height() as f64;
                if w <= 0.0 || h <= 0.0 { return; }

                let mut s = state.borrow_mut();
                s.last_mouse_pos = Some((x, y));
                if s.mode != DrawMode::Measure {
                    drop(s);
                    area.queue_draw();
                    return;
                }

                let (cx, cy) = widget_to_canvas(&s, x, y);
                s.hover_pos = Some((cx, cy));

                let px_step = grid_px_step(s.scale_px_per_m, s.measurement_grid_spacing_m);
                let new_cell = ((cx / px_step).floor() as i64, (cy / px_step).floor() as i64);

                if s.hover_cell != Some(new_cell) {
                    s.hover_cell = Some(new_cell);
                    s.tooltip_visible = false;
                    s.hover_generation = s.hover_generation.wrapping_add(1);
                    let expected_gen = s.hover_generation;
                    drop(s);

                    let state_show = state.clone();
                    let area_show = area_weak.clone();
                    gtk4::glib::timeout_add_local_once(Duration::from_secs(7), move || {
                        let mut s = state_show.borrow_mut();
                        if s.hover_generation != expected_gen { return; }
                        s.tooltip_visible = true;
                        drop(s);
                        if let Some(a) = area_show.upgrade() { a.queue_draw(); }

                        let state_hide = state_show.clone();
                        let area_hide = area_show.clone();
                        gtk4::glib::timeout_add_local_once(Duration::from_secs(5), move || {
                            let mut s = state_hide.borrow_mut();
                            if s.hover_generation != expected_gen { return; }
                            s.tooltip_visible = false;
                            drop(s);
                            if let Some(a) = area_hide.upgrade() { a.queue_draw(); }
                        });
                    });
                } else {
                    drop(s);
                }
                area.queue_draw();
            });
        }
        {
            let state = state.clone();
            let area_weak = area.downgrade();
            motion.connect_leave(move |_| {
                let mut s = state.borrow_mut();
                s.hover_pos = None;
                s.hover_cell = None;
                s.tooltip_visible = false;
                s.hover_generation = s.hover_generation.wrapping_add(1);
                drop(s);
                if let Some(a) = area_weak.upgrade() { a.queue_draw(); }
            });
        }
        area.add_controller(motion);

        // Scroll-wheel zoom
        let scroll = EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
        {
            let state = state.clone();
            let area_weak = area.downgrade();
            scroll.connect_scroll(move |_, _dx, dy| {
                let Some(area) = area_weak.upgrade() else {
                    return gtk4::glib::Propagation::Proceed;
                };
                let factor = if dy < 0.0 { 1.1 } else { 1.0 / 1.1 };
                let mut s = state.borrow_mut();
                let (cx, cy) = s.last_mouse_pos.unwrap_or_else(|| {
                    (area.width() as f64 / 2.0, area.height() as f64 / 2.0)
                });
                apply_zoom(&mut s, factor, cx, cy);
                drop(s);
                area.queue_draw();
                gtk4::glib::Propagation::Proceed
            });
        }
        area.add_controller(scroll);

        // Middle-button drag to pan the view
        let pan_drag = GestureDrag::new();
        pan_drag.set_button(2); // middle mouse button
        {
            let state = state.clone();
            pan_drag.connect_drag_begin(move |_, _x, _y| {
                let mut s = state.borrow_mut();
                s.pan_origin = Some((s.pan_x, s.pan_y));
            });
        }
        {
            let state = state.clone();
            let area_weak = area.downgrade();
            pan_drag.connect_drag_update(move |_, dx, dy| {
                let Some(area) = area_weak.upgrade() else { return };
                let mut s = state.borrow_mut();
                if let Some((ox, oy)) = s.pan_origin {
                    s.pan_x = ox + dx;
                    s.pan_y = oy + dy;
                    drop(s);
                    area.queue_draw();
                }
            });
        }
        {
            let state = state.clone();
            pan_drag.connect_drag_end(move |_, _, _| {
                state.borrow_mut().pan_origin = None;
            });
        }
        area.add_controller(pan_drag);

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

    pub fn set_snap_to_grid(&self, snap: bool) {
        self.state.borrow_mut().snap_to_grid = snap;
    }

    pub fn set_measurement_grid_spacing(&self, spacing_m: f64) {
        self.state.borrow_mut().measurement_grid_spacing_m = spacing_m;
        self.widget.queue_draw();
    }

    // ── Callbacks ──────────────────────────────────────────────────────────

    pub fn set_on_measure_click<F: Fn(f64, f64) + 'static>(&self, cb: F) {
        self.state.borrow_mut().on_measure_click = Some(Box::new(cb));
    }

    pub fn set_on_calibration_complete<F: Fn(f64, f64, f64, f64) + 'static>(&self, cb: F) {
        self.state.borrow_mut().on_calibration_complete = Some(Box::new(cb));
    }

    pub fn set_on_draw_complete<F: Fn() + 'static>(&self, cb: F) {
        self.state.borrow_mut().on_draw_complete = Some(Box::new(cb));
    }

    // ── Zoom ───────────────────────────────────────────────────────────────

    /// Zoom in (1.25×), centred on the widget centre.
    pub fn zoom_in(&self) {
        let cx = self.widget.width()  as f64 / 2.0;
        let cy = self.widget.height() as f64 / 2.0;
        let mut s = self.state.borrow_mut();
        apply_zoom(&mut s, 1.25, cx, cy);
        self.widget.queue_draw();
    }

    /// Zoom out (0.8×), centred on the widget centre.
    pub fn zoom_out(&self) {
        let cx = self.widget.width()  as f64 / 2.0;
        let cy = self.widget.height() as f64 / 2.0;
        let mut s = self.state.borrow_mut();
        apply_zoom(&mut s, 1.0 / 1.25, cx, cy);
        self.widget.queue_draw();
    }

    /// Reset to 1:1 zoom with no pan offset.
    pub fn reset_zoom(&self) {
        let mut s = self.state.borrow_mut();
        s.zoom  = 1.0;
        s.pan_x = 0.0;
        s.pan_y = 0.0;
        self.widget.queue_draw();
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Convert a widget-space point to canvas (unzoomed) space.
fn widget_to_canvas(s: &FloorPlanState, wx: f64, wy: f64) -> (f64, f64) {
    ((wx - s.pan_x) / s.zoom, (wy - s.pan_y) / s.zoom)
}

/// Zoom by `factor` keeping the screen point `(cx, cy)` stationary.
fn apply_zoom(s: &mut FloorPlanState, factor: f64, cx: f64, cy: f64) {
    let new_zoom = (s.zoom * factor).clamp(0.1, 10.0);
    let ratio = new_zoom / s.zoom;
    s.pan_x = cx + (s.pan_x - cx) * ratio;
    s.pan_y = cy + (s.pan_y - cy) * ratio;
    s.zoom  = new_zoom;
}

fn draw_all(state: &FloorPlanState, ctx: &Context, w: i32, h: i32) {
    let wf = w as f64;
    let hf = h as f64;

    // 1. Background — painted outside the zoom transform so it always fills the widget.
    ctx.set_source_rgb(0.12, 0.12, 0.12);
    let _ = ctx.paint();

    // Apply zoom/pan transform for all content layers.
    ctx.save().unwrap();
    ctx.translate(state.pan_x, state.pan_y);
    ctx.scale(state.zoom, state.zoom);

    // 2. Grid (behind the floor plan image)
    if state.show_grid {
        // Compute the visible canvas-space rectangle so the grid extends to fill it.
        let vx0 = -state.pan_x / state.zoom;
        let vy0 = -state.pan_y / state.zoom;
        let vx1 = (wf - state.pan_x) / state.zoom;
        let vy1 = (hf - state.pan_y) / state.zoom;
        draw_grid(ctx, vx0, vy0, vx1, vy1, state.scale_px_per_m, state.grid_spacing_m);
    }

    // 2.5 Hover cell highlight (Measure mode only, uses measurement grid)
    if state.mode == DrawMode::Measure {
        if let Some((px, py)) = state.hover_pos {
            draw_hover_highlight(ctx, px, py, state.scale_px_per_m, state.measurement_grid_spacing_m);
        }
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

    // 4.5 Preview line for snapped segment (shown while dragging before commit)
    if let Some(((ax, ay), (bx, by))) = state.preview_line {
        let (r, g, b) = state.stroke_color;
        ctx.set_source_rgba(r, g, b, 0.6);
        ctx.set_line_width(3.0);
        ctx.set_line_cap(cairo::LineCap::Round);
        ctx.set_dash(&[6.0, 4.0], 0.0);
        ctx.move_to(ax, ay);
        ctx.line_to(bx, by);
        let _ = ctx.stroke();
        ctx.set_dash(&[], 0.0);
    }

    // 5. Measurement cell coloring (replaces heatmap and individual dots)
    if state.show_heatmap && !state.measurements.is_empty() {
        draw_measurement_cells(ctx, wf, hf, &state.measurements, state.scale_px_per_m, state.measurement_grid_spacing_m);
    }

    // 6. Calibration visualization
    draw_calibration(ctx, wf, hf, state);

    // 7. Hover tooltip
    if state.tooltip_visible && state.mode == DrawMode::Measure {
        if let Some((px, py)) = state.hover_pos {
            draw_tooltip(ctx, px, py, wf, hf, "Click to take measurement");
        }
    }

    // End of zoom/pan transform
    ctx.restore().unwrap();
}

fn draw_grid(ctx: &Context, x0: f64, y0: f64, x1: f64, y1: f64, scale_px_per_m: Option<f64>, spacing_m: f64) {
    let px_step = grid_px_step(scale_px_per_m, spacing_m);
    if px_step < 8.0 { return; }

    ctx.save().unwrap();
    ctx.set_source_rgba(0.5, 0.7, 1.0, 0.18);
    ctx.set_line_width(0.5);

    // Start at the first grid line at or before the visible left/top edge.
    let first_x = (x0 / px_step).floor() * px_step;
    let mut x = first_x;
    while x <= x1 {
        ctx.move_to(x, y0);
        ctx.line_to(x, y1);
        let _ = ctx.stroke();
        x += px_step;
    }
    let first_y = (y0 / px_step).floor() * px_step;
    let mut y = first_y;
    while y <= y1 {
        ctx.move_to(x0, y);
        ctx.line_to(x1, y);
        let _ = ctx.stroke();
        y += px_step;
    }

    // Axis labels
    ctx.set_source_rgba(0.7, 0.9, 1.0, 0.55);
    ctx.set_font_size(10.0);
    let mut x = first_x;
    while x <= x1 {
        let col = (x / px_step).round() as i64;
        if col != 0 {
            let label = format!("{} m", col as f64 * spacing_m);
            ctx.move_to(x + 2.0, y0 + 12.0);
            let _ = ctx.show_text(&label);
        }
        x += px_step;
    }
    let mut y = first_y;
    while y <= y1 {
        let row = (y / px_step).round() as i64;
        if row != 0 {
            let label = format!("{} m", row as f64 * spacing_m);
            ctx.move_to(x0 + 2.0, y - 2.0);
            let _ = ctx.show_text(&label);
        }
        y += px_step;
    }

    ctx.restore().unwrap();
}

fn grid_px_step(scale_px_per_m: Option<f64>, spacing_m: f64) -> f64 {
    match scale_px_per_m {
        Some(s) if s > 0.0 => (s * spacing_m).max(8.0),
        _ => (60.0 * spacing_m).max(8.0), // 60 px = 1 m reference when uncalibrated
    }
}

fn draw_measurement_cells(
    ctx: &Context,
    w: f64,
    h: f64,
    measurements: &[Measurement],
    scale_px_per_m: Option<f64>,
    spacing_m: f64,
) {
    let px_step = grid_px_step(scale_px_per_m, spacing_m);
    ctx.save().unwrap();
    for m in measurements {
        let px = m.x * w;
        let py = m.y * h;
        // The measurement is stored at cell center; find cell origin
        let cell_x = (px / px_step).floor() * px_step;
        let cell_y = (py / px_step).floor() * px_step;
        let (r, g, b) = signal_color(m.signal_dbm);
        ctx.set_source_rgba(r, g, b, 0.72);
        ctx.rectangle(cell_x, cell_y, px_step, px_step);
        ctx.fill().unwrap();
        // Subtle darker border so adjacent cells are distinguishable
        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.25);
        ctx.set_line_width(0.8);
        ctx.rectangle(cell_x, cell_y, px_step, px_step);
        ctx.stroke().unwrap();
    }
    ctx.restore().unwrap();
}

fn draw_hover_highlight(ctx: &Context, px: f64, py: f64, scale_px_per_m: Option<f64>, spacing_m: f64) {
    let px_step = grid_px_step(scale_px_per_m, spacing_m);

    // Highlighted cell (floor to nearest grid line)
    let cell_x = (px / px_step).floor() * px_step;
    let cell_y = (py / px_step).floor() * px_step;

    ctx.save().unwrap();
    ctx.set_source_rgba(0.4, 0.7, 1.0, 0.22);
    ctx.rectangle(cell_x, cell_y, px_step, px_step);
    ctx.fill().unwrap();

    // Bright border around the cell
    ctx.set_source_rgba(0.4, 0.8, 1.0, 0.55);
    ctx.set_line_width(1.0);
    ctx.rectangle(cell_x, cell_y, px_step, px_step);
    ctx.stroke().unwrap();

    // Snap intersection dot (nearest grid corner)
    let snap_x = (px / px_step).round() * px_step;
    let snap_y = (py / px_step).round() * px_step;
    ctx.set_source_rgba(0.5, 0.9, 1.0, 0.9);
    ctx.arc(snap_x, snap_y, 4.5, 0.0, std::f64::consts::TAU);
    ctx.fill().unwrap();

    ctx.restore().unwrap();
}

fn draw_tooltip(ctx: &Context, px: f64, py: f64, w: f64, h: f64, text: &str) {
    const PADDING: f64 = 8.0;
    const FONT_SIZE: f64 = 12.0;
    const CORNER_R: f64 = 5.0;

    ctx.save().unwrap();
    ctx.set_font_size(FONT_SIZE);

    let extents = match ctx.text_extents(text) {
        Ok(e) => e,
        Err(_) => { ctx.restore().unwrap(); return; }
    };
    let box_w = extents.width() + PADDING * 2.0;
    let box_h = FONT_SIZE + PADDING * 2.0;

    // Position above the cursor, clamped to widget bounds
    let mut tx = px - box_w / 2.0;
    let mut ty = py - box_h - 14.0;
    tx = tx.clamp(4.0, w - box_w - 4.0);
    ty = ty.clamp(4.0, h - box_h - 4.0);

    // Rounded rectangle background
    ctx.new_sub_path();
    ctx.arc(tx + CORNER_R,         ty + CORNER_R,         CORNER_R, std::f64::consts::PI, 3.0 * std::f64::consts::PI / 2.0);
    ctx.arc(tx + box_w - CORNER_R, ty + CORNER_R,         CORNER_R, 3.0 * std::f64::consts::PI / 2.0, 0.0);
    ctx.arc(tx + box_w - CORNER_R, ty + box_h - CORNER_R, CORNER_R, 0.0, std::f64::consts::PI / 2.0);
    ctx.arc(tx + CORNER_R,         ty + box_h - CORNER_R, CORNER_R, std::f64::consts::PI / 2.0, std::f64::consts::PI);
    ctx.close_path();
    ctx.set_source_rgba(0.1, 0.1, 0.1, 0.88);
    ctx.fill().unwrap();

    // Text
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    ctx.move_to(tx + PADDING, ty + PADDING + FONT_SIZE * 0.85);
    let _ = ctx.show_text(text);

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
