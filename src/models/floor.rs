use serde::{Deserialize, Serialize};
use crate::models::Measurement;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Floor {
    pub name: String,
    /// Path to the imported floor plan image (PNG/JPG)
    pub image_path: Option<String>,
    /// Path to the freehand drawing canvas PNG
    #[serde(default)]
    pub drawing_path: Option<String>,
    pub measurements: Vec<Measurement>,
    /// Calibrated scale: pixels per meter (relative to image dimensions)
    #[serde(default)]
    pub scale_px_per_m: Option<f64>,
    /// Calibration point A (relative coords 0..1)
    #[serde(default)]
    pub calib_point_a: Option<(f64, f64)>,
    /// Calibration point B (relative coords 0..1)
    #[serde(default)]
    pub calib_point_b: Option<(f64, f64)>,
    /// Selected PDF page (0-based); only relevant when image_path points to a .pdf file
    #[serde(default)]
    pub pdf_page: Option<u32>,
    /// Origin (zero point) for the grid/axis overlay, in relative image coords (0..1)
    #[serde(default)]
    pub origin: Option<(f64, f64)>,
}

impl Floor {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            image_path: None,
            drawing_path: None,
            measurements: Vec::new(),
            scale_px_per_m: None,
            calib_point_a: None,
            calib_point_b: None,
            pdf_page: None,
            origin: None,
        }
    }

    pub fn add_measurement(&mut self, measurement: Measurement) {
        self.measurements.push(measurement);
    }

    pub fn remove_measurement(&mut self, id: &str) {
        self.measurements.retain(|m| m.id != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_measurement(id: &str) -> Measurement {
        let mut m = Measurement::new(0.5, 0.5, "SSID".to_string(), "AA:BB:CC:DD:EE:FF".to_string(), 2412, 1, -60);
        m.id = id.to_string();
        m
    }

    #[test]
    fn test_floor_new() {
        let f = Floor::new("Ground Floor");
        assert_eq!(f.name, "Ground Floor");
        assert!(f.measurements.is_empty());
        assert!(f.image_path.is_none());
        assert!(f.drawing_path.is_none());
        assert!(f.scale_px_per_m.is_none());
        assert!(f.calib_point_a.is_none());
        assert!(f.calib_point_b.is_none());
        assert!(f.pdf_page.is_none());
        assert!(f.origin.is_none());
    }

    #[test]
    fn test_add_measurement() {
        let mut f = Floor::new("Test");
        assert_eq!(f.measurements.len(), 0);
        f.add_measurement(make_measurement("id-1"));
        assert_eq!(f.measurements.len(), 1);
        f.add_measurement(make_measurement("id-2"));
        assert_eq!(f.measurements.len(), 2);
    }

    #[test]
    fn test_remove_measurement() {
        let mut f = Floor::new("Test");
        f.add_measurement(make_measurement("id-1"));
        f.add_measurement(make_measurement("id-2"));
        f.remove_measurement("id-1");
        assert_eq!(f.measurements.len(), 1);
        assert_eq!(f.measurements[0].id, "id-2");
    }

    #[test]
    fn test_remove_nonexistent_measurement() {
        let mut f = Floor::new("Test");
        f.add_measurement(make_measurement("id-1"));
        f.remove_measurement("nonexistent-id");
        assert_eq!(f.measurements.len(), 1);
    }

    #[test]
    fn test_remove_all_measurements() {
        let mut f = Floor::new("Test");
        f.add_measurement(make_measurement("id-1"));
        f.add_measurement(make_measurement("id-2"));
        f.remove_measurement("id-1");
        f.remove_measurement("id-2");
        assert!(f.measurements.is_empty());
    }

    #[test]
    fn test_floor_new_with_string_owned() {
        let name = String::from("Level 2");
        let f = Floor::new(name);
        assert_eq!(f.name, "Level 2");
    }
}



