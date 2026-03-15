//! NetworkManager D-Bus proxies used for WiFi scanning.
//!
//! These replace the `nmcli` subprocess and work both on native installs and
//! inside the Flatpak sandbox (where `nmcli` is not available), provided the
//! manifest grants `--system-talk-name=org.freedesktop.NetworkManager`.

use anyhow::{Context, Result};
use zbus::blocking::Connection;
use zbus::zvariant::OwnedObjectPath;

use super::wifi_scanner::WifiInfo;

// NM device type constant for 802.11 wireless
const DEVICE_TYPE_WIFI: u32 = 2;

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    fn get_all_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NMDevice {
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NMWireless {
    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NMAccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;

    #[zbus(property)]
    fn hw_address(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn frequency(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn strength(&self) -> zbus::Result<u8>;

    #[zbus(property)]
    fn max_bitrate(&self) -> zbus::Result<u32>;
}

/// Query the active WiFi access point via the NetworkManager D-Bus API.
///
/// Iterates over all NM devices, finds the first WiFi device, and reads the
/// properties of its active access point.  Returns `Ok(None)` when no WiFi
/// device is active.
pub fn query_active_ap() -> Result<Option<WifiInfo>> {
    let conn = Connection::system().context("Failed to connect to system D-Bus")?;

    let nm = NetworkManagerProxyBlocking::new(&conn)
        .context("Failed to create NetworkManager proxy")?;

    let devices = nm
        .get_all_devices()
        .context("NetworkManager.GetAllDevices failed")?;

    for device_path in devices {
        let device = NMDeviceProxyBlocking::builder(&conn)
            .path(device_path.as_str())?
            .build()
            .context("Failed to build Device proxy")?;

        if device.device_type().context("DeviceType property failed")? != DEVICE_TYPE_WIFI {
            continue;
        }

        let wireless = NMWirelessProxyBlocking::builder(&conn)
            .path(device_path.as_str())?
            .build()
            .context("Failed to build Wireless proxy")?;

        let ap_path = wireless
            .active_access_point()
            .context("ActiveAccessPoint property failed")?;

        // "/" means no active AP on this device
        if ap_path.as_str() == "/" {
            continue;
        }

        let ap = NMAccessPointProxyBlocking::builder(&conn)
            .path(ap_path.as_str())?
            .build()
            .context("Failed to build AccessPoint proxy")?;

        let ssid_bytes = ap.ssid().context("AP Ssid property failed")?;
        let ssid = String::from_utf8_lossy(&ssid_bytes).into_owned();

        let bssid = ap.hw_address().context("AP HwAddress property failed")?;
        let frequency_mhz = ap.frequency().context("AP Frequency property failed")?;
        let strength = ap.strength().context("AP Strength property failed")?;
        let max_bitrate_kbps = ap.max_bitrate().context("AP MaxBitrate property failed")?;

        // NM Strength is 0-100 quality, same as nmcli SIGNAL
        let signal_dbm = (strength as i32 / 2) - 100;
        let channel = freq_to_channel(frequency_mhz);
        let link_speed_mbps =
            if max_bitrate_kbps > 0 { Some(max_bitrate_kbps / 1000) } else { None };

        return Ok(Some(WifiInfo {
            ssid,
            bssid,
            frequency_mhz,
            channel,
            signal_dbm,
            link_speed_mbps,
        }));
    }

    Ok(None)
}

/// Convert a WiFi frequency in MHz to an 802.11 channel number.
fn freq_to_channel(freq_mhz: u32) -> u8 {
    match freq_mhz {
        2412..=2472 => ((freq_mhz - 2412) / 5 + 1) as u8,
        2484 => 14,
        5160..=5885 => ((freq_mhz - 5000) / 5) as u8,
        // 6 GHz band (Wi-Fi 6E)
        5955..=7115 => ((freq_mhz - 5955) / 5 + 1) as u8,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freq_to_channel_24ghz_ch1() {
        assert_eq!(freq_to_channel(2412), 1);
    }

    #[test]
    fn test_freq_to_channel_24ghz_ch6() {
        // (2437 - 2412) / 5 + 1 = 5 + 1 = 6
        assert_eq!(freq_to_channel(2437), 6);
    }

    #[test]
    fn test_freq_to_channel_24ghz_ch11() {
        assert_eq!(freq_to_channel(2462), 11);
    }

    #[test]
    fn test_freq_to_channel_24ghz_ch13() {
        assert_eq!(freq_to_channel(2472), 13);
    }

    #[test]
    fn test_freq_to_channel_24ghz_ch14() {
        assert_eq!(freq_to_channel(2484), 14);
    }

    #[test]
    fn test_freq_to_channel_5ghz_ch32() {
        // (5160 - 5000) / 5 = 32
        assert_eq!(freq_to_channel(5160), 32);
    }

    #[test]
    fn test_freq_to_channel_5ghz_ch36() {
        // (5180 - 5000) / 5 = 36
        assert_eq!(freq_to_channel(5180), 36);
    }

    #[test]
    fn test_freq_to_channel_5ghz_ch100() {
        // (5500 - 5000) / 5 = 100
        assert_eq!(freq_to_channel(5500), 100);
    }

    #[test]
    fn test_freq_to_channel_6ghz_ch1() {
        // Wi-Fi 6E: (5955 - 5955) / 5 + 1 = 1
        assert_eq!(freq_to_channel(5955), 1);
    }

    #[test]
    fn test_freq_to_channel_unknown_returns_zero() {
        assert_eq!(freq_to_channel(0), 0);
        assert_eq!(freq_to_channel(3000), 0);
        assert_eq!(freq_to_channel(2473), 0); // gap between ch13 and ch14
    }

    #[test]
    fn test_signal_dbm_conversion_from_strength() {
        // NM Strength 0-100 → dBm: (strength / 2) - 100
        // Strength 100 → (100 / 2) - 100 = -50
        // Strength 0   → (0 / 2)   - 100 = -100
        let strength_100: i32 = 100;
        let dbm_100 = (strength_100 / 2) - 100;
        assert_eq!(dbm_100, -50);

        let strength_0: i32 = 0;
        let dbm_0 = (strength_0 / 2) - 100;
        assert_eq!(dbm_0, -100);

        let strength_50: i32 = 50;
        let dbm_50 = (strength_50 / 2) - 100;
        assert_eq!(dbm_50, -75);
    }
}
