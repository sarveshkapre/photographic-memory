use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use std::path::Path;
use std::process::Command;

#[async_trait]
pub trait ScreenshotProvider: Send + Sync {
    async fn capture(&self, output_path: &Path) -> Result<()>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MacOsScreenshotProvider;

#[async_trait]
impl ScreenshotProvider for MacOsScreenshotProvider {
    async fn capture(&self, output_path: &Path) -> Result<()> {
        let status = Command::new("screencapture")
            .arg("-x")
            .arg("-t")
            .arg("png")
            .arg(output_path)
            .status()
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
