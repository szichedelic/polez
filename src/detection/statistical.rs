use std::collections::HashMap;

use serde::Serialize;

use crate::audio::AudioBuffer;
use crate::sanitization::dsp::{stats, stft};

/// Statistical analysis result.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StatisticalResult {
    pub ai_probability: f64,
    pub human_confidence: f64,
    pub anomalies: Vec<Anomaly>,
    pub features: HashMap<String, f64>,
    pub temporal: TemporalAnalysis,
    pub spectral: SpectralAnalysis,
}

#[derive(Debug, Clone, Serialize)]
pub struct Anomaly {
    pub anomaly_type: String,
    pub severity: String,
    pub description: String,
    pub value: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TemporalAnalysis {
    pub onset_regularity: f64,
    pub onset_count: usize,
    pub temporal_entropy: f64,
    pub suspicious: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SpectralAnalysis {
    pub centroid_variance: f64,
    pub rolloff_variance: f64,
    pub flatness_mean: f64,
    pub harmonic_ratio: f64,
    pub suspicious: bool,
}

// Human audio characteristic ranges
const ENTROPY_RANGE: (f64, f64) = (6.0, 10.0);
const KURTOSIS_RANGE: (f64, f64) = (1.5, 6.0);
const SKEWNESS_RANGE: (f64, f64) = (-0.5, 0.5);

/// Statistical analyzer for AI-generated audio characteristics.
pub struct StatisticalAnalyzer;

impl StatisticalAnalyzer {
    /// Analyze audio for statistical anomalies and compute AI probability.
    pub fn analyze(buffer: &AudioBuffer) -> StatisticalResult {
        let mut result = StatisticalResult::default();

        let mut all_features: Vec<HashMap<String, f64>> = Vec::new();
        for ch in 0..buffer.num_channels() {
            let channel: Vec<f32> = buffer.channel(ch).to_vec();
            let features = analyze_channel(&channel, buffer.sample_rate);
            all_features.push(features);
        }

        result.features = combine_features(&all_features);
        result.anomalies = detect_anomalies(&result.features);
        result.ai_probability = calculate_ai_probability(&result.features);
        result.human_confidence = 1.0 - result.ai_probability;

        let mono = buffer.to_mono();
        let channel: Vec<f32> = mono.channel(0).to_vec();
        result.temporal = analyze_temporal_patterns(&channel, buffer.sample_rate);
        result.spectral = analyze_spectral_patterns(&channel, buffer.sample_rate);

        result
    }
}

fn analyze_channel(channel: &[f32], _sr: u32) -> HashMap<String, f64> {
    let mut features = HashMap::new();

    features.insert("mean".into(), stats::mean(channel));
    features.insert("std".into(), stats::std_dev(channel));
    features.insert("skewness".into(), stats::skewness(channel));
    features.insert("kurtosis".into(), stats::kurtosis(channel));
    features.insert("rms_energy".into(), stats::rms_energy(channel));
    features.insert(
        "zero_crossing_rate".into(),
        stats::zero_crossing_rate(channel),
    );

    let hist = stats::histogram(channel, 100);
    features.insert("entropy".into(), stats::entropy(&hist));

    let fft = stft::real_fft(channel);
    let magnitude: Vec<f32> = fft.iter().map(|c| c.norm()).collect();

    features.insert(
        "spectral_centroid".into(),
        stats::spectral_centroid(&magnitude),
    );
    features.insert(
        "spectral_flatness".into(),
        stats::spectral_flatness(&magnitude),
    );
    features.insert(
        "spectral_rolloff".into(),
        stats::spectral_rolloff(&magnitude, 0.85) as f64,
    );

    // Spectral bandwidth: weighted std around centroid
    let centroid = stats::spectral_centroid(&magnitude);
    let total: f64 = magnitude.iter().map(|&m| m as f64).sum();
    if total > 1e-10 {
        let bandwidth: f64 = magnitude
            .iter()
            .enumerate()
            .map(|(i, &m)| (i as f64 - centroid).powi(2) * m as f64)
            .sum::<f64>()
            / total;
        features.insert("spectral_bandwidth".into(), bandwidth.sqrt());
    }

    if channel.len() > 1 {
        let diff: Vec<f32> = channel.windows(2).map(|w| w[1] - w[0]).collect();
        features.insert("diff_std".into(), stats::std_dev(&diff));
        let diff_hist = stats::histogram(&diff, 100);
        features.insert("diff_entropy".into(), stats::entropy(&diff_hist));
    }

    features
}

fn combine_features(channel_features: &[HashMap<String, f64>]) -> HashMap<String, f64> {
    let mut combined = HashMap::new();
    if channel_features.is_empty() {
        return combined;
    }

    let keys: Vec<String> = channel_features[0].keys().cloned().collect();

    for key in &keys {
        let values: Vec<f64> = channel_features
            .iter()
            .filter_map(|f| f.get(key))
            .cloned()
            .collect();
        if !values.is_empty() {
            let mean_val = values.iter().sum::<f64>() / values.len() as f64;
            combined.insert(key.clone(), mean_val);

            // Store std across channels so detectors can flag unnaturally consistent stereo fields
            if values.len() > 1 {
                let std_val: f64 = {
                    let var = values.iter().map(|&v| (v - mean_val).powi(2)).sum::<f64>()
                        / values.len() as f64;
                    var.sqrt()
                };
                combined.insert(format!("{key}_std"), std_val);
            }
        }
    }

    combined
}

fn detect_anomalies(features: &HashMap<String, f64>) -> Vec<Anomaly> {
    let mut anomalies = Vec::new();

    if let Some(&ent) = features.get("entropy") {
        if ent < ENTROPY_RANGE.0 {
            anomalies.push(Anomaly {
                anomaly_type: "low_entropy".into(),
                severity: "high".into(),
                description: format!(
                    "Entropy {ent:.3} below natural range ({:.1}-{:.1})",
                    ENTROPY_RANGE.0, ENTROPY_RANGE.1
                ),
                value: ent,
            });
        } else if ent > ENTROPY_RANGE.1 {
            anomalies.push(Anomaly {
                anomaly_type: "high_entropy".into(),
                severity: "medium".into(),
                description: format!("Entropy {ent:.3} above natural range"),
                value: ent,
            });
        }
    }

    if let Some(&kurt) = features.get("kurtosis") {
        if kurt < KURTOSIS_RANGE.0 || kurt > KURTOSIS_RANGE.1 {
            anomalies.push(Anomaly {
                anomaly_type: "abnormal_kurtosis".into(),
                severity: "medium".into(),
                description: format!(
                    "Kurtosis {kurt:.3} outside natural range ({:.1}-{:.1})",
                    KURTOSIS_RANGE.0, KURTOSIS_RANGE.1
                ),
                value: kurt,
            });
        }
    }

    if let Some(&skew) = features.get("skewness") {
        if skew.abs() > SKEWNESS_RANGE.1 {
            anomalies.push(Anomaly {
                anomaly_type: "abnormal_skewness".into(),
                severity: "medium".into(),
                description: format!("Skewness {skew:.3} outside natural range"),
                value: skew,
            });
        }
    }

    if let Some(&sc_std) = features.get("spectral_centroid_std") {
        if sc_std < 0.01 {
            anomalies.push(Anomaly {
                anomaly_type: "low_spectral_variance".into(),
                severity: "medium".into(),
                description: format!("Spectral centroid std {sc_std:.6} is unnaturally low"),
                value: sc_std,
            });
        }
    }

    if let Some(&zcr) = features.get("zero_crossing_rate") {
        if !(0.01..=0.2).contains(&zcr) {
            anomalies.push(Anomaly {
                anomaly_type: "abnormal_zcr".into(),
                severity: "low".into(),
                description: format!("Zero crossing rate {zcr:.4} outside typical range"),
                value: zcr,
            });
        }
    }

    anomalies
}

fn score_feature(value: f64, range: (f64, f64)) -> f64 {
    if value >= range.0 && value <= range.1 {
        0.0
    } else if value < range.0 {
        (-(value / range.0)).exp().min(1.0)
    } else {
        (1.0 - (-(value / range.1 - 1.0)).exp()).min(1.0)
    }
}

fn calculate_ai_probability(features: &HashMap<String, f64>) -> f64 {
    let mut weighted_score = 0.0;
    let mut total_weight = 0.0;

    if let Some(&ent) = features.get("entropy") {
        weighted_score += 0.20 * score_feature(ent, ENTROPY_RANGE);
        total_weight += 0.20;
    }

    if let Some(&kurt) = features.get("kurtosis") {
        weighted_score += 0.20 * score_feature(kurt, KURTOSIS_RANGE);
        total_weight += 0.20;
    }

    if let Some(&skew) = features.get("skewness") {
        weighted_score += 0.15 * score_feature(skew.abs(), SKEWNESS_RANGE);
        total_weight += 0.15;
    }

    if let Some(&sc_std) = features.get("spectral_centroid_std") {
        let consistency = 1.0 - (sc_std * 100.0).min(1.0);
        weighted_score += 0.25 * consistency;
        total_weight += 0.25;
    }

    if let Some(&flatness) = features.get("spectral_flatness") {
        // Very low flatness = too tonal; very high = too noisy — both are suspicious
        let flat_score = if flatness < 0.1 {
            0.7
        } else if flatness > 0.8 {
            0.6
        } else {
            0.0
        };
        weighted_score += 0.20 * flat_score;
        total_weight += 0.20;
    }

    if total_weight > 0.0 {
        (weighted_score / total_weight).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn analyze_temporal_patterns(channel: &[f32], sr: u32) -> TemporalAnalysis {
    let mut analysis = TemporalAnalysis::default();

    // Simple onset detection: compute energy in short frames, find peaks
    let frame_size = (sr as usize / 100).max(64); // ~10ms frames
    let hop = frame_size / 2;
    let mut energies: Vec<f64> = Vec::new();

    let mut pos = 0;
    while pos + frame_size <= channel.len() {
        let frame = &channel[pos..pos + frame_size];
        let energy = stats::rms_energy(frame);
        energies.push(energy);
        pos += hop;
    }

    if energies.len() < 3 {
        return analysis;
    }

    let flux: Vec<f64> = energies
        .windows(2)
        .map(|w| (w[1] - w[0]).max(0.0))
        .collect();

    let max_flux = flux.iter().cloned().fold(0.0_f64, f64::max);
    let threshold = max_flux * 0.3;
    let peaks = stats::find_peaks(&flux, threshold, (sr as usize / hop / 10).max(1));

    analysis.onset_count = peaks.len();

    if peaks.len() >= 2 {
        let intervals: Vec<f64> = peaks
            .windows(2)
            .map(|w| (w[1].index - w[0].index) as f64 * hop as f64 / sr as f64)
            .collect();

        let mean_interval: f64 = intervals.iter().sum::<f64>() / intervals.len() as f64;
        let std_interval: f64 = {
            let var = intervals
                .iter()
                .map(|&i| (i - mean_interval).powi(2))
                .sum::<f64>()
                / intervals.len() as f64;
            var.sqrt()
        };

        analysis.onset_regularity = 1.0 - (std_interval / (mean_interval + 1e-10));
        analysis.suspicious = analysis.onset_regularity > 0.8;
    }

    let hist = stats::histogram(channel, 50);
    analysis.temporal_entropy = stats::entropy(&hist);

    analysis
}

fn analyze_spectral_patterns(channel: &[f32], _sr: u32) -> SpectralAnalysis {
    let mut analysis = SpectralAnalysis::default();

    let nperseg = 2048.min(channel.len() / 4).max(256);
    let noverlap = nperseg / 2;
    let (spectrogram, _) = stft::stft(channel, nperseg, noverlap);

    if spectrogram.is_empty() {
        return analysis;
    }

    let mut centroids: Vec<f64> = Vec::new();
    let mut rolloffs: Vec<f64> = Vec::new();
    let mut flatnesses: Vec<f64> = Vec::new();
    let mut low_energy: f64 = 0.0;
    let mut total_energy: f64 = 0.0;

    for frame in &spectrogram {
        let mag: Vec<f32> = frame.iter().map(|c| c.norm()).collect();
        centroids.push(stats::spectral_centroid(&mag));
        rolloffs.push(stats::spectral_rolloff(&mag, 0.85) as f64);
        flatnesses.push(stats::spectral_flatness(&mag));

        let n_freqs = mag.len();
        let low_cutoff = n_freqs / 4;
        let frame_low: f64 = mag[..low_cutoff].iter().map(|&m| (m as f64).powi(2)).sum();
        let frame_total: f64 = mag.iter().map(|&m| (m as f64).powi(2)).sum();
        low_energy += frame_low;
        total_energy += frame_total;
    }

    let cent_mean: f64 = centroids.iter().sum::<f64>() / centroids.len() as f64;
    analysis.centroid_variance = centroids
        .iter()
        .map(|&c| (c - cent_mean).powi(2))
        .sum::<f64>()
        / centroids.len() as f64;

    let roll_mean: f64 = rolloffs.iter().sum::<f64>() / rolloffs.len() as f64;
    analysis.rolloff_variance = rolloffs
        .iter()
        .map(|&r| (r - roll_mean).powi(2))
        .sum::<f64>()
        / rolloffs.len() as f64;

    analysis.flatness_mean = flatnesses.iter().sum::<f64>() / flatnesses.len() as f64;

    if total_energy > 1e-10 {
        analysis.harmonic_ratio = low_energy / total_energy;
    }

    analysis.suspicious = analysis.harmonic_ratio < 0.1 || analysis.harmonic_ratio > 0.9;

    analysis
}
