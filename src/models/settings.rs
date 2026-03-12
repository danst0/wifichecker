use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // iperf3
    pub iperf_enabled: bool,
    pub iperf_server: String,
    pub iperf_port: u16,
    pub iperf_duration_secs: u32,

    // Samba
    pub smb_enabled: bool,
    pub smb_server: String,
    pub smb_share: String,
    pub smb_username: String,
    pub smb_password: String,

    // Grid
    pub show_grid: bool,
    pub grid_spacing_m: f64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            iperf_enabled: false,
            iperf_server: String::new(),
            iperf_port: 5201,
            iperf_duration_secs: 5,

            smb_enabled: false,
            smb_server: String::new(),
            smb_share: String::new(),
            smb_username: String::new(),
            smb_password: String::new(),

            show_grid: true,
            grid_spacing_m: 1.0,
        }
    }
}
