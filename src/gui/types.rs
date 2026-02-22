use serde::Serialize;

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
