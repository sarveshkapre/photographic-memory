use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureDecision {
    Allow,
    Skip { reason: String },
}

#[derive(Debug, Clone)]
pub struct PrivacyStatus {
    pub config_path: PathBuf,
    pub enabled: bool,
    pub rule_summary: String,
}

#[async_trait]
pub trait PrivacyGuard: Send + Sync {
    async fn decision(&self) -> CaptureDecision;
    fn status(&self) -> PrivacyStatus;
    fn reload(&self) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct AllowAllPrivacyGuard {
    config_path: PathBuf,
}

impl AllowAllPrivacyGuard {
    pub fn new(config_path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
        }
    }
}

#[async_trait]
impl PrivacyGuard for AllowAllPrivacyGuard {
    async fn decision(&self) -> CaptureDecision {
        CaptureDecision::Allow
    }

    fn status(&self) -> PrivacyStatus {
        PrivacyStatus {
            config_path: self.config_path.clone(),
            enabled: false,
            rule_summary: "disabled".to_string(),
        }
    }

    fn reload(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ForegroundAppSnapshot {
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub browser_private_window: Option<bool>,
}

#[async_trait]
pub trait ForegroundAppProvider: Send + Sync {
    async fn foreground_app(&self) -> Result<ForegroundAppSnapshot>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MacOsForegroundAppProvider;

#[async_trait]
impl ForegroundAppProvider for MacOsForegroundAppProvider {
    async fn foreground_app(&self) -> Result<ForegroundAppSnapshot> {
        let output = run_osascript(
            r#"
tell application "System Events"
    set frontApp to first application process whose frontmost is true
    set frontName to name of frontApp
    set frontBundle to ""
    try
        set frontBundle to bundle identifier of frontApp
    on error
        set frontBundle to ""
    end try
end tell

set privateMode to ""
if frontName is "Google Chrome" or frontName is "Google Chrome Canary" or frontName is "Brave Browser" or frontName is "Microsoft Edge" or frontName is "Chromium" then
    tell application "System Events"
        if exists process frontName then
            tell application frontName
                try
                    set privateMode to (mode of front window) as string
                on error
                    set privateMode to ""
                end try
            end tell
        end if
    end tell
end if

return frontName & "\n" & frontBundle & "\n" & privateMode
"#,
        )
        .await
        .context("failed to query foreground app via AppleScript")?;

        let mut lines = output.lines();
        let app_name = lines.next().unwrap_or("Unknown").trim().to_string();
        let bundle_id = lines
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let browser_private_window = lines
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(|mode| match mode.to_ascii_lowercase().as_str() {
                "incognito" => Some(true),
                "normal" => Some(false),
                _ => None,
            });

        Ok(ForegroundAppSnapshot {
            app_name,
            bundle_id,
            browser_private_window,
        })
    }
}

async fn run_osascript(script: &str) -> Result<String> {
    // NOTE: We keep AppleScript narrow and avoid querying window titles/URLs to reduce sensitivity.
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("failed to spawn osascript")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(anyhow!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[derive(Debug, Clone, Default)]
struct PrivacyPolicy {
    deny_apps: Vec<String>,
    allow_override: Vec<String>,
    deny_browser_private_windows: bool,
}

impl PrivacyPolicy {
    fn decision_for(&self, foreground: &ForegroundAppSnapshot) -> CaptureDecision {
        let app_name = foreground.app_name.to_ascii_lowercase();
        let bundle = foreground
            .bundle_id
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();

        if matches_any(&app_name, &bundle, &self.allow_override) {
            return CaptureDecision::Allow;
        }

        if matches_any(&app_name, &bundle, &self.deny_apps) {
            return CaptureDecision::Skip {
                reason: "privacy: denied foreground app".to_string(),
            };
        }

        if self.deny_browser_private_windows
            && matches!(foreground.browser_private_window, Some(true))
        {
            return CaptureDecision::Skip {
                reason: "privacy: browser private window".to_string(),
            };
        }

        CaptureDecision::Allow
    }
}

fn matches_any(app_name_lower: &str, bundle_lower: &str, rules: &[String]) -> bool {
    rules.iter().any(|raw| {
        let rule = raw.trim().to_ascii_lowercase();
        if rule.is_empty() {
            return false;
        }
        if rule.contains('.') {
            rule == bundle_lower
        } else {
            rule == app_name_lower
        }
    })
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PrivacyConfigFile {
    #[serde(default)]
    deny: DenySection,
    #[serde(default)]
    allow: AllowSection,
}

#[derive(Debug, Clone, Deserialize)]
struct DenySection {
    #[serde(default)]
    apps: Vec<String>,
    #[serde(default = "default_true")]
    browser_private_windows: bool,
    // Parsed for forwards-compatibility, not enforced yet.
    #[allow(dead_code)]
    #[serde(default)]
    domains: Vec<String>,
}

impl Default for DenySection {
    fn default() -> Self {
        Self {
            apps: Vec::new(),
            browser_private_windows: true,
            domains: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct AllowSection {
    #[serde(rename = "override", default)]
    override_apps: Vec<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug)]
struct CachedPolicy {
    mtime: Option<SystemTime>,
    policy: PrivacyPolicy,
}

#[derive(Debug)]
pub struct ConfigPrivacyGuard<P: ForegroundAppProvider> {
    config_path: PathBuf,
    provider: P,
    cached: Mutex<CachedPolicy>,
    foreground_timeout: Duration,
}

impl<P: ForegroundAppProvider> ConfigPrivacyGuard<P> {
    pub fn new(config_path: impl Into<PathBuf>, provider: P) -> Self {
        let config_path = config_path.into();
        Self {
            config_path: config_path.clone(),
            provider,
            cached: Mutex::new(CachedPolicy {
                mtime: None,
                policy: PrivacyPolicy::default(),
            }),
            // Keep this bounded so AppleScript can't stall capture loops.
            foreground_timeout: Duration::from_millis(250),
        }
    }

    fn load_policy_from_disk(&self) -> Result<PrivacyPolicy> {
        let path = &self.config_path;
        if !path.exists() {
            return Ok(PrivacyPolicy::default());
        }

        let bytes = std::fs::read(path).with_context(|| {
            format!(
                "failed to read privacy config {}",
                self.config_path.display()
            )
        })?;

        let text = std::str::from_utf8(&bytes)
            .with_context(|| format!("privacy config {} is not valid UTF-8", path.display()))?;

        let file: PrivacyConfigFile = toml::from_str(text).with_context(|| {
            format!(
                "failed to parse privacy config {} (expected TOML)",
                self.config_path.display()
            )
        })?;

        Ok(PrivacyPolicy {
            deny_apps: file.deny.apps,
            allow_override: file.allow.override_apps,
            deny_browser_private_windows: file.deny.browser_private_windows,
        })
    }

    fn reload_if_needed(&self) -> Result<()> {
        let meta = std::fs::metadata(&self.config_path).ok();
        let mtime = meta.and_then(|m| m.modified().ok()).or_else(|| {
            if self.config_path.exists() {
                Some(SystemTime::UNIX_EPOCH)
            } else {
                None
            }
        });

        let mut cached = self.cached.lock().expect("privacy policy mutex poisoned");
        if cached.mtime == mtime {
            return Ok(());
        }

        cached.policy = self.load_policy_from_disk()?;
        cached.mtime = mtime;
        Ok(())
    }

    fn cached_policy(&self) -> PrivacyPolicy {
        self.cached
            .lock()
            .expect("privacy policy mutex poisoned")
            .policy
            .clone()
    }

    pub fn ensure_sample_config_exists(&self) -> Result<()> {
        ensure_sample_privacy_config(&self.config_path)
    }
}

#[async_trait]
impl<P: ForegroundAppProvider> PrivacyGuard for ConfigPrivacyGuard<P> {
    async fn decision(&self) -> CaptureDecision {
        if let Err(err) = self.reload_if_needed() {
            return CaptureDecision::Skip {
                reason: format!("privacy: config error ({err})"),
            };
        }

        let foreground =
            match tokio::time::timeout(self.foreground_timeout, self.provider.foreground_app())
                .await
            {
                Ok(Ok(value)) => value,
                Ok(Err(err)) => {
                    return CaptureDecision::Skip {
                        reason: format!("privacy: foreground check failed ({err})"),
                    };
                }
                Err(_) => {
                    return CaptureDecision::Skip {
                        reason: "privacy: foreground check timed out".to_string(),
                    };
                }
            };

        let policy = self.cached_policy();
        policy.decision_for(&foreground)
    }

    fn status(&self) -> PrivacyStatus {
        let policy = self.cached_policy();
        let enabled = !policy.deny_apps.is_empty() || policy.deny_browser_private_windows;
        let mut parts = Vec::new();
        if !policy.deny_apps.is_empty() {
            parts.push(format!("{} denied app rules", policy.deny_apps.len()));
        }
        if policy.deny_browser_private_windows {
            parts.push("private windows excluded".to_string());
        }

        PrivacyStatus {
            config_path: self.config_path.clone(),
            enabled,
            rule_summary: if parts.is_empty() {
                "disabled".to_string()
            } else {
                parts.join(", ")
            },
        }
    }

    fn reload(&self) -> Result<()> {
        // Force refresh regardless of mtime check.
        let mut cached = self.cached.lock().expect("privacy policy mutex poisoned");
        cached.policy = self.load_policy_from_disk()?;
        cached.mtime = std::fs::metadata(&self.config_path)
            .and_then(|m| m.modified())
            .ok();
        Ok(())
    }
}

pub fn ensure_sample_privacy_config(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create privacy config parent directory {}",
                parent.display()
            )
        })?;
    }

    // Keep this sample minimal and explicitly avoid window titles/URLs.
    // Safari private-mode detection is not reliable via AppleScript; deny Safari explicitly if needed.
    let sample = r#"# Photographic Memory Privacy Policy (TOML)
#
# This file controls which capture ticks are skipped before a screenshot is taken.
# Skipped events are logged as a rule-only reason; window titles and URLs are never recorded here.

[deny]
# App names (e.g. "1Password") OR bundle IDs (e.g. "com.apple.KeychainAccess").
apps = [
  "1Password",
  "com.apple.KeychainAccess",
]

# When supported, skip captures when the foreground browser is in a private/incognito window.
# Supported (best-effort): Google Chrome, Brave, Edge, Chromium.
browser_private_windows = true

[allow]
# Override rules that always allow capture even if they match deny apps or private-window checks.
override = []
"#;

    std::fs::write(path, sample)
        .with_context(|| format!("failed to write sample privacy config {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureDecision, ConfigPrivacyGuard, ForegroundAppProvider, ForegroundAppSnapshot,
        PrivacyGuard,
    };
    use anyhow::Result;
    use async_trait::async_trait;
    use std::path::Path;
    use tempfile::tempdir;

    #[derive(Debug, Clone)]
    struct StaticForeground {
        snapshot: ForegroundAppSnapshot,
    }

    #[async_trait]
    impl ForegroundAppProvider for StaticForeground {
        async fn foreground_app(&self) -> Result<ForegroundAppSnapshot> {
            Ok(self.snapshot.clone())
        }
    }

    #[tokio::test]
    async fn default_policy_allows_capture_when_no_config_file_exists() {
        let temp = tempdir().expect("tempdir");
        let config_path = temp.path().join("privacy.toml");
        let guard = ConfigPrivacyGuard::new(
            &config_path,
            StaticForeground {
                snapshot: ForegroundAppSnapshot {
                    app_name: "Finder".to_string(),
                    bundle_id: Some("com.apple.finder".to_string()),
                    browser_private_window: None,
                },
            },
        );

        assert_eq!(guard.decision().await, CaptureDecision::Allow);
    }

    #[tokio::test]
    async fn deny_app_rule_skips_capture_without_leaking_titles_or_urls() {
        let temp = tempdir().expect("tempdir");
        let config_path = temp.path().join("privacy.toml");
        std::fs::write(
            &config_path,
            r#"
[deny]
apps = ["com.apple.KeychainAccess"]
browser_private_windows = true
"#,
        )
        .expect("write config");

        let guard = ConfigPrivacyGuard::new(
            &config_path,
            StaticForeground {
                snapshot: ForegroundAppSnapshot {
                    app_name: "Keychain Access".to_string(),
                    bundle_id: Some("com.apple.KeychainAccess".to_string()),
                    browser_private_window: None,
                },
            },
        );

        match guard.decision().await {
            CaptureDecision::Skip { reason } => assert!(reason.starts_with("privacy:")),
            other => panic!("expected skip decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn allow_override_wins_over_deny_app() {
        let temp = tempdir().expect("tempdir");
        let config_path = temp.path().join("privacy.toml");
        std::fs::write(
            &config_path,
            r#"
[deny]
apps = ["Keychain Access"]
browser_private_windows = true

[allow]
override = ["Keychain Access"]
"#,
        )
        .expect("write config");

        let guard = ConfigPrivacyGuard::new(
            &config_path,
            StaticForeground {
                snapshot: ForegroundAppSnapshot {
                    app_name: "Keychain Access".to_string(),
                    bundle_id: Some("com.apple.KeychainAccess".to_string()),
                    browser_private_window: None,
                },
            },
        );

        assert_eq!(guard.decision().await, CaptureDecision::Allow);
    }

    #[tokio::test]
    async fn ensure_sample_config_writes_file() {
        let temp = tempdir().expect("tempdir");
        let config_path = temp.path().join("privacy.toml");
        super::ensure_sample_privacy_config(Path::new(&config_path)).expect("ensure sample");
        assert!(config_path.exists());
    }
}
