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
