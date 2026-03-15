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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ap(frequency_mhz: u32) -> AccessPoint {
        AccessPoint {
            ssid: "Test".to_string(),
            bssid: "AA:BB:CC:DD:EE:FF".to_string(),
            frequency_mhz,
            channel: 1,
            signal_dbm: -60,
        }
    }

    #[test]
    fn test_band_24ghz() {
        assert_eq!(make_ap(2412).band(), "2.4 GHz");
        assert_eq!(make_ap(2462).band(), "2.4 GHz");
        assert_eq!(make_ap(2400).band(), "2.4 GHz");
        assert_eq!(make_ap(4999).band(), "2.4 GHz");
    }

    #[test]
    fn test_band_5ghz() {
        assert_eq!(make_ap(5000).band(), "5 GHz");
        assert_eq!(make_ap(5180).band(), "5 GHz");
        assert_eq!(make_ap(5885).band(), "5 GHz");
    }

    #[test]
    fn test_band_6ghz_classified_as_5ghz() {
        // 6 GHz frequencies (Wi-Fi 6E) are >= 5000, so classified as "5 GHz"
        assert_eq!(make_ap(5955).band(), "5 GHz");
        assert_eq!(make_ap(6000).band(), "5 GHz");
    }

    #[test]
    fn test_band_unknown() {
        assert_eq!(make_ap(0).band(), "Unknown");
        assert_eq!(make_ap(1000).band(), "Unknown");
        assert_eq!(make_ap(2399).band(), "Unknown");
    }
}
