use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ContextEntry {
    pub capture_index: u64,
    pub timestamp: DateTime<Utc>,
    pub image_path: PathBuf,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct ContextLog {
    path: PathBuf,
}

impl ContextLog {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, entry: &ContextEntry) -> Result<()> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create context parent directory {}",
                    parent.display()
                )
            })?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("failed to open context file {}", self.path.display()))?;

        writeln!(
            file,
            "## Capture {} at {}",
            entry.capture_index,
            entry.timestamp.to_rfc3339()
        )?;
        writeln!(file, "- Image: {}", entry.image_path.display())?;
        writeln!(file, "- Summary: {}", entry.summary.replace('\n', " "))?;
        writeln!(file)?;
        Ok(())
    }

    pub fn append_skipped(
        &self,
        tick_index: u64,
        timestamp: DateTime<Utc>,
        reason: &str,
    ) -> Result<()> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create context parent directory {}",
                    parent.display()
                )
            })?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("failed to open context file {}", self.path.display()))?;

        writeln!(
            file,
            "## Skipped tick {} at {}",
            tick_index,
            timestamp.to_rfc3339()
        )?;
        writeln!(file, "- Reason: {}", reason.replace('\n', " "))?;
        writeln!(file)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ContextEntry, ContextLog};
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn appends_markdown_entry() {
        let temp = tempdir().expect("tempdir");
        let context_path = temp.path().join("context.md");
        let context = ContextLog::new(&context_path);

        context
            .append(&ContextEntry {
                capture_index: 1,
                timestamp: Utc::now(),
                image_path: temp.path().join("capture.png"),
                summary: "hello world".to_string(),
            })
            .expect("append succeeds");

        let content = std::fs::read_to_string(&context_path).expect("context exists");
        assert!(content.contains("## Capture 1"));
        assert!(content.contains("Summary: hello world"));
    }
}
