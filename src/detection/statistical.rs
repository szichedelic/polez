//! Statistical analysis for detecting AI-generated audio.
//!
//! Computes entropy, kurtosis, spectral features, temporal patterns, and
//! AI-specific indicators to estimate the probability that audio was machine-generated.

use std::collections::HashMap;

use serde::Serialize;

use crate::audio::AudioBuffer;
use crate::sanitization::dsp::{stats, stft};

/// Statistical analysis result.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StatisticalResult {
    /// Estimated probability that the audio is AI-generated (0.0 to 1.0).
    pub ai_probability: f64,
    /// Complement of `ai_probability` (1.0 - ai_probability).
    pub human_confidence: f64,
    /// Statistical anomalies found in the audio.
    pub anomalies: Vec<Anomaly>,
    /// Computed feature values keyed by feature name.
    pub features: HashMap<String, f64>,
    /// Temporal pattern analysis results.
    pub temporal: TemporalAnalysis,
    /// Spectral pattern analysis results.
    pub spectral: SpectralAnalysis,
    /// AI-specific generation indicators.
    pub ai_indicators: AiIndicators,
}

/// Specific AI generation indicators.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AiIndicators {
    /// How smooth the spectrogram is frame-to-frame (higher = more AI-like).
    pub spectral_continuity: f64,
    /// Score for periodic micro-silence patterns at chunk boundaries.
    pub micro_silence_score: f64,
    /// How consistent overtone ratios are across frames (higher = more AI-like).
    pub harmonic_regularity: f64,
    /// How precise onset timing is (higher = more machine-like).
    pub onset_machine_score: f64,
    /// Human-readable descriptions of detected AI indicators.
    pub indicators_found: Vec<String>,
}

/// A detected statistical anomaly in the audio signal.
#[derive(Debug, Clone, Serialize)]
pub struct Anomaly {
    /// Category of the anomaly (e.g., "low_entropy", "abnormal_kurtosis").
    pub anomaly_type: String,
    /// Severity level: "low", "medium", or "high".
    pub severity: String,
    /// Human-readable description of the anomaly.
    pub description: String,
    /// The measured value that triggered the anomaly.
    pub value: f64,
}

/// Temporal pattern analysis of onset timing and entropy.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TemporalAnalysis {
    /// How regular onset intervals are (0.0 = irregular, 1.0 = perfectly regular).
    pub onset_regularity: f64,
    /// Number of detected onsets.
    pub onset_count: usize,
    /// Shannon entropy of the temporal amplitude distribution.
    pub temporal_entropy: f64,
    /// Whether the temporal patterns are suspicious for AI generation.
    pub suspicious: bool,
}

/// Spectral pattern analysis of frequency-domain features over time.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SpectralAnalysis {
    /// Variance of spectral centroid across STFT frames.
    pub centroid_variance: f64,
    /// Variance of spectral rolloff across STFT frames.
    pub rolloff_variance: f64,
    /// Mean spectral flatness across frames.
    pub flatness_mean: f64,
    /// Ratio of low-frequency to total energy.
    pub harmonic_ratio: f64,
    /// Whether the spectral patterns are suspicious for AI generation.
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

        let mono = buffer.to_mono();
        let channel: Vec<f32> = mono.channel(0).to_vec();
        result.temporal = analyze_temporal_patterns(&channel, buffer.sample_rate);
        result.spectral = analyze_spectral_patterns(&channel, buffer.sample_rate);
        result.ai_indicators = analyze_ai_indicators(&channel, buffer.sample_rate);

        // Recalculate AI probability incorporating new indicators
        result.ai_probability = calculate_ai_probability(&result.features, &result.ai_indicators);
        result.human_confidence = 1.0 - result.ai_probability;

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

fn calculate_ai_probability(features: &HashMap<String, f64>, ai_indicators: &AiIndicators) -> f64 {
    let mut weighted_score = 0.0;
    let mut total_weight = 0.0;

    // Classic statistical features (40% weight)
    if let Some(&ent) = features.get("entropy") {
        weighted_score += 0.10 * score_feature(ent, ENTROPY_RANGE);
        total_weight += 0.10;
    }

    if let Some(&kurt) = features.get("kurtosis") {
        weighted_score += 0.10 * score_feature(kurt, KURTOSIS_RANGE);
        total_weight += 0.10;
    }

    if let Some(&skew) = features.get("skewness") {
        weighted_score += 0.10 * score_feature(skew.abs(), SKEWNESS_RANGE);
        total_weight += 0.10;
    }

    if let Some(&flatness) = features.get("spectral_flatness") {
        let flat_score = if flatness < 0.1 {
            0.7
        } else if flatness > 0.8 {
            0.6
        } else {
            0.0
        };
        weighted_score += 0.10 * flat_score;
        total_weight += 0.10;
    }

    // AI-specific indicators (60% weight)
    // Spectral continuity: AI audio has unnaturally smooth spectrograms
    weighted_score += 0.20 * ai_indicators.spectral_continuity;
    total_weight += 0.20;

    // Micro-silence patterns: AI models produce characteristic gaps
    weighted_score += 0.15 * ai_indicators.micro_silence_score;
    total_weight += 0.15;

    // Harmonic regularity: AI overtone patterns are too regular
    weighted_score += 0.15 * ai_indicators.harmonic_regularity;
    total_weight += 0.15;

    // Onset machine score: AI onsets are unnaturally precise
    weighted_score += 0.10 * ai_indicators.onset_machine_score;
    total_weight += 0.10;

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

/// Analyze audio for specific AI generation indicators.
fn analyze_ai_indicators(channel: &[f32], sr: u32) -> AiIndicators {
    let mut indicators = AiIndicators::default();

    if channel.len() < 8192 {
        return indicators;
    }

    // 1. Spectral continuity: AI audio has unnaturally smooth spectrograms
    indicators.spectral_continuity = measure_spectral_continuity(channel);
    if indicators.spectral_continuity > 0.5 {
        indicators.indicators_found.push(format!(
            "High spectral continuity: {:.3} (AI models produce overly smooth spectra)",
            indicators.spectral_continuity
        ));
    }

    // 2. Micro-silence pattern detection
    indicators.micro_silence_score = detect_micro_silences(channel, sr);
    if indicators.micro_silence_score > 0.3 {
        indicators.indicators_found.push(format!(
            "Micro-silence patterns: {:.3} (characteristic of AI chunk boundaries)",
            indicators.micro_silence_score
        ));
    }

    // 3. Harmonic regularity: AI overtone patterns are too uniform
    indicators.harmonic_regularity = measure_harmonic_regularity(channel, sr);
    if indicators.harmonic_regularity > 0.5 {
        indicators.indicators_found.push(format!(
            "Harmonic regularity: {:.3} (AI overtones are unnaturally consistent)",
            indicators.harmonic_regularity
        ));
    }

    // 4. Onset machine score: AI timing is unnaturally precise
    indicators.onset_machine_score = measure_onset_precision(channel, sr);
    if indicators.onset_machine_score > 0.5 {
        indicators.indicators_found.push(format!(
            "Machine-like onsets: {:.3} (AI lacks human timing variation)",
            indicators.onset_machine_score
        ));
    }

    indicators
}

/// Measure spectral continuity — how smooth the spectrogram is frame-to-frame.
/// AI-generated audio often has unnaturally smooth spectral transitions.
fn measure_spectral_continuity(channel: &[f32]) -> f64 {
    let nperseg = 2048.min(channel.len() / 4).max(256);
    let noverlap = nperseg * 3 / 4;
    let (spectrogram, _) = stft::stft(channel, nperseg, noverlap);

    if spectrogram.len() < 4 {
        return 0.0;
    }

    let n_freqs = spectrogram[0].len();

    // Compute frame-to-frame spectral difference
    let mut frame_diffs: Vec<f64> = Vec::new();
    for i in 1..spectrogram.len() {
        let mut diff_sum = 0.0f64;
        for bin in 0..n_freqs {
            let prev = spectrogram[i - 1][bin].norm() as f64;
            let curr = spectrogram[i][bin].norm() as f64;
            diff_sum += (curr - prev).abs();
        }
        frame_diffs.push(diff_sum / n_freqs as f64);
    }

    if frame_diffs.is_empty() {
        return 0.0;
    }

    let mean_diff: f64 = frame_diffs.iter().sum::<f64>() / frame_diffs.len() as f64;
    let diff_std: f64 = {
        let var = frame_diffs
            .iter()
            .map(|&d| (d - mean_diff).powi(2))
            .sum::<f64>()
            / frame_diffs.len() as f64;
        var.sqrt()
    };

    // Low variation in spectral differences = unnaturally smooth
    // Natural audio has more varied spectral changes
    if mean_diff > 1e-10 {
        let cv = diff_std / mean_diff; // coefficient of variation
                                       // Low CV means very consistent frame-to-frame changes (AI-like)
        (1.0 - cv.min(2.0) / 2.0).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Detect micro-silence patterns characteristic of AI chunk boundaries.
/// AI models often produce brief energy dips at generation chunk boundaries.
fn detect_micro_silences(channel: &[f32], sr: u32) -> f64 {
    let frame_ms = 5; // 5ms frames
    let frame_size = (sr as usize * frame_ms / 1000).max(32);
    let hop = frame_size;

    let mut energies: Vec<f64> = Vec::new();
    let mut pos = 0;
    while pos + frame_size <= channel.len() {
        let frame = &channel[pos..pos + frame_size];
        let rms = stats::rms_energy(frame);
        energies.push(rms);
        pos += hop;
    }

    if energies.len() < 20 {
        return 0.0;
    }

    let median_energy = {
        let mut sorted = energies.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        sorted[sorted.len() / 2]
    };

    if median_energy < 1e-10 {
        return 0.0;
    }

    // Find micro-silences: frames with energy < 5% of median, surrounded by normal audio
    let threshold = median_energy * 0.05;
    let mut micro_silence_indices: Vec<usize> = Vec::new();

    for i in 2..energies.len().saturating_sub(2) {
        if energies[i] < threshold
            && energies[i - 1] > median_energy * 0.3
            && energies[i + 1] > median_energy * 0.3
        {
            micro_silence_indices.push(i);
        }
    }

    if micro_silence_indices.len() < 2 {
        return 0.0;
    }

    // Check if micro-silences are periodic (AI chunk boundaries)
    let intervals: Vec<f64> = micro_silence_indices
        .windows(2)
        .map(|w| (w[1] - w[0]) as f64 * frame_ms as f64 / 1000.0)
        .collect();

    if intervals.is_empty() {
        return 0.0;
    }

    let mean_interval: f64 = intervals.iter().sum::<f64>() / intervals.len() as f64;
    let interval_std: f64 = {
        let var = intervals
            .iter()
            .map(|&i| (i - mean_interval).powi(2))
            .sum::<f64>()
            / intervals.len() as f64;
        var.sqrt()
    };

    // Periodic micro-silences are suspicious
    let regularity = if mean_interval > 0.01 {
        1.0 - (interval_std / mean_interval).min(1.0)
    } else {
        0.0
    };

    // More micro-silences + more regular = more suspicious
    let count_score = (micro_silence_indices.len() as f64 / 10.0).min(1.0);
    (regularity * 0.6 + count_score * 0.4).clamp(0.0, 1.0)
}

/// Measure harmonic regularity — how consistent overtone ratios are across frames.
/// Real instruments have natural variation in harmonic structure; AI is too uniform.
fn measure_harmonic_regularity(channel: &[f32], sr: u32) -> f64 {
    let nperseg = 4096.min(channel.len() / 2).max(512);
    let noverlap = nperseg * 3 / 4;
    let (spectrogram, _) = stft::stft(channel, nperseg, noverlap);

    if spectrogram.len() < 4 {
        return 0.0;
    }

    let n_freqs = spectrogram[0].len();
    let freq_res = sr as f64 / nperseg as f64;

    // For each frame, find fundamental and measure harmonic ratios
    let mut harmonic_ratio_sets: Vec<Vec<f64>> = Vec::new();

    for frame in spectrogram.iter().take(32) {
        let mag: Vec<f64> = frame.iter().map(|c| c.norm() as f64).collect();
        let max_mag = mag.iter().cloned().fold(0.0f64, f64::max);
        if max_mag < 1e-10 {
            continue;
        }

        // Find fundamental (strongest peak in 80-2000 Hz)
        let lo_bin = (80.0 / freq_res) as usize;
        let hi_bin = ((2000.0 / freq_res) as usize).min(n_freqs);
        if lo_bin >= hi_bin {
            continue;
        }

        let mut fund_bin = lo_bin;
        let mut fund_mag = 0.0f64;
        for (bin, &val) in mag.iter().enumerate().take(hi_bin).skip(lo_bin) {
            if val > fund_mag {
                fund_mag = val;
                fund_bin = bin;
            }
        }

        if fund_mag < max_mag * 0.1 {
            continue;
        }

        // Measure ratios at harmonics 2-6
        let mut ratios = Vec::new();
        for h in 2..=6 {
            let h_bin = fund_bin * h;
            if h_bin >= n_freqs {
                break;
            }
            // Check a small window around expected harmonic
            let search_lo = h_bin.saturating_sub(2);
            let search_hi = (h_bin + 3).min(n_freqs);
            let h_mag = mag[search_lo..search_hi]
                .iter()
                .cloned()
                .fold(0.0f64, f64::max);
            ratios.push(h_mag / fund_mag);
        }

        if ratios.len() >= 3 {
            harmonic_ratio_sets.push(ratios);
        }
    }

    if harmonic_ratio_sets.len() < 3 {
        return 0.0;
    }

    // Measure how consistent harmonic ratios are across frames
    let n_harmonics = harmonic_ratio_sets
        .iter()
        .map(|r| r.len())
        .min()
        .unwrap_or(0);
    if n_harmonics == 0 {
        return 0.0;
    }

    let mut total_cv = 0.0f64;
    for h in 0..n_harmonics {
        let vals: Vec<f64> = harmonic_ratio_sets.iter().map(|r| r[h]).collect();
        let mean: f64 = vals.iter().sum::<f64>() / vals.len() as f64;
        let std: f64 = {
            let var = vals.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / vals.len() as f64;
            var.sqrt()
        };
        if mean > 1e-10 {
            total_cv += std / mean;
        }
    }

    let avg_cv = total_cv / n_harmonics as f64;
    // Low CV = too consistent = AI-like
    (1.0 - avg_cv.min(1.0)).clamp(0.0, 1.0)
}

/// Measure onset timing precision.
/// AI-generated audio has unnaturally precise onset timing compared to human performance.
fn measure_onset_precision(channel: &[f32], sr: u32) -> f64 {
    let frame_size = (sr as usize / 200).max(64); // ~5ms frames
    let hop = frame_size / 2;

    let mut energies: Vec<f64> = Vec::new();
    let mut pos = 0;
    while pos + frame_size <= channel.len() {
        let frame = &channel[pos..pos + frame_size];
        energies.push(stats::rms_energy(frame));
        pos += hop;
    }

    if energies.len() < 10 {
        return 0.0;
    }

    // Compute spectral flux (onset strength)
    let flux: Vec<f64> = energies
        .windows(2)
        .map(|w| (w[1] - w[0]).max(0.0))
        .collect();

    let max_flux = flux.iter().cloned().fold(0.0f64, f64::max);
    if max_flux < 1e-10 {
        return 0.0;
    }

    let threshold = max_flux * 0.2;
    let peaks = stats::find_peaks(&flux, threshold, (sr as usize / hop / 15).max(1));

    if peaks.len() < 4 {
        return 0.0;
    }

    // Measure onset shape consistency: rise time of each onset
    let mut rise_times: Vec<f64> = Vec::new();
    for peak in &peaks {
        let idx = peak.index;
        // Look backwards to find where onset started (energy below threshold)
        let mut start = idx;
        while start > 0 && flux[start] > threshold * 0.1 {
            start -= 1;
        }
        let rise = (idx - start) as f64 * hop as f64 / sr as f64;
        if rise > 0.0 {
            rise_times.push(rise);
        }
    }

    if rise_times.len() < 3 {
        return 0.0;
    }

    let mean_rise: f64 = rise_times.iter().sum::<f64>() / rise_times.len() as f64;
    let rise_std: f64 = {
        let var = rise_times
            .iter()
            .map(|&r| (r - mean_rise).powi(2))
            .sum::<f64>()
            / rise_times.len() as f64;
        var.sqrt()
    };

    // Very consistent rise times = machine-like
    if mean_rise > 1e-10 {
        let cv = rise_std / mean_rise;
        (1.0 - cv.min(1.0)).clamp(0.0, 1.0)
    } else {
        0.0
    }
}
