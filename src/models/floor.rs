use serde::{Deserialize, Serialize};
use crate::models::Measurement;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Floor {
    pub name: String,
    /// Path to the floor plan image (PNG/JPG)
    pub image_path: Option<String>,
    pub measurements: Vec<Measurement>,
}

impl Floor {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            image_path: None,
            measurements: Vec::new(),
        }
    }

    pub fn add_measurement(&mut self, measurement: Measurement) {
        self.measurements.push(measurement);
    }

    pub fn remove_measurement(&mut self, id: &str) {
        self.measurements.retain(|m| m.id != id);
    }
}
