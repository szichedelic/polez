use serde::{Deserialize, Serialize};

use crate::detection::{
    metadata_scan::MetadataScanResult, polez::PolezDetectionResult, statistical::StatisticalResult,
    watermark::WatermarkResult,
};

#[derive(Serialize)]
pub struct FileInfo {
    pub file_path: String,
    pub format: String,
    pub duration_secs: f64,
    pub sample_rate: u32,
    pub channels: usize,
}

#[derive(Serialize)]
pub struct AllAnalysisResult {
    pub watermark: WatermarkResult,
    pub polez: PolezDetectionResult,
    pub statistical: StatisticalResult,
    pub metadata: MetadataScanResult,
}

#[derive(Serialize)]
pub struct WaveformData {
    pub min: Vec<f32>,
    pub max: Vec<f32>,
    pub sample_rate: u32,
    pub duration_secs: f64,
    pub channels: usize,
}

#[derive(Serialize)]
pub struct SpectrogramData {
    pub magnitudes: Vec<Vec<f64>>,
    pub freq_min: f64,
    pub freq_max: f64,
    pub time_start: f64,
    pub time_end: f64,
    pub num_freq_bins: usize,
    pub num_time_frames: usize,
}

#[derive(Serialize)]
pub struct BitPlaneData {
    pub planes: Vec<PlaneSummary>,
}

#[derive(Serialize)]
pub struct PlaneSummary {
    pub bit: u8,
    pub ones_ratio: f64,
    pub bias: f64,
}

#[derive(Serialize)]
pub struct PresetInfo {
    pub name: String,
    pub description: String,
    pub builtin: bool,
    pub paranoia_level: String,
    pub preserve_quality: String,
}

#[derive(Deserialize)]
pub struct CleanRequest {
    pub mode: Option<String>,
    pub preset: Option<String>,
    pub advanced_flags: Option<AdvancedFlagsRequest>,
    pub fingerprint_flags: Option<FingerprintFlagsRequest>,
}

#[derive(Deserialize)]
pub struct AdvancedFlagsRequest {
    pub phase_dither: Option<bool>,
    pub comb_mask: Option<bool>,
    pub transient_shift: Option<bool>,
    pub resample_nudge: Option<bool>,
    pub phase_noise: Option<bool>,
    pub phase_swirl: Option<bool>,
    pub masked_hf_phase: Option<bool>,
    pub gated_resample_nudge: Option<bool>,
    pub micro_eq_flutter: Option<bool>,
    pub hf_decorrelate: Option<bool>,
    pub refined_transient: Option<bool>,
    pub adaptive_transient: Option<bool>,
    pub adaptive_notch: Option<bool>,
}

#[derive(Deserialize)]
pub struct FingerprintFlagsRequest {
    pub statistical_normalization: Option<bool>,
    pub temporal_randomization: Option<bool>,
    pub phase_randomization: Option<bool>,
    pub micro_timing_perturbation: Option<bool>,
    pub human_imperfections: Option<bool>,
}

#[derive(Serialize)]
pub struct VerificationResult {
    pub original_threats: usize,
    pub remaining_threats: usize,
    pub removal_effectiveness: f64,
    pub snr_db: f64,
    pub spectral_similarity: f64,
    pub quality_score: f64,
    pub grade: String,
    pub verdict: String,
    pub verdict_color: String,
}

#[derive(Serialize)]
pub struct CleanResponse {
    pub success: bool,
    pub metadata_removed: usize,
    pub patterns_found: usize,
    pub patterns_suppressed: usize,
    pub quality_loss: f64,
    pub processing_time: f64,
    pub before: AllAnalysisResult,
    pub after: AllAnalysisResult,
    pub verification: VerificationResult,
}
