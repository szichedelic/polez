//! Request and response types for the GUI REST API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::detection::{
    metadata_scan::MetadataScanResult, polez::PolezDetectionResult, statistical::StatisticalResult,
    watermark::WatermarkResult,
};

/// Basic file information returned after loading audio.
#[derive(Serialize)]
pub struct FileInfo {
    /// Absolute path to the loaded file.
    pub file_path: String,
    /// Detected audio format (e.g. "wav", "mp3").
    pub format: String,
    /// Total duration in seconds.
    pub duration_secs: f64,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of audio channels.
    pub channels: usize,
}

/// Combined results from all detection algorithms.
#[derive(Serialize)]
pub struct AllAnalysisResult {
    /// Spread-spectrum and pattern-based watermark detection.
    pub watermark: WatermarkResult,
    /// Polez-specific detection heuristics.
    pub polez: PolezDetectionResult,
    /// Statistical analysis (entropy, kurtosis, AI probability).
    pub statistical: StatisticalResult,
    /// Metadata tag and suspicious chunk scan.
    pub metadata: MetadataScanResult,
}

/// Downsampled waveform data for visualization.
#[derive(Serialize)]
pub struct WaveformData {
    /// Per-chunk minimum sample values.
    pub min: Vec<f32>,
    /// Per-chunk maximum sample values.
    pub max: Vec<f32>,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Total duration in seconds.
    pub duration_secs: f64,
    /// Number of audio channels.
    pub channels: usize,
}

/// Spectrogram magnitude data for frontend rendering.
#[derive(Serialize)]
pub struct SpectrogramData {
    /// 2D array of magnitude values in dB (time x frequency).
    pub magnitudes: Vec<Vec<f64>>,
    /// Lower frequency bound in Hz.
    pub freq_min: f64,
    /// Upper frequency bound in Hz.
    pub freq_max: f64,
    /// Start time in seconds.
    pub time_start: f64,
    /// End time in seconds.
    pub time_end: f64,
    /// Number of frequency bins per time frame.
    pub num_freq_bins: usize,
    /// Number of time frames.
    pub num_time_frames: usize,
}

/// Summary of all 8 bit planes for LSB watermark analysis.
#[derive(Serialize)]
pub struct BitPlaneData {
    /// Per-plane statistics.
    pub planes: Vec<PlaneSummary>,
}

/// Statistics for a single bit plane.
#[derive(Serialize)]
pub struct PlaneSummary {
    /// Bit plane index (0 = LSB, 7 = MSB).
    pub bit: u8,
    /// Ratio of one-bits to total bits (0.0 - 1.0).
    pub ones_ratio: f64,
    /// Deviation from expected 0.5 ratio.
    pub bias: f64,
}

/// Configuration preset metadata for the GUI preset selector.
#[derive(Serialize)]
pub struct PresetInfo {
    /// Preset name identifier.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Whether this is a built-in (non-deletable) preset.
    pub builtin: bool,
    /// Paranoia level label.
    pub paranoia_level: String,
    /// Quality preservation level label.
    pub preserve_quality: String,
}

/// Request body for the `/api/clean` endpoint.
#[derive(Deserialize)]
pub struct CleanRequest {
    /// Sanitization mode name (e.g. "fast", "standard", "aggressive").
    pub mode: Option<String>,
    /// Named preset to apply.
    pub preset: Option<String>,
    /// Optional overrides for stealth DSP flags.
    pub advanced_flags: Option<AdvancedFlagsRequest>,
    /// Optional overrides for fingerprint removal flags.
    pub fingerprint_flags: Option<FingerprintFlagsRequest>,
}

/// Optional overrides for advanced stealth DSP operation toggles.
#[derive(Deserialize)]
pub struct AdvancedFlagsRequest {
    /// Sub-block phase dither toggle.
    pub phase_dither: Option<bool>,
    /// Dynamic comb masking toggle.
    pub comb_mask: Option<bool>,
    /// Transient micro-shift toggle.
    pub transient_shift: Option<bool>,
    /// Resample nudge toggle.
    pub resample_nudge: Option<bool>,
    /// FFT phase noise toggle.
    pub phase_noise: Option<bool>,
    /// Phase swirl toggle.
    pub phase_swirl: Option<bool>,
    /// Masked high-frequency phase noise toggle.
    pub masked_hf_phase: Option<bool>,
    /// RMS-gated resample nudge toggle.
    pub gated_resample_nudge: Option<bool>,
    /// Gated micro-EQ flutter toggle.
    pub micro_eq_flutter: Option<bool>,
    /// HF band decorrelation toggle.
    pub hf_decorrelate: Option<bool>,
    /// Refined transient micro-shift toggle.
    pub refined_transient: Option<bool>,
    /// Adaptive transient shift toggle.
    pub adaptive_transient: Option<bool>,
    /// Adaptive ultrasonic notch filter toggle.
    pub adaptive_notch: Option<bool>,
}

/// Optional overrides for fingerprint removal technique toggles.
#[derive(Deserialize)]
pub struct FingerprintFlagsRequest {
    /// Statistical normalization (kurtosis adjustment) toggle.
    pub statistical_normalization: Option<bool>,
    /// Temporal randomization (sample-level jitter) toggle.
    pub temporal_randomization: Option<bool>,
    /// Phase randomization toggle.
    pub phase_randomization: Option<bool>,
    /// Micro-timing perturbation toggle.
    pub micro_timing_perturbation: Option<bool>,
    /// Human imperfections (velocity drift, micro distortion) toggle.
    pub human_imperfections: Option<bool>,
}

/// Verification results returned as part of a clean response.
#[derive(Serialize)]
pub struct VerificationResult {
    /// Threats in the original file.
    pub original_threats: usize,
    /// Threats remaining after cleaning.
    pub remaining_threats: usize,
    /// Percentage of threats removed (0-100).
    pub removal_effectiveness: f64,
    /// Signal-to-noise ratio in dB.
    pub snr_db: f64,
    /// Spectral similarity (Pearson correlation of FFT magnitudes).
    pub spectral_similarity: f64,
    /// Combined quality preservation score (0.0 - 1.0).
    pub quality_score: f64,
    /// Letter grade (e.g. "A", "B", "C").
    pub grade: String,
    /// Verdict label (e.g. "EXCELLENT", "GOOD", "POOR").
    pub verdict: String,
    /// CSS color name for the verdict.
    pub verdict_color: String,
}

/// Response body from the `/api/clean` endpoint.
#[derive(Serialize)]
pub struct CleanResponse {
    /// Whether sanitization completed without error.
    pub success: bool,
    /// Number of metadata tags removed.
    pub metadata_removed: usize,
    /// Number of watermark patterns detected.
    pub patterns_found: usize,
    /// Number of patterns successfully suppressed.
    pub patterns_suppressed: usize,
    /// Estimated quality loss as a percentage.
    pub quality_loss: f64,
    /// Processing time in seconds.
    pub processing_time: f64,
    /// Analysis results before sanitization.
    pub before: AllAnalysisResult,
    /// Analysis results after sanitization.
    pub after: AllAnalysisResult,
    /// Before/after verification comparison.
    pub verification: VerificationResult,
}

/// Query parameters for the batch clean endpoint.
#[derive(Deserialize)]
pub struct BatchCleanQuery {
    /// Sanitization mode name.
    pub mode: Option<String>,
}

/// Per-file result in a batch clean operation.
#[derive(Serialize)]
pub struct BatchFileResult {
    /// Original filename.
    pub filename: String,
    /// Whether this file was cleaned successfully.
    pub success: bool,
    /// Error message if processing failed.
    pub error: Option<String>,
    /// Quality loss percentage, if available.
    pub quality_loss: Option<f64>,
    /// Processing time in seconds, if available.
    pub processing_time: Option<f64>,
    /// Download ID for retrieving the cleaned file.
    pub download_id: Option<String>,
}

/// Response body from the batch clean endpoint.
#[derive(Serialize)]
pub struct BatchCleanResponse {
    /// Per-file processing results.
    pub results: Vec<BatchFileResult>,
    /// Map of filename to download ID for retrieving cleaned files.
    pub download_ids: HashMap<String, String>,
}
