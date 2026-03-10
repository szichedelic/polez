//! Configuration types and data structures for all polez settings.
//!
//! These types are serialized to/from YAML for persistent configuration.

use serde::{Deserialize, Serialize};

/// Top-level application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Configuration schema version.
    pub version: String,
    /// Controls processing intensity and number of passes.
    pub paranoia_level: ParanoiaLevel,
    /// Target quality preservation level.
    pub preserve_quality: QualityLevel,
    /// Output audio format (or preserve original).
    pub output_format: OutputFormat,
    /// Whether to keep backup copies of original files.
    pub backup_originals: bool,
    /// Name of the active preset, if any.
    #[serde(default)]
    pub preset: Option<String>,
    /// Audio I/O and sample format settings.
    pub audio_processing: AudioProcessingConfig,
    /// Enabled watermark detection algorithms.
    pub watermark_detection: Vec<DetectionMethod>,
    /// Frequency-domain cleaning parameters.
    pub spectral_cleaning: SpectralCleaningConfig,
    /// Tag and metadata stripping options.
    pub metadata_cleaning: MetadataCleaningConfig,
    /// Statistical fingerprint removal settings.
    pub fingerprint_removal: FingerprintRemovalConfig,
    /// SNR and dynamics preservation targets.
    pub quality_preservation: QualityPreservationConfig,
    /// Parallel batch processing options.
    pub batch_processing: BatchProcessingConfig,
    /// Post-processing verification settings.
    pub verification: VerificationConfig,
    /// Console UI display preferences.
    pub ui: UiConfig,
    /// Format-specific encoding options.
    pub formats: FormatConfig,
    /// Toggles for individual stealth DSP operations.
    #[serde(default)]
    pub advanced_flags: AdvancedFlags,
}

/// Processing intensity level controlling sanitization depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParanoiaLevel {
    /// Minimal processing, fastest execution.
    Low,
    /// Balanced processing (default).
    Medium,
    /// Thorough processing with additional passes.
    High,
    /// Most aggressive processing with multi-pass paranoid mode.
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

/// Audio quality preservation target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityLevel {
    /// Allow significant quality degradation for stronger sanitization.
    Low,
    /// Moderate quality preservation.
    Medium,
    /// Prioritize audio quality (default).
    High,
    /// Preserve maximum fidelity, limiting aggressive operations.
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

/// Output audio file format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Keep the same format as the input file.
    Preserve,
    /// Encode output as MP3.
    Mp3,
    /// Encode output as WAV.
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

/// Watermark detection algorithm identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionMethod {
    /// Detect spread-spectrum watermarks embedded across the frequency range.
    SpreadSpectrum,
    /// Detect echo-based watermarks using autocorrelation analysis.
    EchoBased,
    /// Detect fingerprints via statistical distribution anomalies.
    Statistical,
    /// Detect phase-modulation watermarks.
    PhaseModulation,
    /// Detect amplitude-modulation watermarks.
    AmplitudeModulation,
    /// Detect frequency-domain embedded patterns.
    FrequencyDomain,
}

/// Audio I/O and sample format configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioProcessingConfig {
    /// Target sample rate in Hz, or `None` to preserve original.
    pub sample_rate: Option<u32>,
    /// Target bit depth, or `None` to preserve original.
    pub bit_depth: Option<u16>,
    /// Channel layout: "preserve", "mono", or "stereo".
    pub channels: String,
    /// Whether to normalize audio levels after processing.
    pub normalize: bool,
    /// Whether to apply dithering when reducing bit depth.
    pub dithering: bool,
}

/// Frequency-domain spectral cleaning parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralCleaningConfig {
    /// Upper frequency limit in Hz for watermark scanning.
    pub high_freq_cutoff: u32,
    /// Q factor for notch filters applied to detected watermark frequencies.
    pub notch_filter_q: u32,
    /// Smoothing window size in frames for spectral operations.
    pub smoothing_window: u32,
    /// Enable adaptive noise floor estimation.
    pub adaptive_noise: bool,
}

/// Metadata and tag stripping options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataCleaningConfig {
    /// Strip all metadata including non-standard chunks.
    pub aggressive_mode: bool,
    /// Keep date/time tags when stripping metadata.
    pub preserve_date: bool,
    /// Keep technical tags (sample rate, bit depth, etc.).
    pub preserve_technical: bool,
    /// Remove non-standard binary chunks from containers.
    pub strip_binary_chunks: bool,
    /// Remove ID3v1 tags.
    pub remove_id3v1: bool,
    /// Remove ID3v2 tags.
    pub remove_id3v2: bool,
    /// Remove APE tags.
    pub remove_ape_tags: bool,
}

/// Statistical fingerprint removal settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintRemovalConfig {
    /// Normalize statistical distributions to remove fingerprint signatures.
    pub statistical_normalization: bool,
    /// Randomize temporal patterns to break time-domain fingerprints.
    pub temporal_randomization: bool,
    /// Randomize phase information across frequency bins.
    pub phase_randomization: bool,
    /// Apply subtle timing jitter to defeat sample-accurate matching.
    pub micro_timing_perturbation: bool,
    /// Inject natural-sounding imperfections to mask processing artifacts.
    pub human_imperfections: bool,
}

/// Quality preservation constraints for sanitization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityPreservationConfig {
    /// Target signal-to-noise ratio in dB.
    pub target_snr: u32,
    /// Maximum acceptable quality loss as a percentage.
    pub max_quality_loss: u32,
    /// Preserve dynamic range during processing.
    pub preserve_dynamics: bool,
    /// Preserve frequency response characteristics.
    pub preserve_frequency_response: bool,
}

/// Parallel batch processing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProcessingConfig {
    /// Number of parallel worker threads.
    pub workers: u32,
    /// Show progress updates during batch processing.
    pub progress_updates: bool,
    /// Continue processing remaining files when one fails.
    pub continue_on_error: bool,
    /// Output directory override, or `None` to use input directory.
    pub output_directory: Option<String>,
    /// Output filename pattern with `{name}` and `{ext}` placeholders.
    pub naming_pattern: String,
}

/// Post-processing verification settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Automatically verify output after sanitization.
    pub auto_verify: bool,
    /// Run deep statistical analysis during verification.
    pub deep_analysis: bool,
    /// Compare before/after spectral characteristics.
    pub compare_spectra: bool,
    /// Re-check for watermarks in the output file.
    pub check_watermarks: bool,
    /// Calculate quality metrics (SNR, spectral difference, etc.).
    pub calculate_metrics: bool,
}

/// Console UI display preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Enable colored terminal output.
    pub color_output: bool,
    /// Use Unicode symbols in output.
    pub unicode_symbols: bool,
    /// Show progress bars during processing.
    pub progress_bars: bool,
    /// Print verbose/detailed output.
    pub detailed_output: bool,
    /// Display motivational quotes in banners.
    pub show_quotes: bool,
    /// Show ASCII art banners.
    pub ascii_art: bool,
}

/// Format-specific encoding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatConfig {
    /// MP3 encoding settings.
    pub mp3: Mp3Config,
    /// WAV encoding settings.
    pub wav: WavConfig,
}

/// MP3 encoding parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mp3Config {
    /// Target bitrate (e.g., "320", "256") or "preserve".
    pub bitrate: String,
    /// LAME quality setting (0=best, 9=worst).
    pub quality: u8,
    /// Use joint stereo encoding.
    pub joint_stereo: bool,
}

/// WAV encoding parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavConfig {
    /// Target bit depth (e.g., "16", "24") or "preserve".
    pub bit_depth: String,
    /// Sample format: "pcm" or "float".
    pub sample_format: String,
}

/// Toggles for individual stealth DSP operations.
///
/// Each flag enables or disables a specific processing step in the stealth pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedFlags {
    /// Apply random phase dithering to frequency bins.
    pub phase_dither: bool,
    /// Apply comb filter masking to obscure spectral patterns.
    pub comb_mask: bool,
    /// Shift transient positions by micro-amounts.
    pub transient_shift: bool,
    /// Apply subtle sample rate nudging via resampling.
    pub resample_nudge: bool,
    /// Inject low-level phase noise across the spectrum.
    pub phase_noise: bool,
    /// Apply slow phase rotation (swirl) effect.
    pub phase_swirl: bool,
    /// Randomize phase only in masked high-frequency bands.
    pub masked_hf_phase: bool,
    /// Gate-controlled resampling nudge for transient regions.
    pub gated_resample_nudge: bool,
    /// Apply micro EQ flutter for subtle frequency variation.
    pub micro_eq_flutter: bool,
    /// Decorrelate high-frequency stereo content.
    pub hf_decorrelate: bool,
    /// Use refined transient detection for more accurate shifting.
    pub refined_transient: bool,
    /// Adaptive transient shifting based on signal content.
    pub adaptive_transient: bool,
    /// Apply adaptive notch filtering at detected watermark frequencies.
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
