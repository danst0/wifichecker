use serde::{Deserialize, Serialize};
use crate::models::Floor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub floors: Vec<Floor>,
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            floors: Vec::new(),
        }
    }

    pub fn add_floor(&mut self, floor: Floor) {
        self.floors.push(floor);
    }

    pub fn remove_floor(&mut self, index: usize) {
        if index < self.floors.len() {
            self.floors.remove(index);
        }
    }
}
