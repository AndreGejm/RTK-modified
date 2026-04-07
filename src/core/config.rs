//! Reads user settings from config.toml.

use super::constants::{CONFIG_TOML, DEFAULT_HISTORY_DAYS, RTK_DATA_DIR};
use anyhow::Result;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::cell::RefCell;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub tracking: TrackingConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub filters: FilterConfig,
    #[serde(default)]
    pub tee: crate::core::tee::TeeConfig,
    #[serde(default)]
    pub telemetry: TelemetryConfig,
    #[serde(default)]
    pub evaluation: EvaluationConfig,
    #[serde(default)]
    pub hooks: HooksConfig,
    #[serde(default)]
    pub limits: LimitsConfig,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HooksConfig {
    /// Commands to exclude from auto-rewrite (e.g. ["curl", "playwright"]).
    /// Survives `rtk init -g` re-runs since config.toml is user-owned.
    #[serde(default)]
    pub exclude_commands: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrackingConfig {
    pub enabled: bool,
    pub history_days: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_path: Option<PathBuf>,
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            history_days: DEFAULT_HISTORY_DAYS as u32,
            database_path: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub colors: bool,
    pub emoji: bool,
    pub max_width: usize,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            colors: true,
            emoji: true,
            max_width: 120,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilterConfig {
    pub ignore_dirs: Vec<String>,
    pub ignore_files: Vec<String>,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            ignore_dirs: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(),
                "__pycache__".into(),
                ".venv".into(),
                "vendor".into(),
            ],
            ignore_files: vec!["*.lock".into(), "*.min.js".into(), "*.min.css".into()],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub enabled: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EvaluationConfig {
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LimitsConfig {
    /// Max total grep results to show (default: 200)
    pub grep_max_results: usize,
    /// Max matches per file in grep output (default: 25)
    pub grep_max_per_file: usize,
    /// Max staged/modified files shown in git status (default: 15)
    pub status_max_files: usize,
    /// Max untracked files shown in git status (default: 10)
    pub status_max_untracked: usize,
    /// Max chars for parser passthrough fallback (default: 2000)
    pub passthrough_max_chars: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            grep_max_results: 200,
            grep_max_per_file: 25,
            status_max_files: 15,
            status_max_untracked: 10,
            passthrough_max_chars: 2000,
        }
    }
}

/// Get limits config. Falls back to defaults if config can't be loaded.
pub fn limits() -> LimitsConfig {
    Config::load().map(|c| c.limits).unwrap_or_default()
}

/// Check if telemetry is enabled in config. Returns None if config can't be loaded.
pub fn telemetry_enabled() -> Option<bool> {
    Config::load().ok().map(|c| c.telemetry.enabled)
}

/// Check if evaluation logging is enabled in config. Returns None if config can't be loaded.
pub fn evaluation_enabled() -> Option<bool> {
    #[cfg(test)]
    if let Some(enabled) = evaluation_enabled_override() {
        return Some(enabled);
    }

    Config::load().ok().map(|c| c.evaluation.enabled)
}

#[cfg(test)]
thread_local! {
    static EVALUATION_ENABLED_OVERRIDE: RefCell<Option<bool>> = const { RefCell::new(None) };
    static CONFIG_PATH_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

#[cfg(test)]
fn evaluation_enabled_override() -> Option<bool> {
    EVALUATION_ENABLED_OVERRIDE.with(|slot| *slot.borrow())
}

#[cfg(test)]
pub(crate) struct EvaluationEnabledOverrideGuard {
    previous: Option<bool>,
}

#[cfg(test)]
impl EvaluationEnabledOverrideGuard {
    pub(crate) fn set(enabled: Option<bool>) -> Self {
        let previous = EVALUATION_ENABLED_OVERRIDE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let previous = *slot;
            *slot = enabled;
            previous
        });

        Self { previous }
    }
}

#[cfg(test)]
impl Drop for EvaluationEnabledOverrideGuard {
    fn drop(&mut self) {
        EVALUATION_ENABLED_OVERRIDE.with(|slot| {
            *slot.borrow_mut() = self.previous;
        });
    }
}

#[cfg(test)]
pub(crate) struct ConfigPathOverrideGuard {
    previous: Option<PathBuf>,
}

#[cfg(test)]
impl ConfigPathOverrideGuard {
    pub(crate) fn set(path: PathBuf) -> Self {
        let previous = CONFIG_PATH_OVERRIDE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let previous = slot.clone();
            *slot = Some(path);
            previous
        });

        Self { previous }
    }
}

#[cfg(test)]
impl Drop for ConfigPathOverrideGuard {
    fn drop(&mut self) {
        CONFIG_PATH_OVERRIDE.with(|slot| {
            *slot.borrow_mut() = self.previous.clone();
        });
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = get_config_path()?;

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = get_config_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn create_default() -> Result<PathBuf> {
        let config = Config::default();
        config.save()?;
        get_config_path()
    }
}

fn get_config_path() -> Result<PathBuf> {
    #[cfg(test)]
    if let Some(path) = CONFIG_PATH_OVERRIDE.with(|slot| slot.borrow().clone()) {
        return Ok(path.join(RTK_DATA_DIR).join(CONFIG_TOML));
    }

    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    Ok(config_dir.join(RTK_DATA_DIR).join(CONFIG_TOML))
}

pub fn show_config() -> Result<()> {
    let path = get_config_path()?;
    println!("Config: {}", path.display());
    println!();

    if path.exists() {
        let config = Config::load()?;
        println!("{}", toml::to_string_pretty(&config)?);
    } else {
        println!("(default config, file not created)");
        println!();
        let config = Config::default();
        println!("{}", toml::to_string_pretty(&config)?);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hooks_config_deserialize() {
        let toml = r#"
[hooks]
exclude_commands = ["curl", "gh"]
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        assert_eq!(config.hooks.exclude_commands, vec!["curl", "gh"]);
    }

    #[test]
    fn test_hooks_config_default_empty() {
        let config = Config::default();
        assert!(config.hooks.exclude_commands.is_empty());
    }

    #[test]
    fn test_config_without_hooks_section_is_valid() {
        let toml = r#"
[tracking]
enabled = true
history_days = 90
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        assert!(config.hooks.exclude_commands.is_empty());
    }

    #[test]
    fn test_evaluation_config_default_disabled() {
        let config = Config::default();
        assert!(!config.evaluation.enabled);
    }

    #[test]
    fn test_evaluation_config_deserializes() {
        let toml = r#"
[evaluation]
enabled = true
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        assert!(config.evaluation.enabled);
    }

    #[test]
    fn test_legacy_config_without_evaluation_section_defaults_disabled() {
        let toml = r#"
[tracking]
enabled = true
history_days = 90
"#;
        let config: Config = toml::from_str(toml).expect("valid toml");
        assert!(!config.evaluation.enabled);
    }

    #[test]
    fn test_evaluation_enabled_override_is_thread_local() {
        let config_root = std::env::temp_dir().join(format!(
            "rtk_config_override_scope_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before UNIX_EPOCH")
                .as_nanos()
        ));
        let _config_guard = ConfigPathOverrideGuard::set(config_root.clone());
        let _override = EvaluationEnabledOverrideGuard::set(Some(true));
        assert_eq!(evaluation_enabled(), Some(true));

        let handle = std::thread::spawn(move || {
            let _config_guard = ConfigPathOverrideGuard::set(config_root);
            evaluation_enabled()
        });

        assert_eq!(
            handle.join().expect("scope test thread panicked"),
            Some(false)
        );

        assert_eq!(evaluation_enabled(), Some(true));
    }
}
