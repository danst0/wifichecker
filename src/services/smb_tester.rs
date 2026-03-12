use anyhow::{Context, Result};
use std::process::Command;

pub struct SmbTester {
    pub server: String,
    pub share: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl SmbTester {
    pub fn new(server: impl Into<String>, share: impl Into<String>) -> Self {
        Self {
            server: server.into(),
            share: share.into(),
            username: None,
            password: None,
        }
    }

    /// Test SMB upload speed with a 10 MB file (blocking). Returns Mbps.
    pub fn run_test(&self) -> Result<f64> {
        let share_path = format!("//{}/{}", self.server, self.share);
        let test_file = "wifichecker_speedtest.dat";
        let tmp_path = format!("/tmp/{}", test_file);

        std::fs::write(&tmp_path, vec![0u8; 10 * 1024 * 1024])
            .context("Failed to create temp test file")?;

        let mut args = vec![
            share_path,
            "-c".to_string(),
            format!("put {} {}", tmp_path, test_file),
        ];
        if let Some(ref user) = self.username {
            args.extend(["-U".to_string(), user.clone()]);
        }
        if let Some(ref pass) = self.password {
            args.extend(["--password".to_string(), pass.clone()]);
        }

        let start = std::time::Instant::now();
        let status = Command::new("smbclient")
            .args(&args)
            .output()
            .context("Failed to run smbclient (is it installed?)")?;
        let elapsed = start.elapsed().as_secs_f64();

        let _ = std::fs::remove_file(&tmp_path);

        if !status.status.success() {
            anyhow::bail!("smbclient failed: {}", String::from_utf8_lossy(&status.stderr));
        }

        Ok((10.0 * 8.0) / elapsed)
    }
}

