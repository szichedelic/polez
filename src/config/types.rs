use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: String,
    pub paranoia_level: ParanoiaLevel,
    pub preserve_quality: QualityLevel,
    pub output_format: OutputFormat,
    pub backup_originals: bool,
    #[serde(default)]
    pub preset: Option<String>,
    pub audio_processing: AudioProcessingConfig,
    pub watermark_detection: Vec<DetectionMethod>,
    pub spectral_cleaning: SpectralCleaningConfig,
    pub metadata_cleaning: MetadataCleaningConfig,
    pub fingerprint_removal: FingerprintRemovalConfig,
    pub quality_preservation: QualityPreservationConfig,
    pub batch_processing: BatchProcessingConfig,
    pub verification: VerificationConfig,
    pub ui: UiConfig,
    pub formats: FormatConfig,
    #[serde(default)]
    pub advanced_flags: AdvancedFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParanoiaLevel {
    Low,
    Medium,
    High,
    Maximum,
}

impl std::fmt::Display for ParanoiaLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParanoiaLevel::Low => write!(f, "low"),
            ParanoiaLevel::Medium => write!(f, "medium"),
            ParanoiaLevel::High => write!(f, "high"),
            ParanoiaLevel::Maximum => write!(f, "maximum"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityLevel {
    Low,
    Medium,
    High,
    Maximum,
}

impl std::fmt::Display for QualityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityLevel::Low => write!(f, "low"),
            QualityLevel::Medium => write!(f, "medium"),
            QualityLevel::High => write!(f, "high"),
            QualityLevel::Maximum => write!(f, "maximum"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Preserve,
    Mp3,
    Wav,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Preserve => write!(f, "preserve"),
            OutputFormat::Mp3 => write!(f, "mp3"),
            OutputFormat::Wav => write!(f, "wav"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionMethod {
    SpreadSpectrum,
    EchoBased,
    Statistical,
    PhaseModulation,
    AmplitudeModulation,
    FrequencyDomain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioProcessingConfig {
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u16>,
    pub channels: String,
    pub normalize: bool,
    pub dithering: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralCleaningConfig {
    pub high_freq_cutoff: u32,
    pub notch_filter_q: u32,
    pub smoothing_window: u32,
    pub adaptive_noise: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataCleaningConfig {
    pub aggressive_mode: bool,
    pub preserve_date: bool,
    pub preserve_technical: bool,
    pub strip_binary_chunks: bool,
    pub remove_id3v1: bool,
    pub remove_id3v2: bool,
    pub remove_ape_tags: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintRemovalConfig {
    pub statistical_normalization: bool,
    pub temporal_randomization: bool,
    pub phase_randomization: bool,
    pub micro_timing_perturbation: bool,
    pub human_imperfections: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityPreservationConfig {
    pub target_snr: u32,
    pub max_quality_loss: u32,
    pub preserve_dynamics: bool,
    pub preserve_frequency_response: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProcessingConfig {
    pub workers: u32,
    pub progress_updates: bool,
    pub continue_on_error: bool,
    pub output_directory: Option<String>,
    pub naming_pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    pub auto_verify: bool,
    pub deep_analysis: bool,
    pub compare_spectra: bool,
    pub check_watermarks: bool,
    pub calculate_metrics: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub color_output: bool,
    pub unicode_symbols: bool,
    pub progress_bars: bool,
    pub detailed_output: bool,
    pub show_quotes: bool,
    pub ascii_art: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatConfig {
    pub mp3: Mp3Config,
    pub wav: WavConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mp3Config {
    pub bitrate: String,
    pub quality: u8,
    pub joint_stereo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavConfig {
    pub bit_depth: String,
    pub sample_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedFlags {
    pub phase_dither: bool,
    pub comb_mask: bool,
    pub transient_shift: bool,
    pub resample_nudge: bool,
    pub phase_noise: bool,
    pub phase_swirl: bool,
    pub masked_hf_phase: bool,
    pub gated_resample_nudge: bool,
    pub micro_eq_flutter: bool,
    pub hf_decorrelate: bool,
    pub refined_transient: bool,
    pub adaptive_transient: bool,
    pub adaptive_notch: bool,
}

impl Default for AdvancedFlags {
    fn default() -> Self {
        Self {
            phase_dither: true,
            comb_mask: true,
            transient_shift: true,
            resample_nudge: true,
            phase_noise: true,
            phase_swirl: true,
            masked_hf_phase: false,
            gated_resample_nudge: false,
            micro_eq_flutter: false,
            hf_decorrelate: false,
            refined_transient: false,
            adaptive_transient: false,
            adaptive_notch: false,
        }
    }
}
