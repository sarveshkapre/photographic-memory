use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[async_trait]
pub trait ScreenshotProvider: Send + Sync {
    async fn capture(&self, output_path: &Path) -> Result<()>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MacOsScreenshotProvider;

const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(10);

#[async_trait]
impl ScreenshotProvider for MacOsScreenshotProvider {
    async fn capture(&self, output_path: &Path) -> Result<()> {
        let mut command = Command::new("screencapture");
        command.arg("-x").arg("-t").arg("png").arg(output_path);

        let status = timeout(SCREENSHOT_TIMEOUT, command.status())
            .await
            .map_err(|_| {
                anyhow!(
                    "screencapture timed out after {:.0}s — check Screen Recording permission",
                    SCREENSHOT_TIMEOUT.as_secs_f32()
                )
            })?
            .context("failed to execute screencapture")?;

        if !status.success() {
            bail!("screencapture exited with status {status}");
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MockScreenshotProvider;

#[async_trait]
impl ScreenshotProvider for MockScreenshotProvider {
    async fn capture(&self, output_path: &Path) -> Result<()> {
        std::fs::write(output_path, b"mock-image").with_context(|| {
            format!(
                "failed to write mock screenshot at {}",
                output_path.display()
            )
        })?;
        Ok(())
    }
}
