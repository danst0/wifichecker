use anyhow::{Context, Result};
use std::process::Command;

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
    /// Query the currently connected WiFi access point via nmcli.
    /// Uses IN-USE field (* = active) which is locale-independent.
    pub fn scan() -> Result<Option<WifiInfo>> {
        let output = Command::new("nmcli")
            .args(["-t", "-f", "IN-USE,SSID,BSSID,FREQ,SIGNAL,CHAN", "dev", "wifi", "list"])
            .output()
            .context("Failed to run nmcli – is NetworkManager installed?")?;

        let text = String::from_utf8_lossy(&output.stdout);

        for line in text.lines() {
            let fields = parse_terse_line(line);
            // IN-USE field is "*" when this is the active connection
            if fields.first().map(|s| s.trim()) != Some("*") {
                continue;
            }

            let ssid = fields.get(1).cloned().unwrap_or_default();
            let bssid = fields.get(2).cloned().unwrap_or_default();
            let freq_str = fields.get(3).cloned().unwrap_or_default();
            let signal_str = fields.get(4).cloned().unwrap_or_default();
            let chan_str = fields.get(5).cloned().unwrap_or_default();

            // Frequency field looks like "5180 MHz"
            let frequency_mhz: u32 = freq_str
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2412);

            // nmcli SIGNAL is 0–100 quality; approximate dBm conversion
            let quality: i32 = signal_str.trim().parse().unwrap_or(0);
            let signal_dbm = (quality / 2) - 100;

            let channel: u8 = chan_str.trim().parse().unwrap_or(0);

            return Ok(Some(WifiInfo {
                ssid,
                bssid,
                frequency_mhz,
                channel,
                signal_dbm,
                link_speed_mbps: None,
            }));
        }

        Ok(None)
    }
}

/// Parse a nmcli terse-mode line, respecting `\:` escape sequences.
fn parse_terse_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                chars.next();
                current.push(next);
            }
        } else if c == ':' {
            fields.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    fields.push(current);
    fields
}

