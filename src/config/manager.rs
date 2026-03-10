use std::path::{Path, PathBuf};

use crate::config::defaults::{builtin_presets, default_config};
use crate::config::types::AppConfig;
use crate::error::{PolezError, Result};

pub struct ConfigManager {
    config_dir: PathBuf,
    config_file: PathBuf,
    pub config: AppConfig,
    pub env_overrides: Vec<String>,
}

impl ConfigManager {
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

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.config_dir)
            .map_err(|e| PolezError::Config(format!("Failed to create config dir: {e}")))?;
        let yaml = serde_yaml::to_string(&self.config)
            .map_err(|e| PolezError::Config(format!("Config serialize error: {e}")))?;
        std::fs::write(&self.config_file, yaml)
            .map_err(|e| PolezError::Config(format!("Failed to write config: {e}")))?;
        Ok(())
    }

    pub fn reset_to_defaults(&mut self) -> Result<()> {
        self.config = default_config();
        self.save()
    }

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

    pub fn create_preset(&self, name: &str, config: &AppConfig) -> Result<()> {
        let preset_path = self.config_dir.join(format!("preset_{name}.yaml"));
        let yaml = serde_yaml::to_string(config)
            .map_err(|e| PolezError::Config(format!("Serialize error: {e}")))?;
        std::fs::write(&preset_path, yaml)
            .map_err(|e| PolezError::Config(format!("Failed to write preset: {e}")))?;
        Ok(())
    }

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

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }
}

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
