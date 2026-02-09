use std::path::PathBuf;

pub fn default_data_dir() -> PathBuf {
    match std::env::var_os("HOME") {
        Some(home) => {
            let path = PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("photographic-memory");
            let _ = std::fs::create_dir_all(&path);
            path
        }
        None => PathBuf::from("."),
    }
}

pub fn default_privacy_config_path() -> PathBuf {
    default_data_dir().join("privacy.toml")
}
