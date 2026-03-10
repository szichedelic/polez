pub mod defaults;
pub mod manager;
pub mod types;

pub use manager::ConfigManager;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::defaults::{builtin_presets, default_config};
    use super::types::*;

    #[test]
    fn test_default_config_valid() {
        let config = default_config();
        assert_eq!(config.version, "2.0.0");
        assert!(matches!(config.paranoia_level, ParanoiaLevel::Medium));
        assert!(matches!(config.preserve_quality, QualityLevel::High));
        assert!(matches!(config.output_format, OutputFormat::Preserve));
        assert_eq!(config.watermark_detection.len(), 6);
    }

    #[test]
    fn test_default_advanced_flags() {
        let flags = AdvancedFlags::default();
        assert!(flags.phase_dither);
        assert!(flags.comb_mask);
        assert!(flags.transient_shift);
        assert!(flags.resample_nudge);
        assert!(flags.phase_noise);
        assert!(flags.phase_swirl);
        assert!(!flags.masked_hf_phase);
        assert!(!flags.gated_resample_nudge);
        assert!(!flags.adaptive_notch);
    }

    #[test]
    fn test_builtin_presets() {
        let presets = builtin_presets();
        assert_eq!(presets.len(), 5);
        let names: Vec<&str> = presets.iter().map(|p| p.name).collect();
        assert!(names.contains(&"stealth"));
        assert!(names.contains(&"stealth-plus"));
        assert!(names.contains(&"fast"));
        assert!(names.contains(&"quality"));
        assert!(names.contains(&"research"));
    }

    #[test]
    fn test_stealth_plus_has_custom_flags() {
        let presets = builtin_presets();
        let sp = presets.iter().find(|p| p.name == "stealth-plus").unwrap();
        assert!(sp.advanced_flags.is_some());
        let flags = sp.advanced_flags.as_ref().unwrap();
        assert!(flags.gated_resample_nudge);
        assert!(!flags.phase_dither);
    }

    #[test]
    fn test_default_spectral_config() {
        let config = default_config();
        assert_eq!(config.spectral_cleaning.high_freq_cutoff, 15000);
        assert_eq!(config.spectral_cleaning.notch_filter_q, 30);
        assert!(config.spectral_cleaning.adaptive_noise);
    }

    #[test]
    fn test_default_metadata_cleaning() {
        let config = default_config();
        assert!(config.metadata_cleaning.remove_id3v1);
        assert!(config.metadata_cleaning.remove_id3v2);
        assert!(config.metadata_cleaning.remove_ape_tags);
        assert!(config.metadata_cleaning.strip_binary_chunks);
    }
}
