use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ThroughputUnit {
    Mbit,
    MByte,
}

impl Default for ThroughputUnit {
    fn default() -> Self { ThroughputUnit::Mbit }
}

impl ThroughputUnit {
    /// Format a Mbit/s value according to this unit.
    pub fn format(&self, mbit_per_sec: f64) -> String {
        match self {
            ThroughputUnit::Mbit  => format!("{:.1} Mbit/s", mbit_per_sec),
            ThroughputUnit::MByte => format!("{:.1} MB/s",   mbit_per_sec / 8.0),
        }
    }
    /// Short label used in list rows.
    pub fn format_short(&self, mbit_per_sec: f64) -> String {
        match self {
            ThroughputUnit::Mbit  => format!("{:.0} Mbit/s", mbit_per_sec),
            ThroughputUnit::MByte => format!("{:.0} MB/s",   mbit_per_sec / 8.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    // iperf3
    pub iperf_enabled: bool,
    pub iperf_server: String,
    pub iperf_port: u16,
    pub iperf_duration_secs: u32,
    pub iperf_parallel_streams: u8,

    // Samba
    pub smb_enabled: bool,
    pub smb_server: String,
    pub smb_share: String,
    pub smb_username: String,
    pub smb_password: String,

    // Grid
    pub show_grid: bool,
    pub grid_spacing_m: f64,        // visual grid line spacing
    pub measurement_grid_spacing_m: f64, // cell size for snapping & cell coloring
    pub snap_to_grid: bool,

    // Display
    pub throughput_unit: ThroughputUnit,

    // UI state
    pub last_floor_index: usize,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            iperf_enabled: false,
            iperf_server: String::new(),
            iperf_port: 5201,
            iperf_duration_secs: 5,
            iperf_parallel_streams: 4,

            smb_enabled: false,
            smb_server: String::new(),
            smb_share: String::new(),
            smb_username: String::new(),
            smb_password: String::new(),

            show_grid: true,
            grid_spacing_m: 1.0,
            measurement_grid_spacing_m: 1.0,
            snap_to_grid: false,

            throughput_unit: ThroughputUnit::Mbit,

            last_floor_index: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_throughput_unit_default_is_mbit() {
        assert_eq!(ThroughputUnit::default(), ThroughputUnit::Mbit);
    }

    #[test]
    fn test_format_mbit() {
        let unit = ThroughputUnit::Mbit;
        assert_eq!(unit.format(100.0), "100.0 Mbit/s");
        assert_eq!(unit.format(0.0), "0.0 Mbit/s");
        assert_eq!(unit.format(54.7), "54.7 Mbit/s");
    }

    #[test]
    fn test_format_mbyte() {
        let unit = ThroughputUnit::MByte;
        assert_eq!(unit.format(80.0), "10.0 MB/s");
        assert_eq!(unit.format(8.0), "1.0 MB/s");
        assert_eq!(unit.format(0.0), "0.0 MB/s");
    }

    #[test]
    fn test_format_short_mbit() {
        let unit = ThroughputUnit::Mbit;
        assert_eq!(unit.format_short(100.0), "100 Mbit/s");
        assert_eq!(unit.format_short(54.7), "55 Mbit/s");
    }

    #[test]
    fn test_format_short_mbyte() {
        let unit = ThroughputUnit::MByte;
        assert_eq!(unit.format_short(80.0), "10 MB/s");
        assert_eq!(unit.format_short(16.0), "2 MB/s");
    }

    #[test]
    fn test_appsettings_defaults() {
        let s = AppSettings::default();
        assert!(!s.iperf_enabled);
        assert_eq!(s.iperf_port, 5201);
        assert_eq!(s.iperf_duration_secs, 5);
        assert_eq!(s.iperf_parallel_streams, 4);
        assert!(s.iperf_server.is_empty());
        assert!(!s.smb_enabled);
        assert!(s.smb_server.is_empty());
        assert!(s.show_grid);
        assert_eq!(s.grid_spacing_m, 1.0);
        assert_eq!(s.measurement_grid_spacing_m, 1.0);
        assert!(!s.snap_to_grid);
        assert_eq!(s.throughput_unit, ThroughputUnit::Mbit);
    }

    #[test]
    fn test_appsettings_json_roundtrip() {
        let s = AppSettings {
            iperf_enabled: true,
            iperf_server: "192.168.1.100".to_string(),
            iperf_port: 5202,
            throughput_unit: ThroughputUnit::MByte,
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let s2: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s2.iperf_enabled, true);
        assert_eq!(s2.iperf_server, "192.168.1.100");
        assert_eq!(s2.iperf_port, 5202);
        assert_eq!(s2.throughput_unit, ThroughputUnit::MByte);
    }

    #[test]
    fn test_appsettings_deserialize_missing_fields_uses_defaults() {
        // Partial JSON — missing fields should fall back to defaults via #[serde(default)]
        let json = r#"{"iperf_port": 9999}"#;
        let s: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(s.iperf_port, 9999);
        assert!(!s.iperf_enabled); // default
        assert_eq!(s.throughput_unit, ThroughputUnit::Mbit); // default
    }
}
