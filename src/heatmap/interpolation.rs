use cairo::{Format, ImageSurface};
use crate::models::Measurement;

pub struct HeatmapRenderer;

impl HeatmapRenderer {
    /// Render a heatmap overlay as a Cairo ImageSurface.
    /// width/height: dimensions of the floor plan in pixels.
    pub fn render(
        measurements: &[Measurement],
        width: i32,
        height: i32,
        alpha: f64,
    ) -> Option<ImageSurface> {
        if measurements.is_empty() {
            return None;
        }

        let mut surface = ImageSurface::create(Format::ARgb32, width, height).ok()?;
        let _ctx = cairo::Context::new(&surface).ok()?;

        // Render each pixel using IDW interpolation
        let stride = surface.stride() as usize;
        let mut data = surface.data().ok()?;

        for py in 0..height {
            for px in 0..width {
                let rx = px as f64 / width as f64;
                let ry = py as f64 / height as f64;

                let dbm = idw_interpolate(measurements, rx, ry, 2.0);
                let (r, g, b) = dbm_to_color(dbm);

                let offset = py as usize * stride + px as usize * 4;
                if offset + 3 < data.len() {
                    data[offset]     = (b * 255.0) as u8; // Blue
                    data[offset + 1] = (g * 255.0) as u8; // Green
                    data[offset + 2] = (r * 255.0) as u8; // Red
                    data[offset + 3] = (alpha * 255.0) as u8; // Alpha
                }
            }
        }
        drop(data);

        Some(surface)
    }
}

/// Inverse Distance Weighting interpolation
/// Returns interpolated dBm value at relative position (rx, ry)
fn idw_interpolate(measurements: &[Measurement], rx: f64, ry: f64, power: f64) -> f64 {
    let mut weighted_sum = 0.0f64;
    let mut weight_total = 0.0f64;

    for m in measurements {
        let dx = m.x - rx;
        let dy = m.y - ry;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist < 1e-9 {
            return m.signal_dbm as f64;
        }

        let weight = 1.0 / dist.powf(power);
        weighted_sum += weight * m.signal_dbm as f64;
        weight_total += weight;
    }

    if weight_total > 0.0 {
        weighted_sum / weight_total
    } else {
        -90.0
    }
}

/// Map dBm value to RGB color:
/// -30 dBm → green (excellent)
/// -60 dBm → yellow (fair)
/// -80 dBm → red (poor)
/// -90+ dBm → dark red (no signal)
fn dbm_to_color(dbm: f64) -> (f64, f64, f64) {
    let t = ((dbm + 90.0) / 60.0).clamp(0.0, 1.0); // 0.0 = bad, 1.0 = excellent

    if t >= 0.5 {
        // Yellow → Green
        let s = (t - 0.5) * 2.0;
        (1.0 - s, 1.0, 0.0)
    } else {
        // Dark red → Red → Yellow
        let s = t * 2.0;
        (1.0, s, 0.0)
    }
}
