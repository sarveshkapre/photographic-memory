use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

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
    Ok(stat.f_bavail as u64 * stat.f_frsize as u64)
}

#[cfg(not(target_family = "unix"))]
fn available_bytes(_path: &Path) -> std::io::Result<u64> {
    Ok(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::ensure_disk_headroom;
    use tempfile::tempdir;

    #[test]
    fn passes_when_threshold_zero() {
        let dir = tempdir().expect("tempdir");
        ensure_disk_headroom(dir.path(), 0).expect("zero threshold succeeds");
    }

    #[test]
    fn fails_when_threshold_exceeds_available() {
        let dir = tempdir().expect("tempdir");
        let err = ensure_disk_headroom(dir.path(), u64::MAX).expect_err("guard should fail");
        assert!(err.to_string().contains("Insufficient disk space"));
    }
}
