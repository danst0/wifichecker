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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_measurement_at(x: f64, y: f64, signal_dbm: i32) -> Measurement {
        Measurement::new(x, y, "SSID".to_string(), "AA:BB:CC:DD:EE:FF".to_string(), 2412, 1, signal_dbm)
    }

    // --- dbm_to_color ---

    #[test]
    fn test_dbm_to_color_excellent() {
        // -30 dBm → t = 1.0 → green: (0.0, 1.0, 0.0)
        let (r, g, b) = dbm_to_color(-30.0);
        assert!((r - 0.0).abs() < 1e-9);
        assert!((g - 1.0).abs() < 1e-9);
        assert!((b - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_dbm_to_color_no_signal() {
        // -90 dBm → t = 0.0 → dark red: (1.0, 0.0, 0.0)
        let (r, g, b) = dbm_to_color(-90.0);
        assert!((r - 1.0).abs() < 1e-9);
        assert!((g - 0.0).abs() < 1e-9);
        assert!((b - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_dbm_to_color_fair() {
        // -60 dBm → t = 0.5 → yellow: (1.0, 1.0, 0.0)
        let (r, g, b) = dbm_to_color(-60.0);
        assert!((r - 1.0).abs() < 1e-9);
        assert!((g - 1.0).abs() < 1e-9);
        assert!((b - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_dbm_to_color_clamped_above() {
        // Better than -30 dBm → same as -30
        let (r1, g1, b1) = dbm_to_color(-30.0);
        let (r2, g2, b2) = dbm_to_color(-10.0);
        assert!((r1 - r2).abs() < 1e-9);
        assert!((g1 - g2).abs() < 1e-9);
        assert!((b1 - b2).abs() < 1e-9);
    }

    #[test]
    fn test_dbm_to_color_clamped_below() {
        // Worse than -90 dBm → same as -90
        let (r1, g1, b1) = dbm_to_color(-90.0);
        let (r2, g2, b2) = dbm_to_color(-120.0);
        assert!((r1 - r2).abs() < 1e-9);
        assert!((g1 - g2).abs() < 1e-9);
        assert!((b1 - b2).abs() < 1e-9);
    }

    #[test]
    fn test_dbm_to_color_blue_channel_is_always_zero() {
        for dbm in [-90.0, -75.0, -60.0, -45.0, -30.0] {
            let (_, _, b) = dbm_to_color(dbm);
            assert!((b - 0.0).abs() < 1e-9, "blue should be 0 for {dbm} dBm");
        }
    }

    // --- idw_interpolate ---

    #[test]
    fn test_idw_exact_hit_returns_measurement_value() {
        let measurements = vec![make_measurement_at(0.5, 0.5, -60)];
        let result = idw_interpolate(&measurements, 0.5, 0.5, 2.0);
        assert!((result - (-60.0)).abs() < 1e-6);
    }

    #[test]
    fn test_idw_equal_measurements_midpoint() {
        // Two points with identical signal; midpoint should return the same value
        let measurements = vec![
            make_measurement_at(0.0, 0.5, -60),
            make_measurement_at(1.0, 0.5, -60),
        ];
        let result = idw_interpolate(&measurements, 0.5, 0.5, 2.0);
        assert!((result - (-60.0)).abs() < 1e-6);
    }

    #[test]
    fn test_idw_closer_measurement_dominates() {
        // Point near -50 measurement, far from -80
        let measurements = vec![
            make_measurement_at(0.1, 0.5, -50),
            make_measurement_at(0.9, 0.5, -80),
        ];
        let result = idw_interpolate(&measurements, 0.1, 0.5, 2.0);
        // At exact location of first measurement → returns -50
        assert!((result - (-50.0)).abs() < 1e-6);
    }

    #[test]
    fn test_idw_empty_returns_fallback() {
        let result = idw_interpolate(&[], 0.5, 0.5, 2.0);
        assert!((result - (-90.0)).abs() < 1e-6);
    }

    #[test]
    fn test_idw_single_measurement_any_point() {
        let measurements = vec![make_measurement_at(0.3, 0.7, -55)];
        // Far from measurement — should still pull toward -55 (only one data point)
        let result = idw_interpolate(&measurements, 0.9, 0.1, 2.0);
        assert!((result - (-55.0)).abs() < 1e-6);
    }
}
