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
