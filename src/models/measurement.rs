use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single WiFi measurement at a position on the floor plan.
/// x and y are relative coordinates in [0.0, 1.0].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    pub id: String,
    /// Relative x coordinate on floor plan image (0.0 = left, 1.0 = right)
    pub x: f64,
    /// Relative y coordinate on floor plan image (0.0 = top, 1.0 = bottom)
    pub y: f64,
    pub timestamp: DateTime<Utc>,
    pub ssid: String,
    pub bssid: String,
    pub frequency_mhz: u32,
    pub channel: u8,
    pub signal_dbm: i32,
    pub noise_dbm: Option<i32>,
    pub link_speed_mbps: Option<u32>,
    pub iperf_mbps: Option<f64>,
    pub smb_mbps: Option<f64>,
}

impl Measurement {
    pub fn new(x: f64, y: f64, ssid: String, bssid: String, frequency_mhz: u32, channel: u8, signal_dbm: i32) -> Self {
        Self {
            id: uuid_v4(),
            x,
            y,
            timestamp: Utc::now(),
            ssid,
            bssid,
            frequency_mhz,
            channel,
            signal_dbm,
            noise_dbm: None,
            link_speed_mbps: None,
            iperf_mbps: None,
            smb_mbps: None,
        }
    }

    pub fn signal_quality_percent(&self) -> u8 {
        // Convert dBm to quality percentage (0-100)
        // Typical range: -30 dBm (excellent) to -90 dBm (no signal)
        let clamped = self.signal_dbm.clamp(-90, -30);
        ((clamped + 90) * 100 / 60) as u8
    }
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        t,
        (t >> 16) & 0xffff,
        (t >> 4) & 0x0fff,
        0x8000 | ((t >> 2) & 0x3fff),
        t as u64 * 0x1000000,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_measurement(signal_dbm: i32) -> Measurement {
        Measurement::new(0.5, 0.5, "TestSSID".to_string(), "AA:BB:CC:DD:EE:FF".to_string(), 2412, 1, signal_dbm)
    }

    #[test]
    fn test_signal_quality_excellent() {
        assert_eq!(make_measurement(-30).signal_quality_percent(), 100);
    }

    #[test]
    fn test_signal_quality_no_signal() {
        assert_eq!(make_measurement(-90).signal_quality_percent(), 0);
    }

    #[test]
    fn test_signal_quality_midpoint() {
        // (-60 + 90) * 100 / 60 = 50
        assert_eq!(make_measurement(-60).signal_quality_percent(), 50);
    }

    #[test]
    fn test_signal_quality_clamped_above() {
        assert_eq!(make_measurement(-20).signal_quality_percent(), 100);
    }

    #[test]
    fn test_signal_quality_clamped_below() {
        assert_eq!(make_measurement(-100).signal_quality_percent(), 0);
    }

    #[test]
    fn test_signal_quality_near_boundaries() {
        // -31 dBm → (59 * 100) / 60 = 98
        assert_eq!(make_measurement(-31).signal_quality_percent(), 98);
        // -89 dBm → (1 * 100) / 60 = 1
        assert_eq!(make_measurement(-89).signal_quality_percent(), 1);
    }

    #[test]
    fn test_measurement_new_optional_fields_are_none() {
        let m = make_measurement(-60);
        assert!(m.noise_dbm.is_none());
        assert!(m.link_speed_mbps.is_none());
        assert!(m.iperf_mbps.is_none());
        assert!(m.smb_mbps.is_none());
    }

    #[test]
    fn test_measurement_new_fields() {
        let m = Measurement::new(0.3, 0.7, "SSID".to_string(), "AA:BB:CC:DD:EE:FF".to_string(), 5180, 36, -65);
        assert_eq!(m.x, 0.3);
        assert_eq!(m.y, 0.7);
        assert_eq!(m.frequency_mhz, 5180);
        assert_eq!(m.channel, 36);
        assert_eq!(m.signal_dbm, -65);
        assert_eq!(m.ssid, "SSID");
        assert_eq!(m.bssid, "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn test_uuid_format() {
        let m = make_measurement(-60);
        assert!(!m.id.is_empty());
        // UUID-like format: 5 groups separated by dashes
        let parts: Vec<&str> = m.id.split('-').collect();
        assert_eq!(parts.len(), 5);
        // All parts are non-empty hex strings
        for part in &parts {
            assert!(!part.is_empty());
            assert!(part.chars().all(|c| c.is_ascii_hexdigit()), "non-hex char in part: {part}");
        }
        // First group is exactly 8 hex chars (formatted with {:08x})
        assert_eq!(parts[0].len(), 8);
        // Second group is exactly 4 hex chars
        assert_eq!(parts[1].len(), 4);
        // Third group starts with '4' (version marker)
        assert!(parts[2].starts_with('4'));
        // Fourth group is exactly 4 hex chars
        assert_eq!(parts[3].len(), 4);
    }

    #[test]
    fn test_uuid_version_and_variant_bits() {
        let m = make_measurement(-60);
        let parts: Vec<&str> = m.id.split('-').collect();
        // Version 4: third group starts with '4'
        assert!(parts[2].starts_with('4'));
        // Variant: fourth group starts with '8', '9', 'a', or 'b'
        let variant = parts[3].chars().next().unwrap();
        assert!(matches!(variant, '8' | '9' | 'a' | 'b'));
    }
}
