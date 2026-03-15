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
        }
    }
}
