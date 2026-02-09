use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug)]
pub struct StorageCapacityError {
    pub path: PathBuf,
    pub available_bytes: u64,
    pub required_bytes: u64,
}

impl std::fmt::Display for StorageCapacityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Insufficient disk space in {}: need at least {:.1} MB free but only {:.1} MB available",
            self.path.display(),
            bytes_to_mb(self.required_bytes),
            bytes_to_mb(self.available_bytes)
        )
    }
}

impl std::error::Error for StorageCapacityError {}

pub fn ensure_disk_headroom(dir: &Path, min_free_bytes: u64) -> Result<()> {
    if min_free_bytes == 0 {
        return Ok(());
    }

    let available = available_bytes(dir)
        .with_context(|| format!("failed to determine free space under {}", dir.display()))?;

    if available < min_free_bytes {
        return Err(StorageCapacityError {
            path: dir.to_path_buf(),
            available_bytes: available,
            required_bytes: min_free_bytes,
        }
        .into());
    }

    Ok(())
}

pub fn available_bytes_under(dir: &Path) -> Result<u64> {
    available_bytes(dir)
        .with_context(|| format!("failed to determine free space under {}", dir.display()))
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReclaimOutcome {
    pub deleted_files: usize,
    pub freed_bytes: u64,
    pub remaining_bytes: u64,
}

const MAX_AUTOPURGE_FILES: usize = 500;

pub fn reclaim_disk_space(dir: &Path, min_free_bytes: u64) -> Result<ReclaimOutcome> {
    let mut outcome = ReclaimOutcome {
        remaining_bytes: available_bytes(dir).with_context(|| {
            format!(
                "failed to determine free space under {} before cleanup",
                dir.display()
            )
        })?,
        ..ReclaimOutcome::default()
    };

    if min_free_bytes == 0 || outcome.remaining_bytes >= min_free_bytes {
        return Ok(outcome);
    }

    let mut candidates: Vec<_> = fs::read_dir(dir)
        .with_context(|| format!("failed to inspect {} for cleanup", dir.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if metadata.is_file() {
                Some(CandidateFile {
                    path: entry.path(),
                    len: metadata.len(),
                    modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                })
            } else {
                None
            }
        })
        .collect();

    candidates.sort_by_key(|entry| entry.modified);

    for candidate in candidates.into_iter().take(MAX_AUTOPURGE_FILES) {
        if outcome.remaining_bytes >= min_free_bytes {
            break;
        }
        fs::remove_file(&candidate.path).with_context(|| {
            format!(
                "failed to delete {} during cleanup",
                candidate.path.display()
            )
        })?;
        outcome.deleted_files += 1;
        outcome.freed_bytes += candidate.len;
        outcome.remaining_bytes = available_bytes(dir).with_context(|| {
            format!(
                "failed to determine free space under {} after deleting {}",
                dir.display(),
                candidate.path.display()
            )
        })?;
    }

    Ok(outcome)
}

#[derive(Debug, Clone)]
struct CandidateFile {
    path: PathBuf,
    len: u64,
    modified: SystemTime,
}

fn bytes_to_mb(bytes: u64) -> f64 {
    const MB: f64 = 1024.0 * 1024.0;
    (bytes as f64) / MB
}

#[cfg(target_family = "unix")]
fn available_bytes(path: &Path) -> std::io::Result<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains null byte")
    })?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(stat.f_bavail as u64 * stat.f_frsize)
}

#[cfg(not(target_family = "unix"))]
fn available_bytes(_path: &Path) -> std::io::Result<u64> {
    Ok(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{available_bytes_under, ensure_disk_headroom, reclaim_disk_space};
    use std::io::Write;
    use std::path::Path;
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn passes_when_threshold_zero() {
        let dir = tempdir().expect("tempdir");
        ensure_disk_headroom(dir.path(), 0).expect("zero threshold succeeds");
    }

    #[test]
    fn reports_available_bytes_for_existing_path() {
        let dir = tempdir().expect("tempdir");
        let bytes = available_bytes_under(dir.path()).expect("available bytes");
        assert!(bytes > 0);
    }

    #[test]
    fn fails_when_threshold_exceeds_available() {
        let dir = tempdir().expect("tempdir");
        let err = ensure_disk_headroom(dir.path(), u64::MAX).expect_err("guard should fail");
        assert!(err.to_string().contains("Insufficient disk space"));
    }

    #[test]
    fn reclaims_oldest_captures_until_threshold_met() {
        let dir = tempdir().expect("tempdir");
        let capture_dir = dir.path();
        let old_path = capture_dir.join("capture-000.png");
        let new_path = capture_dir.join("capture-001.png");

        write_dummy_file(&old_path, 2 * 1024 * 1024);
        thread::sleep(Duration::from_millis(10));
        write_dummy_file(&new_path, 2 * 1024 * 1024);

        let baseline = super::available_bytes(capture_dir).expect("available bytes");
        let target = baseline + 1_000_000; // require ~1 MB more than currently free

        let outcome = reclaim_disk_space(capture_dir, target).expect("reclaim succeeds");
        assert!(outcome.deleted_files >= 1);
        assert!(outcome.freed_bytes >= 1_000_000);
        assert!(outcome.remaining_bytes >= target);
        assert!(!old_path.exists(), "oldest capture should be deleted");
        assert!(new_path.exists(), "newest capture should be retained");
    }

    fn write_dummy_file(path: &Path, size: usize) {
        let mut file = std::fs::File::create(path).expect("create file");
        let buf = vec![0u8; size];
        file.write_all(&buf).expect("write file");
        file.sync_all().expect("flush file");
    }
}
