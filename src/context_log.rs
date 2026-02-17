use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::fs::{File, OpenOptions, create_dir_all};
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

    fn open_append_file(&self) -> Result<File> {
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

        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("failed to open context file {}", self.path.display()))
    }

    pub fn append(&self, entry: &ContextEntry) -> Result<()> {
        let mut file = self.open_append_file()?;

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
        let mut file = self.open_append_file()?;

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

    pub fn append_session_transition(
        &self,
        timestamp: DateTime<Utc>,
        state: &str,
        trigger: &str,
    ) -> Result<()> {
        let mut file = self.open_append_file()?;

        writeln!(
            file,
            "## Session {} at {}",
            state.replace('\n', " "),
            timestamp.to_rfc3339()
        )?;
        writeln!(file, "- Trigger: {}", trigger.replace('\n', " "))?;
        writeln!(file)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ContextEntry, ContextLog};
    use chrono::{DateTime, Utc};
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

    #[test]
    fn capture_entry_format_is_stable_and_flattens_newlines() {
        let temp = tempdir().expect("tempdir");
        let context_path = temp.path().join("context.md");
        let context = ContextLog::new(&context_path);

        let timestamp: DateTime<Utc> = DateTime::parse_from_rfc3339("2026-02-09T00:00:00Z")
            .expect("valid timestamp")
            .with_timezone(&Utc);

        context
            .append(&ContextEntry {
                capture_index: 7,
                timestamp,
                image_path: "captures/capture-000007.png".into(),
                summary: "line one\nline two".to_string(),
            })
            .expect("append succeeds");

        let content = std::fs::read_to_string(&context_path).expect("context exists");
        assert_eq!(
            content,
            concat!(
                "## Capture 7 at 2026-02-09T00:00:00+00:00\n",
                "- Image: captures/capture-000007.png\n",
                "- Summary: line one line two\n",
                "\n"
            )
        );
    }

    #[test]
    fn skipped_entry_format_is_stable_and_flattens_newlines() {
        let temp = tempdir().expect("tempdir");
        let context_path = temp.path().join("context.md");
        let context = ContextLog::new(&context_path);

        let timestamp: DateTime<Utc> = DateTime::parse_from_rfc3339("2026-02-09T00:00:00Z")
            .expect("valid timestamp")
            .with_timezone(&Utc);

        context
            .append_skipped(3, timestamp, "privacy: denied\napp")
            .expect("append succeeds");

        let content = std::fs::read_to_string(&context_path).expect("context exists");
        assert_eq!(
            content,
            concat!(
                "## Skipped tick 3 at 2026-02-09T00:00:00+00:00\n",
                "- Reason: privacy: denied app\n",
                "\n"
            )
        );
    }

    #[test]
    fn session_transition_entry_format_is_stable_and_flattens_newlines() {
        let temp = tempdir().expect("tempdir");
        let context_path = temp.path().join("context.md");
        let context = ContextLog::new(&context_path);

        let timestamp: DateTime<Utc> = DateTime::parse_from_rfc3339("2026-02-09T00:00:00Z")
            .expect("valid timestamp")
            .with_timezone(&Utc);

        context
            .append_session_transition(timestamp, "Paused", "auto: ScreenLocked\nretry")
            .expect("append succeeds");

        let content = std::fs::read_to_string(&context_path).expect("context exists");
        assert_eq!(
            content,
            concat!(
                "## Session Paused at 2026-02-09T00:00:00+00:00\n",
                "- Trigger: auto: ScreenLocked retry\n",
                "\n"
            )
        );
    }
}
