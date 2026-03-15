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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_new() {
        let p = Project::new("My Project");
        assert_eq!(p.name, "My Project");
        assert!(p.floors.is_empty());
    }

    #[test]
    fn test_add_floor() {
        let mut p = Project::new("Test");
        p.add_floor(Floor::new("Floor 1"));
        assert_eq!(p.floors.len(), 1);
        assert_eq!(p.floors[0].name, "Floor 1");
        p.add_floor(Floor::new("Floor 2"));
        assert_eq!(p.floors.len(), 2);
    }

    #[test]
    fn test_remove_floor_at_index() {
        let mut p = Project::new("Test");
        p.add_floor(Floor::new("Floor 1"));
        p.add_floor(Floor::new("Floor 2"));
        p.add_floor(Floor::new("Floor 3"));
        p.remove_floor(1);
        assert_eq!(p.floors.len(), 2);
        assert_eq!(p.floors[0].name, "Floor 1");
        assert_eq!(p.floors[1].name, "Floor 3");
    }

    #[test]
    fn test_remove_floor_first() {
        let mut p = Project::new("Test");
        p.add_floor(Floor::new("Floor 1"));
        p.add_floor(Floor::new("Floor 2"));
        p.remove_floor(0);
        assert_eq!(p.floors.len(), 1);
        assert_eq!(p.floors[0].name, "Floor 2");
    }

    #[test]
    fn test_remove_floor_out_of_bounds_does_not_panic() {
        let mut p = Project::new("Test");
        p.remove_floor(0);
        p.remove_floor(5);
        assert!(p.floors.is_empty());
    }

    #[test]
    fn test_remove_last_floor() {
        let mut p = Project::new("Test");
        p.add_floor(Floor::new("Only Floor"));
        p.remove_floor(0);
        assert!(p.floors.is_empty());
    }

    #[test]
    fn test_project_new_with_owned_string() {
        let p = Project::new(String::from("My Building"));
        assert_eq!(p.name, "My Building");
    }
}
