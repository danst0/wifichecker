use anyhow::{Context, Result};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::utils::is_flatpak;

pub struct IperfClient {
    pub server: String,
    pub port: u16,
    pub duration_secs: u32,
}

impl IperfClient {
    pub fn new(server: impl Into<String>, port: u16, duration_secs: u32) -> Self {
        Self { server: server.into(), port, duration_secs }
    }

    /// Run iperf test (blocking). Tries iperf3 first, falls back to iperf2.
    /// Returns throughput in Mbps.
    pub fn run_test(&self) -> Result<f64> {
        match self.run_iperf3() {
            Ok(mbps) => return Ok(mbps),
            Err(e) if is_not_found(&e) => {} // fall through to iperf2
            Err(e) => return Err(e),
        }
        self.run_iperf2()
    }

    fn run_iperf3(&self) -> Result<f64> {
        // Inside Flatpak the host iperf3 is not accessible; use the bundled
        // binary installed to /app/bin/iperf3 by the flatpak module.
        let binary = if is_flatpak() { "/app/bin/iperf3" } else { "iperf3" };

        // Total timeout: test duration + 5 s for connection/reporting
        let timeout = Duration::from_secs(self.duration_secs as u64 + 5);
        let mut child = Command::new(binary)
            .args([
                "-c", &self.server,
                "-p", &self.port.to_string(),
                "-t", &self.duration_secs.to_string(),
                "-J",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("iperf3 not found")?;

        let (stdout, stderr) = wait_with_timeout(&mut child, timeout)
            .context("iperf3 timed out — server unreachable?")?;

        if !child.wait().map(|s| s.success()).unwrap_or(false) {
            anyhow::bail!("iperf3 failed: {}", String::from_utf8_lossy(&stderr));
        }

        let json: serde_json::Value = serde_json::from_slice(&stdout)
            .context("Failed to parse iperf3 JSON output")?;

        let bps = json["end"]["sum_received"]["bits_per_second"]
            .as_f64()
            .context("Missing bits_per_second in iperf3 output")?;

        Ok(bps / 1_000_000.0)
    }

    fn run_iperf2(&self) -> Result<f64> {
        let timeout = Duration::from_secs(self.duration_secs as u64 + 5);
        let mut child = Command::new("iperf")
            .args([
                "-c", &self.server,
                "-p", &self.port.to_string(),
                "-t", &self.duration_secs.to_string(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("iperf not found (neither iperf3 nor iperf2 is installed)")?;

        let (stdout, _stderr) = wait_with_timeout(&mut child, timeout)
            .context("iperf timed out — server unreachable?")?;

        if !child.wait().map(|s| s.success()).unwrap_or(false) {
            anyhow::bail!("iperf failed: {}", String::from_utf8_lossy(&_stderr));
        }

        let text = String::from_utf8_lossy(&stdout);
        parse_iperf2_mbps(&text).context("Failed to parse iperf2 output")
    }
}

/// Poll the child every 100 ms; kill and return Err if the deadline is exceeded.
/// On success returns (stdout_bytes, stderr_bytes).
fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> Result<(Vec<u8>, Vec<u8>)> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait()? {
            Some(_status) => {
                // Process finished — drain pipes
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(ref mut out) = child.stdout { out.read_to_end(&mut stdout).ok(); }
                if let Some(ref mut err) = child.stderr { err.read_to_end(&mut stderr).ok(); }
                return Ok((stdout, stderr));
            }
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait(); // reap zombie
                    anyhow::bail!("timed out");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Returns true when the error chain contains an OS "not found" (ENOENT) error.
fn is_not_found(e: &anyhow::Error) -> bool {
    e.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .map(|io| io.kind() == std::io::ErrorKind::NotFound)
            .unwrap_or(false)
    })
}

/// Parse the summary line from iperf2 plain-text output and return Mbps.
///
/// Example line:
///   [  3]  0.0- 5.0 sec  38.1 MBytes  64.0 Mbits/sec
fn parse_iperf2_mbps(output: &str) -> Option<f64> {
    let line = output.lines().filter(|l| l.contains(" sec ")).last()?;
    let tokens: Vec<&str> = line.split_whitespace().collect();
    for (i, token) in tokens.iter().enumerate() {
        let unit_mbps = match *token {
            "Mbits/sec" => 1.0,
            "Gbits/sec" => 1_000.0,
            "Kbits/sec" => 0.001,
            _ => continue,
        };
        if i > 0 {
            if let Ok(val) = tokens[i - 1].parse::<f64>() {
                return Some(val * unit_mbps);
            }
        }
    }
    None
}
