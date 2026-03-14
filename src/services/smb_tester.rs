use anyhow::{Context, Result};
use smb::{Client, ClientConfig, CreateOptions, FileAttributes, FileCreateArgs, Resource, UncPath};
use std::str::FromStr;

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
        tokio::runtime::Runtime::new()
            .context("Failed to create async runtime")?
            .block_on(self.run_test_async())
    }

    async fn run_test_async(&self) -> Result<f64> {
        let share_unc = format!(r"\\{}\{}", self.server, self.share);
        let target = UncPath::from_str(&share_unc)
            .map_err(|e| anyhow::anyhow!("Invalid UNC path '{share_unc}': {e}"))?;

        let client = Client::new(ClientConfig::default());
        let username = self.username.as_deref().unwrap_or("guest");
        let password = self.password.clone().unwrap_or_default();

        client
            .share_connect(&target, username, password)
            .await
            .context("Failed to connect to SMB share")?;

        // Unique filename per run to avoid conflicts
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let file_unc = format!(r"\\{}\{}\wifichecker_{ts}.dat", self.server, self.share);
        let file_path = UncPath::from_str(&file_unc)
            .map_err(|e| anyhow::anyhow!("Invalid file UNC path: {e}"))?;

        let write_args = FileCreateArgs::make_create_new(
            FileAttributes::new(),
            CreateOptions::new(),
        );

        let resource = client
            .create_file(&file_path, &write_args)
            .await
            .context("Failed to create test file on SMB share")?;

        const TOTAL_BYTES: usize = 10 * 1024 * 1024; // 10 MB
        const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB chunks
        let data = vec![0u8; CHUNK_SIZE];

        let start = std::time::Instant::now();

        if let Resource::File(file) = resource {
            let mut offset = 0u64;
            while offset < TOTAL_BYTES as u64 {
                let written = file
                    .write_block(&data, offset, None)
                    .await
                    .context("Failed to write to SMB share")?;
                offset += written as u64;
            }
            file.close().await.ok();
        }

        let elapsed = start.elapsed().as_secs_f64();
        client.close().await.ok();

        Ok((10.0 * 8.0) / elapsed)
    }
}
