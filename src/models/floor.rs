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



