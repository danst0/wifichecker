use anyhow::{Context, Result};
use std::process::Command;

pub struct IperfClient {
    pub server: String,
    pub port: u16,
    pub duration_secs: u32,
}

impl IperfClient {
    pub fn new(server: impl Into<String>, port: u16, duration_secs: u32) -> Self {
        Self { server: server.into(), port, duration_secs }
    }

    /// Run iperf3 test (blocking). Returns throughput in Mbps.
    pub fn run_test(&self) -> Result<f64> {
        let output = Command::new("iperf3")
            .args([
                "-c", &self.server,
                "-p", &self.port.to_string(),
                "-t", &self.duration_secs.to_string(),
                "-J",
            ])
            .output()
            .context("Failed to run iperf3 (is it installed?)")?;

        if !output.status.success() {
            anyhow::bail!("iperf3 failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .context("Failed to parse iperf3 JSON output")?;

        let bps = json["end"]["sum_received"]["bits_per_second"]
            .as_f64()
            .context("Missing bits_per_second in iperf3 output")?;

        Ok(bps / 1_000_000.0)
    }
}
