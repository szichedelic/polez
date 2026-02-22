use crate::config::types::*;

pub fn default_config() -> AppConfig {
    AppConfig {
        version: "2.0.0".to_string(),
        paranoia_level: ParanoiaLevel::Medium,
        preserve_quality: QualityLevel::High,
        output_format: OutputFormat::Preserve,
        backup_originals: false,
        preset: None,
        audio_processing: AudioProcessingConfig {
            sample_rate: None,
            bit_depth: None,
            channels: "preserve".to_string(),
            normalize: true,
            dithering: true,
        },
        watermark_detection: vec![
            DetectionMethod::SpreadSpectrum,
            DetectionMethod::EchoBased,
            DetectionMethod::Statistical,
            DetectionMethod::PhaseModulation,
            DetectionMethod::AmplitudeModulation,
            DetectionMethod::FrequencyDomain,
        ],
        spectral_cleaning: SpectralCleaningConfig {
            high_freq_cutoff: 15000,
            notch_filter_q: 30,
            smoothing_window: 5,
            adaptive_noise: true,
        },
        metadata_cleaning: MetadataCleaningConfig {
            aggressive_mode: false,
            preserve_date: false,
            preserve_technical: false,
            strip_binary_chunks: true,
            remove_id3v1: true,
            remove_id3v2: true,
            remove_ape_tags: true,
        },
        fingerprint_removal: FingerprintRemovalConfig {
            statistical_normalization: true,
            temporal_randomization: true,
            phase_randomization: false,
            micro_timing_perturbation: true,
            human_imperfections: false,
        },
        quality_preservation: QualityPreservationConfig {
            target_snr: 40,
            max_quality_loss: 5,
            preserve_dynamics: true,
            preserve_frequency_response: true,
        },
        batch_processing: BatchProcessingConfig {
            workers: 4,
            progress_updates: true,
            continue_on_error: false,
            output_directory: None,
            naming_pattern: "{name}_clean{ext}".to_string(),
        },
        verification: VerificationConfig {
            auto_verify: true,
            deep_analysis: false,
            compare_spectra: true,
            check_watermarks: true,
            calculate_metrics: true,
        },
        ui: UiConfig {
            color_output: true,
            unicode_symbols: true,
            progress_bars: true,
            detailed_output: false,
            show_quotes: true,
            ascii_art: true,
        },
        formats: FormatConfig {
            mp3: Mp3Config {
                bitrate: "preserve".to_string(),
                quality: 2,
                joint_stereo: true,
            },
            wav: WavConfig {
                bit_depth: "preserve".to_string(),
                sample_format: "pcm".to_string(),
            },
        },
        advanced_flags: AdvancedFlags::default(),
    }
}

/// Built-in preset: partial config overrides.
/// Returns (paranoia_level, preserve_quality, advanced_flags_override, description).
pub struct PresetDef {
    pub name: &'static str,
    pub description: &'static str,
    pub paranoia_level: ParanoiaLevel,
    pub preserve_quality: QualityLevel,
    pub advanced_flags: Option<AdvancedFlags>,
}

pub fn builtin_presets() -> Vec<PresetDef> {
    vec![
        PresetDef {
            name: "stealth",
            description: "Maximum paranoia, quality preservation",
            paranoia_level: ParanoiaLevel::Maximum,
            preserve_quality: QualityLevel::Maximum,
            advanced_flags: None,
        },
        PresetDef {
            name: "stealth-plus",
            description: "Stealth with advanced flags optimized for detector evasion",
            paranoia_level: ParanoiaLevel::Maximum,
            preserve_quality: QualityLevel::Maximum,
            advanced_flags: Some(AdvancedFlags {
                phase_dither: false,
                comb_mask: false,
                transient_shift: false,
                phase_swirl: false,
                masked_hf_phase: false,
                resample_nudge: false,
                gated_resample_nudge: true,
                phase_noise: true,
                micro_eq_flutter: false,
                hf_decorrelate: false,
                refined_transient: false,
                adaptive_transient: false,
                adaptive_notch: false,
            }),
        },
        PresetDef {
            name: "fast",
            description: "Quick processing, basic cleaning",
            paranoia_level: ParanoiaLevel::Low,
            preserve_quality: QualityLevel::Medium,
            advanced_flags: None,
        },
        PresetDef {
            name: "quality",
            description: "Preserve maximum audio quality",
            paranoia_level: ParanoiaLevel::Medium,
            preserve_quality: QualityLevel::Maximum,
            advanced_flags: None,
        },
        PresetDef {
            name: "research",
            description: "Deep analysis, detailed logging",
            paranoia_level: ParanoiaLevel::High,
            preserve_quality: QualityLevel::High,
            advanced_flags: None,
        },
    ]
}
