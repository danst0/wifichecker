use anyhow::Result;

#[derive(Debug, Clone)]
pub struct WifiInfo {
    pub ssid: String,
    pub bssid: String,
    pub frequency_mhz: u32,
    pub channel: u8,
    pub signal_dbm: i32,
    pub link_speed_mbps: Option<u32>,
}

pub struct WifiScanner;

impl WifiScanner {
    /// Query the currently connected WiFi access point via the NetworkManager
    /// D-Bus API.  This works both on native installs and inside the Flatpak
    /// sandbox (where `nmcli` is not available as a subprocess).
    pub fn scan() -> Result<Option<WifiInfo>> {
        super::nm_dbus::query_active_ap()
    }
}

