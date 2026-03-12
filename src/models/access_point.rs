use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPoint {
    pub ssid: String,
    pub bssid: String,
    pub frequency_mhz: u32,
    pub channel: u8,
    pub signal_dbm: i32,
}

impl AccessPoint {
    pub fn band(&self) -> &str {
        if self.frequency_mhz >= 5000 {
            "5 GHz"
        } else if self.frequency_mhz >= 2400 {
            "2.4 GHz"
        } else {
            "Unknown"
        }
    }
}
