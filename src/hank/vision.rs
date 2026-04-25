#![allow(dead_code)]
// hank/vision.rs -- Basic screen capture and lightweight local OCR

use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::process::Command;
use tracing::info;

pub struct ScreenObserver {
    work_dir: PathBuf,
}

impl ScreenObserver {
    pub fn new(work_dir: PathBuf) -> Self {
        Self { work_dir }
    }

    /// Captures the primary screen to a temp file.
    pub async fn capture(&self) -> Result<PathBuf> {
        let path = self
            .work_dir
            .join(format!("capture_{}.png", chrono::Utc::now().timestamp()));

        info!("Hank capturing screen to {}", path.display());

        // Try gnome-screenshot (Linux) or screencapture (macOS)
        #[cfg(target_os = "macos")]
        let mut cmd = Command::new("screencapture");
        #[cfg(target_os = "macos")]
        cmd.args(["-x", path.to_str().unwrap()]);

        #[cfg(target_os = "linux")]
        let mut cmd = Command::new("gnome-screenshot");
        #[cfg(target_os = "linux")]
        cmd.args(["-f", path.to_str().unwrap()]);

        let status = cmd
            .status()
            .await
            .context("Failed to execute screenshot utility")?;

        if !status.success() {
            anyhow::bail!("Screenshot utility failed with {}", status);
        }

        Ok(path)
    }
}
