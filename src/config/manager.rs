//! Configuration file I/O, validation, and preset management.
//!
//! [`ConfigManager`] handles loading/saving YAML config, applying environment
//! variable overrides, validating settings, and managing custom presets.

use std::path::{Path, PathBuf};

use crate::config::defaults::{builtin_presets, default_config};
use crate::config::types::AppConfig;
use crate::error::{PolezError, Result};

/// Manages configuration loading, saving, validation, and preset operations.
pub struct ConfigManager {
    /// Directory containing config and preset files.
    config_dir: PathBuf,
    /// Path to the main config YAML file.
    config_file: PathBuf,
    /// The currently loaded configuration.
    pub config: AppConfig,
    /// Log of environment variable overrides that were applied.
    pub env_overrides: Vec<String>,
}

impl ConfigManager {
    /// Create a new `ConfigManager`, loading config from disk and applying env overrides.
    pub fn new() -> Result<Self> {
        let config_dir = get_config_dir()?;
        let config_file = config_dir.join("config.yaml");
        let mut mgr = Self {
            config_dir,
            config_file,
            config: default_config(),
            env_overrides: Vec::new(),
        };
        mgr.load()?;
        mgr.apply_env_overrides();
        Ok(mgr)
    }

    /// Load configuration from the YAML file, falling back to defaults on error.
    pub fn load(&mut self) -> Result<()> {
        if self.config_file.exists() {
            let contents = std::fs::read_to_string(&self.config_file)
                .map_err(|e| PolezError::Config(format!("Failed to read config: {e}")))?;
            match serde_yaml::from_str::<AppConfig>(&contents) {
                Ok(loaded) => self.config = loaded,
                Err(e) => {
                    tracing::warn!("Config parse error ({e}), using defaults");
                    self.config = default_config();
                }
            }
        } else {
            self.config = default_config();
            self.save()?;
        }
        Ok(())
    }

    /// Apply environment variable overrides to the loaded config.
    /// Env vars override config file values but are overridden by CLI flags.
    fn apply_env_overrides(&mut self) {
        use crate::config::types::{OutputFormat, ParanoiaLevel, QualityLevel};

        if let Ok(val) = std::env::var("POLEZ_MODE") {
            match val.to_lowercase().as_str() {
                "fast" => {
                    self.config.paranoia_level = ParanoiaLevel::Low;
                    self.env_overrides
                        .push(format!("POLEZ_MODE={val} → paranoia_level=low"));
                }
                "standard" => {
                    self.config.paranoia_level = ParanoiaLevel::Medium;
                    self.env_overrides
                        .push(format!("POLEZ_MODE={val} → paranoia_level=medium"));
                }
                "preserving" => {
                    self.config.paranoia_level = ParanoiaLevel::High;
                    self.env_overrides
                        .push(format!("POLEZ_MODE={val} → paranoia_level=high"));
                }
                "aggressive" => {
                    self.config.paranoia_level = ParanoiaLevel::Maximum;
                    self.env_overrides
                        .push(format!("POLEZ_MODE={val} → paranoia_level=maximum"));
                }
                _ => tracing::warn!("Unknown POLEZ_MODE value: {val}"),
            }
        }

        if let Ok(val) = std::env::var("POLEZ_QUALITY") {
            match val.to_lowercase().as_str() {
                "low" => {
                    self.config.preserve_quality = QualityLevel::Low;
                    self.env_overrides
                        .push(format!("POLEZ_QUALITY={val} → preserve_quality=low"));
                }
                "medium" => {
                    self.config.preserve_quality = QualityLevel::Medium;
                    self.env_overrides
                        .push(format!("POLEZ_QUALITY={val} → preserve_quality=medium"));
                }
                "high" => {
                    self.config.preserve_quality = QualityLevel::High;
                    self.env_overrides
                        .push(format!("POLEZ_QUALITY={val} → preserve_quality=high"));
                }
                "maximum" => {
                    self.config.preserve_quality = QualityLevel::Maximum;
                    self.env_overrides
                        .push(format!("POLEZ_QUALITY={val} → preserve_quality=maximum"));
                }
                _ => tracing::warn!("Unknown POLEZ_QUALITY value: {val}"),
            }
        }

        if let Ok(val) = std::env::var("POLEZ_OUTPUT_FORMAT") {
            match val.to_lowercase().as_str() {
                "preserve" => {
                    self.config.output_format = OutputFormat::Preserve;
                    self.env_overrides.push(format!(
                        "POLEZ_OUTPUT_FORMAT={val} → output_format=preserve"
                    ));
                }
                "mp3" => {
                    self.config.output_format = OutputFormat::Mp3;
                    self.env_overrides
                        .push(format!("POLEZ_OUTPUT_FORMAT={val} → output_format=mp3"));
                }
                "wav" => {
                    self.config.output_format = OutputFormat::Wav;
                    self.env_overrides
                        .push(format!("POLEZ_OUTPUT_FORMAT={val} → output_format=wav"));
                }
                _ => tracing::warn!("Unknown POLEZ_OUTPUT_FORMAT value: {val}"),
            }
        }

        if let Ok(val) = std::env::var("POLEZ_PARANOID") {
            match val.to_lowercase().as_str() {
                "1" | "true" | "yes" => {
                    self.config.paranoia_level = ParanoiaLevel::Maximum;
                    self.env_overrides
                        .push(format!("POLEZ_PARANOID={val} → paranoia_level=maximum"));
                }
                "0" | "false" | "no" => {}
                _ => tracing::warn!("Unknown POLEZ_PARANOID value: {val}"),
            }
        }
    }

    /// Validate the current configuration and return a list of issues found.
    /// Errors are critical problems, warnings are potential issues.
    pub fn validate(&self) -> Vec<ConfigIssue> {
        let mut issues = Vec::new();

        // Validate spectral cleaning ranges
        if self.config.spectral_cleaning.high_freq_cutoff == 0 {
            issues.push(ConfigIssue::error(
                "spectral_cleaning.high_freq_cutoff",
                "must be greater than 0",
            ));
        }
        if self.config.spectral_cleaning.smoothing_window == 0 {
            issues.push(ConfigIssue::error(
                "spectral_cleaning.smoothing_window",
                "must be greater than 0",
            ));
        }

        // Validate quality preservation
        if self.config.quality_preservation.target_snr == 0 {
            issues.push(ConfigIssue::warning(
                "quality_preservation.target_snr",
                "target SNR of 0 may produce poor quality output",
            ));
        }

        // Validate batch processing
        if self.config.batch_processing.workers == 0 {
            issues.push(ConfigIssue::error(
                "batch_processing.workers",
                "must be at least 1",
            ));
        }

        // Validate naming pattern has placeholder
        if !self
            .config
            .batch_processing
            .naming_pattern
            .contains("{name}")
        {
            issues.push(ConfigIssue::warning(
                "batch_processing.naming_pattern",
                "pattern should contain {name} placeholder for the original filename",
            ));
        }

        // Validate mp3 quality
        if self.config.formats.mp3.quality > 9 {
            issues.push(ConfigIssue::error(
                "formats.mp3.quality",
                "must be 0-9 (0=best, 9=worst)",
            ));
        }

        issues
    }

    /// Check the raw YAML for unknown fields and return warnings.
    pub fn check_unknown_fields(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if !self.config_file.exists() {
            return warnings;
        }

        let contents = match std::fs::read_to_string(&self.config_file) {
            Ok(c) => c,
            Err(_) => return warnings,
        };

        let raw: serde_yaml::Value = match serde_yaml::from_str(&contents) {
            Ok(v) => v,
            Err(_) => return warnings,
        };

        let known_top_level = [
            "version",
            "paranoia_level",
            "preserve_quality",
            "output_format",
            "backup_originals",
            "preset",
            "audio_processing",
            "watermark_detection",
            "spectral_cleaning",
            "metadata_cleaning",
            "fingerprint_removal",
            "quality_preservation",
            "batch_processing",
            "verification",
            "ui",
            "formats",
            "advanced_flags",
        ];

        if let Some(mapping) = raw.as_mapping() {
            for (key, _) in mapping {
                if let Some(key_str) = key.as_str() {
                    if !known_top_level.contains(&key_str) {
                        let suggestion = find_closest_match(key_str, &known_top_level);
                        let msg = if let Some(closest) = suggestion {
                            format!("Unknown config field '{key_str}' — did you mean '{closest}'?")
                        } else {
                            format!("Unknown config field '{key_str}'")
                        };
                        warnings.push(msg);
                    }
                }
            }
        }

        warnings
    }

    /// Save the current configuration to the YAML file.
    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.config_dir)
            .map_err(|e| PolezError::Config(format!("Failed to create config dir: {e}")))?;
        let yaml = serde_yaml::to_string(&self.config)
            .map_err(|e| PolezError::Config(format!("Config serialize error: {e}")))?;
        std::fs::write(&self.config_file, yaml)
            .map_err(|e| PolezError::Config(format!("Failed to write config: {e}")))?;
        Ok(())
    }

    /// Reset configuration to factory defaults and save.
    pub fn reset_to_defaults(&mut self) -> Result<()> {
        self.config = default_config();
        self.save()
    }

    /// Apply a built-in or custom preset by name, saving the result.
    pub fn apply_preset(&mut self, name: &str) -> Result<()> {
        // Check built-in presets
        if let Some(preset) = builtin_presets().into_iter().find(|p| p.name == name) {
            self.config.paranoia_level = preset.paranoia_level;
            self.config.preserve_quality = preset.preserve_quality;
            if let Some(flags) = preset.advanced_flags {
                self.config.advanced_flags = flags;
            }
            self.config.preset = Some(name.to_string());
            self.save()?;
            return Ok(());
        }

        // Try custom preset file
        let preset_path = self.config_dir.join(format!("preset_{name}.yaml"));
        if preset_path.exists() {
            let contents = std::fs::read_to_string(&preset_path)
                .map_err(|e| PolezError::Config(format!("Failed to read preset: {e}")))?;
            let loaded: AppConfig = serde_yaml::from_str(&contents)
                .map_err(|e| PolezError::Config(format!("Preset parse error: {e}")))?;
            self.config = loaded;
            self.config.preset = Some(name.to_string());
            self.save()?;
            return Ok(());
        }

        Err(PolezError::Config(format!("Preset '{name}' not found")))
    }

    /// Save a configuration as a custom preset file.
    pub fn create_preset(&self, name: &str, config: &AppConfig) -> Result<()> {
        let preset_path = self.config_dir.join(format!("preset_{name}.yaml"));
        let yaml = serde_yaml::to_string(config)
            .map_err(|e| PolezError::Config(format!("Serialize error: {e}")))?;
        std::fs::write(&preset_path, yaml)
            .map_err(|e| PolezError::Config(format!("Failed to write preset: {e}")))?;
        Ok(())
    }

    /// Delete a custom preset file. Built-in presets cannot be deleted.
    pub fn delete_preset(&self, name: &str) -> Result<()> {
        // Don't allow deleting built-in presets
        if builtin_presets().iter().any(|p| p.name == name) {
            return Err(PolezError::Config(format!(
                "Cannot delete built-in preset: {name}"
            )));
        }
        let preset_path = self.config_dir.join(format!("preset_{name}.yaml"));
        if preset_path.exists() {
            std::fs::remove_file(&preset_path)
                .map_err(|e| PolezError::Config(format!("Failed to delete preset: {e}")))?;
            Ok(())
        } else {
            Err(PolezError::Config(format!("Preset '{name}' not found")))
        }
    }

    /// List names of all user-created custom presets.
    pub fn list_custom_presets(&self) -> Vec<String> {
        let mut presets = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.config_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("preset_") && name.ends_with(".yaml") {
                    let preset_name = name
                        .strip_prefix("preset_")
                        .unwrap()
                        .strip_suffix(".yaml")
                        .unwrap()
                        .to_string();
                    presets.push(preset_name);
                }
            }
        }
        presets
    }

    /// Return the path to the configuration directory.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }
}

/// A validation issue found in the config.
pub struct ConfigIssue {
    /// Dot-separated path to the offending config field.
    pub field: String,
    /// Human-readable description of the issue.
    pub message: String,
    /// `true` for critical errors, `false` for warnings.
    pub is_error: bool,
}

impl ConfigIssue {
    fn error(field: &str, message: &str) -> Self {
        Self {
            field: field.to_string(),
            message: message.to_string(),
            is_error: true,
        }
    }

    fn warning(field: &str, message: &str) -> Self {
        Self {
            field: field.to_string(),
            message: message.to_string(),
            is_error: false,
        }
    }
}

/// Find the closest matching string using edit distance.
fn find_closest_match<'a>(input: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let mut best: Option<(&str, usize)> = None;
    for &candidate in candidates {
        let dist = edit_distance(input, candidate);
        // Only suggest if within 3 edits
        if dist <= 3 && (best.is_none() || dist < best.unwrap().1) {
            best = Some((candidate, dist));
        }
    }
    best.map(|(s, _)| s)
}

/// Simple Levenshtein edit distance.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in dp.iter_mut().enumerate().take(a.len() + 1) {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate().take(b.len() + 1) {
        *val = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[a.len()][b.len()]
}

/// Determine the platform-appropriate configuration directory and ensure it exists.
fn get_config_dir() -> Result<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        dirs::data_local_dir()
    } else if cfg!(target_os = "macos") {
        dirs::data_dir()
    } else {
        dirs::config_dir()
    };

    let dir = base
        .ok_or_else(|| PolezError::Config("Cannot determine config directory".to_string()))?
        .join("polez");

    std::fs::create_dir_all(&dir)
        .map_err(|e| PolezError::Config(format!("Cannot create config dir: {e}")))?;

    Ok(dir)
}
