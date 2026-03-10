//! Watermark detection using multiple signal-analysis methods.
//!
//! Includes spread-spectrum, echo, phase/amplitude modulation, frequency-domain,
//! LSB steganography, codec artifact, phase coherence, and spatial encoding detectors.

use std::collections::HashMap;

use rayon::prelude::*;
use serde::Serialize;

use crate::audio::AudioBuffer;
use crate::sanitization::dsp::{hilbert, stats, stft};

/// Results from watermark detection.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WatermarkResult {
    /// Individual watermark detections across all methods.
    pub detected: Vec<WatermarkDetection>,
    /// Per-method detection results keyed by method name.
    pub method_results: HashMap<String, MethodResult>,
    /// Combined confidence score across all methods (0.0 to 1.0).
    pub overall_confidence: f64,
    /// Total number of methods that detected a watermark.
    pub watermark_count: usize,
}

/// A single watermark detection from one method.
#[derive(Debug, Clone, Serialize)]
pub struct WatermarkDetection {
    /// Name of the detection method that triggered.
    pub method: String,
    /// Confidence level of the detection (0.0 to 1.0).
    pub confidence: f64,
    /// Human-readable description of the finding.
    pub description: String,
}

/// Result from an individual detection method.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MethodResult {
    /// Whether this method detected a watermark.
    pub detected: bool,
    /// Confidence level (0.0 to 1.0).
    pub confidence: f64,
    /// Descriptive detail strings about the findings.
    pub details: Vec<String>,
}

/// Known watermark carrier frequencies (Hz).
const WATERMARK_FREQS: [f64; 4] = [18000.0, 19000.0, 20000.0, 21000.0];

/// Watermark detector - runs multiple detection methods.
pub struct WatermarkDetector;

impl WatermarkDetector {
    /// All available detection method names.
    pub const METHOD_NAMES: &'static [&'static str] = &[
        "spread_spectrum",
        "echo_signatures",
        "statistical_anomalies",
        "phase_modulation",
        "amplitude_modulation",
        "frequency_domain",
        "lsb_steganography",
        "codec_artifacts",
        "phase_coherence",
        "spatial_encoding",
    ];

    /// Run all detection methods on the audio buffer.
    pub fn detect_all(buffer: &AudioBuffer) -> WatermarkResult {
        Self::detect_filtered(buffer, None)
    }

    /// Run detection methods, optionally filtered to only the specified names.
    pub fn detect_filtered(buffer: &AudioBuffer, filter: Option<&[String]>) -> WatermarkResult {
        let mut result = WatermarkResult::default();

        if buffer.num_samples() < 4096 {
            return result;
        }

        let mono = buffer.to_mono();
        let channel: Vec<f32> = mono.channel(0).to_vec();
        let sr = buffer.sample_rate;

        type DetectFn = fn(&[f32], u32) -> MethodResult;
        let methods: Vec<(&str, DetectFn)> = vec![
            ("spread_spectrum", detect_spread_spectrum),
            ("echo_signatures", detect_echo_signatures),
            ("statistical_anomalies", analyze_statistical_anomalies),
            ("phase_modulation", detect_phase_modulation),
            ("amplitude_modulation", detect_amplitude_modulation),
            ("frequency_domain", detect_frequency_domain),
            ("lsb_steganography", detect_lsb_steganography),
            ("codec_artifacts", detect_codec_artifacts),
        ];

        // Filter methods first, then run in parallel
        let active_methods: Vec<_> = methods
            .into_iter()
            .filter(|(name, _)| match filter {
                Some(f) => f.iter().any(|flt| flt == name),
                None => true,
            })
            .collect();

        let mono_results: Vec<(String, MethodResult)> = active_methods
            .par_iter()
            .map(|(name, method)| (name.to_string(), method(&channel, sr)))
            .collect();

        for (name, mr) in mono_results {
            if mr.detected {
                result.detected.push(WatermarkDetection {
                    method: name.clone(),
                    confidence: mr.confidence,
                    description: mr.details.first().cloned().unwrap_or_default(),
                });
                result.watermark_count += 1;
            }
            result.method_results.insert(name, mr);
        }

        // Phase coherence and spatial encoding need the full buffer — run in parallel
        let run_phase_coherence = match filter {
            Some(f) => f.iter().any(|name| name == "phase_coherence"),
            None => true,
        };
        let run_spatial = match filter {
            Some(f) => f.iter().any(|name| name == "spatial_encoding"),
            None => true,
        };

        let (phase_mr, spatial_mr) = rayon::join(
            || {
                if run_phase_coherence {
                    Some(detect_phase_coherence(buffer))
                } else {
                    None
                }
            },
            || {
                if run_spatial {
                    Some(detect_spatial_encoding(buffer))
                } else {
                    None
                }
            },
        );

        if let Some(mr) = phase_mr {
            if mr.detected {
                result.detected.push(WatermarkDetection {
                    method: "phase_coherence".to_string(),
                    confidence: mr.confidence,
                    description: mr.details.first().cloned().unwrap_or_default(),
                });
                result.watermark_count += 1;
            }
            result
                .method_results
                .insert("phase_coherence".to_string(), mr);
        }

        if let Some(mr) = spatial_mr {
            if mr.detected {
                result.detected.push(WatermarkDetection {
                    method: "spatial_encoding".to_string(),
                    confidence: mr.confidence,
                    description: mr.details.first().cloned().unwrap_or_default(),
                });
                result.watermark_count += 1;
            }
            result
                .method_results
                .insert("spatial_encoding".to_string(), mr);
        }

        result.overall_confidence = if result.detected.is_empty() {
            0.0
        } else {
            result.detected.iter().map(|d| d.confidence).sum::<f64>() / result.detected.len() as f64
        };

        result
    }
}

/// Method 1: Spread spectrum detection.
/// Looks for unusual consistency in high-frequency energy and known watermark carrier frequencies.
fn detect_spread_spectrum(channel: &[f32], sr: u32) -> MethodResult {
    let mut result = MethodResult::default();
    let n = channel.len();

    let nperseg = 2048.min(n / 8).max(256);
    let noverlap = nperseg / 4;
    let (spectrogram, _) = stft::stft(channel, nperseg, noverlap);

    if spectrogram.is_empty() {
        return result;
    }

    let freq_resolution = sr as f64 / nperseg as f64;
    let high_freq_start_bin = (15000.0 / freq_resolution) as usize;
    let n_freqs = spectrogram[0].len();

    if high_freq_start_bin >= n_freqs {
        return result;
    }

    let mut hf_powers: Vec<f64> = Vec::new();
    for frame in &spectrogram {
        for val in frame.iter().take(n_freqs).skip(high_freq_start_bin) {
            hf_powers.push(val.norm() as f64);
        }
    }

    if hf_powers.is_empty() {
        return result;
    }

    let mean_power: f64 = hf_powers.iter().sum::<f64>() / hf_powers.len() as f64;
    let std_power: f64 = {
        let variance = hf_powers
            .iter()
            .map(|&p| (p - mean_power).powi(2))
            .sum::<f64>()
            / hf_powers.len() as f64;
        variance.sqrt()
    };

    let consistency_score = 1.0 - (std_power / (mean_power + 1e-10));

    if consistency_score > 0.7 {
        result.detected = true;
        result.confidence = consistency_score;
        result.details.push(format!(
            "High-frequency consistency score: {consistency_score:.3}"
        ));
    }

    for &freq in &WATERMARK_FREQS {
        if freq >= sr as f64 / 2.0 {
            continue;
        }
        let bin = (freq / freq_resolution) as usize;
        if bin >= n_freqs {
            continue;
        }

        let mut freq_power = 0.0;
        for frame in &spectrogram {
            freq_power += frame[bin].norm() as f64;
        }
        freq_power /= spectrogram.len() as f64;

        if freq_power > mean_power + 3.0 * std_power {
            result.detected = true;
            result.confidence = result.confidence.max(0.8);
            result.details.push(format!(
                "Suspicious energy at {freq} Hz (power: {freq_power:.3})"
            ));
        }
    }

    result
}

/// Method 2: Echo signature detection.
/// Looks for periodic echo patterns in the autocorrelation.
fn detect_echo_signatures(channel: &[f32], sr: u32) -> MethodResult {
    let mut result = MethodResult::default();

    let max_lag = (sr as usize / 20).min(channel.len()); // up to 50ms
    let truncated: Vec<f32> = channel.iter().take(max_lag * 4).cloned().collect();
    let autocorr = stats::autocorrelation(&truncated);

    let half = autocorr.len() / 2;
    let positive_lags: Vec<f64> = autocorr[half..].to_vec();
    let peaks = stats::find_peaks(&positive_lags, 0.1, 100);

    let mut echo_delays: Vec<f64> = Vec::new();
    for peak in peaks.iter().take(10) {
        if peak.index == 0 {
            continue;
        }
        let delay_ms = peak.index as f64 / sr as f64 * 1000.0;
        let strength = peak.value / (positive_lags[0] + 1e-10);

        if (1.0..=50.0).contains(&delay_ms) && strength > 0.1 {
            echo_delays.push(delay_ms);
        }
    }

    if echo_delays.len() >= 2 {
        let diffs: Vec<f64> = echo_delays.windows(2).map(|w| w[1] - w[0]).collect();
        let diff_mean: f64 = diffs.iter().sum::<f64>() / diffs.len() as f64;
        let diff_std: f64 = {
            let var =
                diffs.iter().map(|&d| (d - diff_mean).powi(2)).sum::<f64>() / diffs.len() as f64;
            var.sqrt()
        };

        let delay_consistency = 1.0 - (diff_std / (diff_mean + 1e-10));

        if delay_consistency > 0.8 {
            result.detected = true;
            result.confidence = delay_consistency;
            result.details.push(format!(
                "Periodic echo pattern: {} echoes, consistency {delay_consistency:.3}",
                echo_delays.len()
            ));
        }
    }

    result
}

/// Method 3: Statistical anomaly analysis.
/// Looks for non-natural statistical properties.
fn analyze_statistical_anomalies(channel: &[f32], _sr: u32) -> MethodResult {
    let mut result = MethodResult::default();
    let mut max_confidence: f64 = 0.0;

    let skew = stats::skewness(channel);
    let kurt = stats::kurtosis(channel);

    let hist = stats::histogram(channel, 100);
    let ent = stats::entropy(&hist);

    if ent < 6.0 {
        result.detected = true;
        max_confidence = max_confidence.max(0.7);
        result
            .details
            .push(format!("Low entropy: {ent:.3} (natural audio > 6.0)"));
    }

    if (kurt - 3.0).abs() > 2.0 {
        result.detected = true;
        max_confidence = max_confidence.max(0.6);
        result
            .details
            .push(format!("Unusual kurtosis: {kurt:.3} (expected ~3.0)"));
    }

    if skew.abs() > 0.5 {
        max_confidence = max_confidence.max(0.5);
        result.details.push(format!("Skewness: {skew:.3}"));
    }

    let fft_result = stft::real_fft(channel);
    let magnitude: Vec<f32> = fft_result.iter().map(|c| c.norm()).collect();
    let mag_f64: Vec<f64> = magnitude.iter().map(|&m| m as f64).collect();
    let total: f64 = mag_f64.iter().sum();
    if total > 1e-10 {
        let probs: Vec<f64> = mag_f64.iter().map(|&m| m / total).collect();
        let spectral_ent = stats::entropy(&probs);
        if spectral_ent < 8.0 {
            result.detected = true;
            max_confidence = max_confidence.max(0.5);
            result
                .details
                .push(format!("Low spectral entropy: {spectral_ent:.3}"));
        }
    }

    result.confidence = max_confidence;
    result
}

/// Method 4: Phase modulation detection.
/// Looks for consistent phase patterns across frequency bins (watermark encoding).
fn detect_phase_modulation(channel: &[f32], _sr: u32) -> MethodResult {
    let mut result = MethodResult::default();

    let nperseg = 2048;
    let noverlap = nperseg * 3 / 4;
    let (spectrogram, _) = stft::stft(channel, nperseg, noverlap);

    if spectrogram.len() < 3 {
        return result;
    }

    let n_freqs = spectrogram[0].len();
    let _n_frames = spectrogram.len();

    let mut phase_stds: Vec<f64> = Vec::new();

    for freq_bin in 0..n_freqs {
        let mut phases: Vec<f64> = spectrogram
            .iter()
            .map(|frame| frame[freq_bin].arg() as f64)
            .collect();

        for t in 1..phases.len() {
            let diff = phases[t] - phases[t - 1];
            if diff > std::f64::consts::PI {
                phases[t] -= 2.0 * std::f64::consts::PI;
            } else if diff < -std::f64::consts::PI {
                phases[t] += 2.0 * std::f64::consts::PI;
            }
        }

        if phases.len() >= 2 {
            let diffs: Vec<f64> = phases.windows(2).map(|w| w[1] - w[0]).collect();
            let mean_diff: f64 = diffs.iter().sum::<f64>() / diffs.len() as f64;
            let std_diff: f64 = {
                let var = diffs.iter().map(|&d| (d - mean_diff).powi(2)).sum::<f64>()
                    / diffs.len() as f64;
                var.sqrt()
            };
            phase_stds.push(std_diff);
        }
    }

    if phase_stds.is_empty() {
        return result;
    }

    let phase_mean: f64 = phase_stds.iter().sum::<f64>() / phase_stds.len() as f64;
    let phase_std: f64 = {
        let var = phase_stds
            .iter()
            .map(|&s| (s - phase_mean).powi(2))
            .sum::<f64>()
            / phase_stds.len() as f64;
        var.sqrt()
    };

    let consistency_score = 1.0 - (phase_mean / (phase_std + 1e-10));
    let consistency_score = consistency_score.clamp(0.0, 1.0);

    if consistency_score > 0.7 {
        result.detected = true;
        result.confidence = consistency_score;
        result
            .details
            .push(format!("Phase consistency score: {consistency_score:.3}"));
    }

    result
}

/// Method 5: Amplitude modulation detection.
/// Looks for modulation patterns in the signal envelope.
fn detect_amplitude_modulation(channel: &[f32], sr: u32) -> MethodResult {
    let mut result = MethodResult::default();

    let env = hilbert::envelope(channel);
    let env_fft = stft::real_fft(&env);
    let env_mag: Vec<f32> = env_fft.iter().map(|c| c.norm()).collect();

    let freq_resolution = sr as f64 / env.len() as f64;

    let low_bin = (1.0 / freq_resolution) as usize;
    let high_bin = (100.0 / freq_resolution) as usize;
    let high_bin = high_bin.min(env_mag.len());

    if low_bin >= high_bin {
        return result;
    }

    let mod_spectrum = &env_mag[low_bin..high_bin];
    let max_power = mod_spectrum.iter().cloned().fold(0.0f32, f32::max);

    if max_power < 1e-10 {
        return result;
    }

    let mod_f64: Vec<f64> = mod_spectrum.iter().map(|&m| m as f64).collect();
    let threshold = max_power as f64 * 0.1;
    let peaks = stats::find_peaks(&mod_f64, threshold, 1);

    if peaks.len() > 5 {
        result.detected = true;
        result.confidence = 0.6;
        result.details.push(format!(
            "Amplitude modulation: {} peaks in 1-100 Hz range",
            peaks.len()
        ));
    }

    result
}

/// Method 6: Frequency domain watermark detection.
/// Checks spectral flatness and peak consistency across multiple window sizes.
fn detect_frequency_domain(channel: &[f32], _sr: u32) -> MethodResult {
    let mut result = MethodResult::default();
    let mut max_confidence: f64 = 0.0;

    for &window_size in &[512, 1024, 2048, 4096] {
        if channel.len() < window_size * 2 {
            continue;
        }

        let noverlap = window_size / 2;
        let (spectrogram, _) = stft::stft(channel, window_size, noverlap);

        if spectrogram.is_empty() {
            continue;
        }

        let mut flatness_values: Vec<f64> = Vec::new();
        for frame in &spectrogram {
            let mag: Vec<f32> = frame.iter().map(|c| c.norm()).collect();
            flatness_values.push(stats::spectral_flatness(&mag));
        }

        let avg_flatness: f64 = flatness_values.iter().sum::<f64>() / flatness_values.len() as f64;
        if avg_flatness > 0.3 {
            result.detected = true;
            max_confidence = max_confidence.max(0.5);
            result.details.push(format!(
                "High spectral flatness: {avg_flatness:.3} (window={window_size})"
            ));
        }

        let mut peak_counts: Vec<f64> = Vec::new();
        for frame in &spectrogram {
            let mag: Vec<f32> = frame.iter().map(|c| c.norm()).collect();
            let max_val = mag.iter().cloned().fold(0.0f32, f32::max);
            let threshold = max_val as f64 * 0.1;
            let mag_f64: Vec<f64> = mag.iter().map(|&m| m as f64).collect();
            let peaks = stats::find_peaks(&mag_f64, threshold, 1);
            peak_counts.push(peaks.len() as f64);
        }

        if peak_counts.len() >= 2 {
            let pc_mean: f64 = peak_counts.iter().sum::<f64>() / peak_counts.len() as f64;
            let pc_std: f64 = {
                let var = peak_counts
                    .iter()
                    .map(|&p| (p - pc_mean).powi(2))
                    .sum::<f64>()
                    / peak_counts.len() as f64;
                var.sqrt()
            };
            let peak_consistency = 1.0 - (pc_std / (pc_mean + 1e-10));

            if peak_consistency > 0.8 {
                result.detected = true;
                max_confidence = max_confidence.max(peak_consistency * 0.7);
                result.details.push(format!(
                    "Peak consistency: {peak_consistency:.3} (window={window_size})"
                ));
            }
        }
    }

    result.confidence = max_confidence;
    result
}

/// Method 7: LSB steganography detection.
/// Analyzes least-significant-bit distribution patterns to detect embedded data.
/// Natural audio has near-random LSBs; steganographic embedding creates statistical bias
/// and periodic patterns aligned to embedding frame boundaries.
fn detect_lsb_steganography(channel: &[f32], _sr: u32) -> MethodResult {
    let mut result = MethodResult::default();
    let mut max_confidence: f64 = 0.0;

    let n = channel.len().min(65536);
    if n < 1024 {
        return result;
    }

    // Convert to 16-bit samples and extract LSB
    let samples_i16: Vec<i16> = channel[..n].iter().map(|&s| (s * 32767.0) as i16).collect();
    let lsb: Vec<u8> = samples_i16.iter().map(|&s| (s & 1) as u8).collect();

    // Test 1: LSB bias — natural audio LSBs should be ~50/50
    let ones: usize = lsb.iter().map(|&b| b as usize).sum();
    let ratio = ones as f64 / n as f64;
    let bias = (ratio - 0.5).abs();

    if bias > 0.02 {
        max_confidence = max_confidence.max(0.4 + bias * 5.0);
        result.details.push(format!(
            "LSB bias: {:.2}% ones (deviation {:.2}%)",
            ratio * 100.0,
            bias * 100.0
        ));
    }

    // Test 2: LSB pair chi-squared — embedded data disrupts natural pair distribution
    let mut pair_counts = [0u64; 4]; // 00, 01, 10, 11
    for pair in lsb.chunks_exact(2) {
        let idx = (pair[0] as usize) * 2 + pair[1] as usize;
        pair_counts[idx] += 1;
    }
    let total_pairs = (n / 2) as f64;
    let expected = total_pairs / 4.0;
    let chi_sq: f64 = pair_counts
        .iter()
        .map(|&c| {
            let diff = c as f64 - expected;
            diff * diff / expected
        })
        .sum();

    // Chi-squared > 7.81 at p=0.05 for df=3
    if chi_sq > 7.81 {
        let conf = (chi_sq / 50.0).min(1.0);
        max_confidence = max_confidence.max(conf);
        result.details.push(format!(
            "LSB pair chi-squared: {chi_sq:.2} (threshold 7.81)"
        ));
    }

    // Test 3: Periodic autocorrelation on LSB stream — frame-aligned embedding
    // creates peaks at the embedding frame size
    let lsb_f: Vec<f64> = lsb.iter().map(|&b| b as f64 - 0.5).collect();
    let test_lags = [128, 256, 441, 512, 576, 1024, 1152, 2048, 2304, 4096, 4608];
    let mut strong_lags = Vec::new();

    for &lag in &test_lags {
        if lag >= lsb_f.len() {
            continue;
        }
        let mut sum = 0.0;
        let count = lsb_f.len() - lag;
        for i in 0..count {
            sum += lsb_f[i] * lsb_f[i + lag];
        }
        let corr = (sum / count as f64 * 4.0).abs();

        if corr > 0.05 {
            strong_lags.push((lag, corr));
        }
    }

    if !strong_lags.is_empty() {
        let best = strong_lags
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        let conf = (best.1 * 2.0).min(1.0);
        max_confidence = max_confidence.max(conf);
        result.details.push(format!(
            "LSB periodic pattern at lag {} (correlation {:.3})",
            best.0, best.1
        ));
    }

    // Test 4: Wald-Wolfowitz runs test — natural LSBs should have random run lengths
    let mut runs = 1usize;
    for i in 1..lsb.len() {
        if lsb[i] != lsb[i - 1] {
            runs += 1;
        }
    }
    let n1 = ones as f64;
    let n2 = (n - ones) as f64;
    let nf = n as f64;
    let expected_runs = 1.0 + 2.0 * n1 * n2 / nf;
    // Correct variance: Var(R) = 2*n1*n2*(2*n1*n2 - n) / (n^2 * (n-1))
    let variance = if nf > 1.0 {
        2.0 * n1 * n2 * (2.0 * n1 * n2 - nf) / (nf * nf * (nf - 1.0))
    } else {
        1.0
    };
    let std_dev = variance.sqrt().max(1.0);
    let runs_z = (runs as f64 - expected_runs).abs() / std_dev;

    if runs_z > 2.58 {
        let conf = (runs_z / 5.0).min(1.0);
        max_confidence = max_confidence.max(conf);
        result.details.push(format!(
            "LSB runs test: z={runs_z:.2} (expected ~{expected_runs:.0}, got {runs})"
        ));
    }

    if max_confidence > 0.3 {
        result.detected = true;
    }

    result.confidence = max_confidence;
    result
}

/// Method 8: Codec artifact fingerprint detection.
/// Detects residual MP3/AAC encoding artifacts even after format conversion.
/// MP3 uses 1152-sample frames which leave periodic spectral discontinuities.
/// Also detects characteristic high-frequency rolloff patterns from lossy codecs.
fn detect_codec_artifacts(channel: &[f32], sr: u32) -> MethodResult {
    let mut result = MethodResult::default();
    let mut max_confidence: f64 = 0.0;

    if channel.len() < 8192 {
        return result;
    }

    // Test 1: MP3 frame boundary detection (1152-sample periodicity)
    // Measure time-domain sample discontinuities at frame boundaries vs random interior positions
    let frame_size = 1152;
    let n_frames = channel.len() / frame_size;
    if n_frames >= 4 {
        let mut boundary_diffs: Vec<f64> = Vec::new();
        let mut interior_diffs: Vec<f64> = Vec::new();

        for f in 0..n_frames.saturating_sub(1) {
            let boundary = f * frame_size + frame_size;
            if boundary + 1 >= channel.len() {
                break;
            }
            // Measure windowed energy change across the frame boundary (4 samples each side)
            let half_win = 4.min(frame_size / 2);
            let left_start = boundary.saturating_sub(half_win);
            let right_end = (boundary + half_win).min(channel.len());
            let left_energy: f64 = channel[left_start..boundary]
                .iter()
                .map(|&s| (s as f64) * (s as f64))
                .sum::<f64>()
                / half_win as f64;
            let right_energy: f64 = channel[boundary..right_end]
                .iter()
                .map(|&s| (s as f64) * (s as f64))
                .sum::<f64>()
                / (right_end - boundary) as f64;
            boundary_diffs.push((left_energy - right_energy).abs());

            // Sample multiple interior positions for robust comparison
            for offset in [frame_size / 4, frame_size / 2, 3 * frame_size / 4] {
                let pos = f * frame_size + offset;
                let l_start = pos.saturating_sub(half_win);
                let r_end = (pos + half_win).min(channel.len());
                if r_end <= pos || l_start >= pos {
                    continue;
                }
                let l_e: f64 = channel[l_start..pos]
                    .iter()
                    .map(|&s| (s as f64) * (s as f64))
                    .sum::<f64>()
                    / (pos - l_start) as f64;
                let r_e: f64 = channel[pos..r_end]
                    .iter()
                    .map(|&s| (s as f64) * (s as f64))
                    .sum::<f64>()
                    / (r_end - pos) as f64;
                interior_diffs.push((l_e - r_e).abs());
            }
        }

        if !boundary_diffs.is_empty() && !interior_diffs.is_empty() {
            let boundary_mean: f64 =
                boundary_diffs.iter().sum::<f64>() / boundary_diffs.len() as f64;
            let interior_mean: f64 =
                interior_diffs.iter().sum::<f64>() / interior_diffs.len() as f64;

            // MP3 artifacts show higher discontinuity at frame boundaries
            if interior_mean > 1e-10 {
                let ratio = boundary_mean / interior_mean;
                if ratio > 1.3 {
                    let conf = ((ratio - 1.0) / 2.0).min(1.0);
                    max_confidence = max_confidence.max(conf);
                    result.details.push(format!(
                        "MP3 frame boundary artifacts: discontinuity ratio {ratio:.2}x (1152-sample period)"
                    ));
                }
            }
        }
    }

    // Test 2: High-frequency rolloff detection
    // Lossy codecs aggressively cut frequencies above a threshold
    let nyquist = sr as f64 / 2.0;
    let nperseg = 4096.min(channel.len() / 2);
    let noverlap = nperseg / 4;
    let (spectrogram, _) = stft::stft(channel, nperseg, noverlap);

    if !spectrogram.is_empty() {
        let n_freqs = spectrogram[0].len();
        let freq_resolution = sr as f64 / nperseg as f64;

        // Average magnitude spectrum across all frames
        let mut avg_spectrum = vec![0.0f64; n_freqs];
        for frame in &spectrogram {
            for (i, val) in frame.iter().enumerate() {
                avg_spectrum[i] += val.norm() as f64;
            }
        }
        for val in &mut avg_spectrum {
            *val /= spectrogram.len() as f64;
        }

        // Find the frequency where energy drops sharply (codec cutoff)
        let max_energy: f64 = avg_spectrum.iter().cloned().fold(0.0, f64::max);
        if max_energy > 1e-10 {
            let threshold = max_energy * 0.01;
            let mut cutoff_bin = n_freqs;

            // Scan from high to low to find where energy rises above threshold
            for i in (n_freqs / 2..n_freqs).rev() {
                if avg_spectrum[i] > threshold {
                    cutoff_bin = i + 1;
                    break;
                }
            }

            let cutoff_freq = cutoff_bin as f64 * freq_resolution;

            // Common lossy codec cutoffs (in Hz)
            let codec_cutoffs = [
                (16000.0, "MP3 128kbps"),
                (17500.0, "MP3 160kbps"),
                (18500.0, "MP3 192kbps"),
                (19500.0, "MP3 256kbps / AAC 128kbps"),
                (20000.0, "MP3 320kbps / AAC 192kbps"),
                (20500.0, "AAC 256kbps"),
            ];

            // Sharp cutoff well below nyquist suggests lossy codec
            if cutoff_freq < nyquist * 0.9 && cutoff_freq > 10000.0 {
                // Check if cutoff is sharp (steep drop)
                let above = cutoff_bin.min(n_freqs);
                let below = cutoff_bin.saturating_sub(10);
                if above > below {
                    let energy_below: f64 = avg_spectrum[below..cutoff_bin.min(n_freqs)]
                        .iter()
                        .sum::<f64>()
                        / (cutoff_bin.min(n_freqs) - below) as f64;
                    let energy_above: f64 = if above < n_freqs {
                        avg_spectrum[above..n_freqs.min(above + 10)]
                            .iter()
                            .sum::<f64>()
                            / (n_freqs.min(above + 10) - above).max(1) as f64
                    } else {
                        0.0
                    };

                    if energy_below > 1e-10 {
                        let drop_ratio = energy_above / energy_below;
                        if drop_ratio < 0.1 {
                            let conf = (1.0 - drop_ratio * 5.0).clamp(0.3, 0.9);
                            max_confidence = max_confidence.max(conf);

                            // Match to known codec profile
                            let mut best_match = "unknown lossy codec";
                            let mut best_dist = f64::MAX;
                            for (freq, name) in &codec_cutoffs {
                                let dist = (cutoff_freq - freq).abs();
                                if dist < best_dist {
                                    best_dist = dist;
                                    best_match = name;
                                }
                            }

                            result.details.push(format!(
                                "Frequency cutoff at {cutoff_freq:.0} Hz — likely {best_match}"
                            ));
                        }
                    }
                }
            }
        }
    }

    // Test 3: Spectral band replication detection (common in AAC HE / MP3 SBR)
    // SBR copies lower frequency content to fill upper bands, creating correlation.
    // Typical SBR crossover: ~5-6 kHz. Source band: just below crossover, replicated band: just above.
    if !spectrogram.is_empty() {
        let n_freqs = spectrogram[0].len();
        let freq_per_bin = sr as f64 / (2.0 * n_freqs as f64);

        // Source band: 2-5 kHz, Replicated band: 5-10 kHz (typical SBR crossover ~5 kHz)
        let src_lo = (2000.0 / freq_per_bin) as usize;
        let src_hi = (5000.0 / freq_per_bin) as usize;
        let rep_lo = (5000.0 / freq_per_bin) as usize;
        let rep_hi = (10000.0 / freq_per_bin) as usize;

        let band_len = src_hi.saturating_sub(src_lo);
        if band_len > 0 && rep_hi <= n_freqs && src_hi <= n_freqs {
            let mut correlation_sum = 0.0f64;
            let mut count = 0usize;

            for frame in spectrogram.iter().take(50) {
                let low_band: Vec<f64> = frame[src_lo..src_hi]
                    .iter()
                    .map(|c| c.norm() as f64)
                    .collect();
                let high_band: Vec<f64> = frame[rep_lo..rep_hi]
                    .iter()
                    .take(low_band.len())
                    .map(|c| c.norm() as f64)
                    .collect();

                if low_band.len() == high_band.len() && !low_band.is_empty() {
                    let corr = stats::pearson_correlation(&low_band, &high_band);
                    if corr.is_finite() {
                        correlation_sum += corr.abs();
                        count += 1;
                    }
                }
            }

            if count > 0 {
                let avg_corr = correlation_sum / count as f64;
                if avg_corr > 0.5 {
                    let conf = (avg_corr * 0.8).min(0.9);
                    max_confidence = max_confidence.max(conf);
                    result.details.push(format!(
                        "Spectral band replication detected: correlation {avg_corr:.3}"
                    ));
                }
            }
        }
    }

    if max_confidence > 0.25 {
        result.detected = true;
    }

    result.confidence = max_confidence;
    result
}

/// Method 9: Phase coherence detection.
/// Detects watermarks embedded via phase manipulation between channels.
/// Only meaningful for stereo/multi-channel audio — mono files are skipped since
/// inter-channel phase coherence is undefined and temporal analysis produces false
/// positives on tonal content.
fn detect_phase_coherence(buffer: &AudioBuffer) -> MethodResult {
    let mut result = MethodResult::default();
    let mut max_confidence: f64 = 0.0;
    let sr = buffer.sample_rate;

    if buffer.num_samples() < 8192 || buffer.is_mono() {
        return result;
    }

    if buffer.is_stereo() {
        // Stereo: analyze inter-channel phase differences
        let left: Vec<f32> = buffer.channel(0).to_vec();
        let right: Vec<f32> = buffer.channel(1).to_vec();

        let nperseg = 2048;
        let noverlap = nperseg * 3 / 4;
        let (spec_l, _) = stft::stft(&left, nperseg, noverlap);
        let (spec_r, _) = stft::stft(&right, nperseg, noverlap);

        let n_frames = spec_l.len().min(spec_r.len());
        if n_frames < 2 {
            return result;
        }
        let n_freqs = spec_l[0].len();

        // Compute phase difference stability per frequency band
        let bands = [
            (100.0, 1000.0, "low-mid"),
            (1000.0, 4000.0, "mid"),
            (4000.0, 8000.0, "high-mid"),
            (8000.0, 16000.0, "high"),
        ];
        let freq_res = sr as f64 / nperseg as f64;

        for &(lo, hi, band_name) in &bands {
            let lo_bin = (lo / freq_res) as usize;
            let hi_bin = ((hi / freq_res) as usize).min(n_freqs);
            if lo_bin >= hi_bin {
                continue;
            }

            // Collect phase differences across frames for this band
            let mut phase_diff_stds: Vec<f64> = Vec::new();

            for bin in lo_bin..hi_bin {
                let diffs: Vec<f64> = (0..n_frames)
                    .map(|f| {
                        let pl = spec_l[f][bin].arg() as f64;
                        let pr = spec_r[f][bin].arg() as f64;
                        let mut d = pl - pr;
                        while d > std::f64::consts::PI {
                            d -= 2.0 * std::f64::consts::PI;
                        }
                        while d < -std::f64::consts::PI {
                            d += 2.0 * std::f64::consts::PI;
                        }
                        d
                    })
                    .collect();

                let mean: f64 = diffs.iter().sum::<f64>() / diffs.len() as f64;
                let var: f64 =
                    diffs.iter().map(|&d| (d - mean).powi(2)).sum::<f64>() / diffs.len() as f64;
                phase_diff_stds.push(var.sqrt());
            }

            if phase_diff_stds.is_empty() {
                continue;
            }

            let avg_std: f64 = phase_diff_stds.iter().sum::<f64>() / phase_diff_stds.len() as f64;
            let locked_ratio = phase_diff_stds.iter().filter(|&&s| s < 0.1).count() as f64
                / phase_diff_stds.len() as f64;

            if locked_ratio > 0.3 {
                let conf = (locked_ratio * 0.9).min(1.0);
                max_confidence = max_confidence.max(conf);
                result.details.push(format!(
                    "Stereo phase lock in {band_name} band: {:.0}% bins locked (avg std {avg_std:.3})",
                    locked_ratio * 100.0
                ));
            }
        }
    } else {
        // Mono: analyze temporal phase consistency across consecutive segments
        let channel: Vec<f32> = buffer.channel(0).to_vec();
        let segment_size = 8192;
        let n_segments = channel.len() / segment_size;

        if n_segments < 3 {
            return result;
        }

        let nperseg = 2048;
        let noverlap = nperseg / 2;

        let mut segment_specs: Vec<Vec<Vec<num_complex::Complex<f32>>>> = Vec::new();
        for s in 0..n_segments.min(16) {
            let start = s * segment_size;
            let end = (start + segment_size).min(channel.len());
            let (spec, _) = stft::stft(&channel[start..end], nperseg, noverlap);
            if !spec.is_empty() {
                segment_specs.push(spec);
            }
        }

        if segment_specs.len() < 3 {
            return result;
        }

        let n_freqs = segment_specs[0][0].len();
        let mut coherence_scores: Vec<f64> = Vec::new();

        for i in 1..segment_specs.len() {
            let frames_a = &segment_specs[i - 1];
            let frames_b = &segment_specs[i];
            let min_frames = frames_a.len().min(frames_b.len());

            if min_frames == 0 {
                continue;
            }

            let mut coherence_sum = 0.0f64;
            let mut count = 0usize;

            for f in 0..min_frames {
                for bin in 1..n_freqs {
                    let pa = frames_a[f][bin].arg() as f64;
                    let pb = frames_b[f][bin].arg() as f64;
                    coherence_sum += (pa - pb).cos();
                    count += 1;
                }
            }

            if count > 0 {
                coherence_scores.push(coherence_sum / count as f64);
            }
        }

        if coherence_scores.is_empty() {
            return result;
        }

        let avg_coherence: f64 =
            coherence_scores.iter().sum::<f64>() / coherence_scores.len() as f64;
        let coherence_std: f64 = {
            let var = coherence_scores
                .iter()
                .map(|&c| (c - avg_coherence).powi(2))
                .sum::<f64>()
                / coherence_scores.len() as f64;
            var.sqrt()
        };

        if avg_coherence > 0.7 && coherence_std < 0.1 {
            let conf = (avg_coherence * 0.8).min(1.0);
            max_confidence = max_confidence.max(conf);
            result.details.push(format!(
                "Temporal phase coherence: avg={avg_coherence:.3}, std={coherence_std:.3}"
            ));
        }

        if coherence_scores.len() >= 4 {
            let diffs: Vec<f64> = coherence_scores
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .collect();
            let avg_diff: f64 = diffs.iter().sum::<f64>() / diffs.len() as f64;

            if avg_diff < 0.05 && avg_coherence > 0.5 {
                let conf = ((0.1 - avg_diff) * 10.0).min(0.9);
                max_confidence = max_confidence.max(conf);
                result.details.push(format!(
                    "Periodic phase pattern: segment-to-segment variation {avg_diff:.4}"
                ));
            }
        }
    }

    if max_confidence > 0.3 {
        result.detected = true;
    }

    result.confidence = max_confidence;
    result
}

/// Method 10: Spatial encoding detection.
/// Detects watermarks embedded in spatial/surround encoding patterns.
/// Analyzes mid/side signal ratios, inter-channel correlation at specific frequencies,
/// and spatial encoding signatures common in broadcast and streaming audio.
fn detect_spatial_encoding(buffer: &AudioBuffer) -> MethodResult {
    let mut result = MethodResult::default();
    let mut max_confidence: f64 = 0.0;

    if buffer.num_samples() < 8192 {
        return result;
    }

    if buffer.num_channels() >= 2 {
        let left: Vec<f32> = buffer.channel(0).to_vec();
        let right: Vec<f32> = buffer.channel(1).to_vec();
        let sr = buffer.sample_rate;
        let n = left.len().min(right.len());

        // Test 1: Mid/side ratio analysis
        let mid: Vec<f32> = (0..n).map(|i| (left[i] + right[i]) * 0.5).collect();
        let side: Vec<f32> = (0..n).map(|i| (left[i] - right[i]) * 0.5).collect();

        let nperseg = 2048;
        let noverlap = nperseg * 3 / 4;
        let (spec_mid, _) = stft::stft(&mid, nperseg, noverlap);
        let (spec_side, _) = stft::stft(&side, nperseg, noverlap);

        if !spec_mid.is_empty() && !spec_side.is_empty() {
            let n_frames = spec_mid.len().min(spec_side.len());
            let n_freqs = spec_mid[0].len();
            let freq_res = sr as f64 / nperseg as f64;

            let bands: &[(f64, f64, &str)] = &[
                (200.0, 2000.0, "low"),
                (2000.0, 6000.0, "mid"),
                (6000.0, 12000.0, "high"),
                (12000.0, 20000.0, "ultrasonic"),
            ];

            for &(lo, hi, band_name) in bands {
                if lo >= sr as f64 / 2.0 {
                    continue;
                }
                let lo_bin = (lo / freq_res) as usize;
                let hi_bin = ((hi / freq_res) as usize).min(n_freqs);
                if lo_bin >= hi_bin {
                    continue;
                }

                let mut ms_ratios: Vec<f64> = Vec::new();
                for f in 0..n_frames {
                    let mut mid_energy = 0.0f64;
                    let mut side_energy = 0.0f64;
                    for bin in lo_bin..hi_bin {
                        mid_energy += spec_mid[f][bin].norm() as f64;
                        side_energy += spec_side[f][bin].norm() as f64;
                    }
                    if mid_energy > 1e-10 {
                        ms_ratios.push(side_energy / mid_energy);
                    }
                }

                if ms_ratios.is_empty() {
                    continue;
                }

                let avg_ratio: f64 = ms_ratios.iter().sum::<f64>() / ms_ratios.len() as f64;
                let ratio_std: f64 = {
                    let var = ms_ratios
                        .iter()
                        .map(|&r| (r - avg_ratio).powi(2))
                        .sum::<f64>()
                        / ms_ratios.len() as f64;
                    var.sqrt()
                };

                // Unnaturally stable M/S ratio suggests spatial encoding watermark
                if ratio_std < 0.02 && avg_ratio > 0.01 && avg_ratio < 0.5 {
                    let conf = ((0.05 - ratio_std) * 20.0).clamp(0.3, 0.9);
                    max_confidence = max_confidence.max(conf);
                    result.details.push(format!(
                        "Stable M/S ratio in {band_name} band: avg={avg_ratio:.3}, std={ratio_std:.4}"
                    ));
                }
            }

            // Test 2: Frequency-specific inter-channel correlation
            let test_freqs = [100.0, 500.0, 1000.0, 4000.0, 8000.0, 16000.0];
            let mut anomalous_freqs = Vec::new();

            let (spec_l, _) = stft::stft(&left, nperseg, noverlap);
            let (spec_r, _) = stft::stft(&right, nperseg, noverlap);
            let nf = spec_l.len().min(spec_r.len());

            for &freq in &test_freqs {
                if freq >= sr as f64 / 2.0 {
                    continue;
                }
                let bin = (freq / freq_res) as usize;
                if bin >= n_freqs {
                    continue;
                }

                let l_mags: Vec<f64> = (0..nf).map(|f| spec_l[f][bin].norm() as f64).collect();
                let r_mags: Vec<f64> = (0..nf).map(|f| spec_r[f][bin].norm() as f64).collect();

                if l_mags.len() >= 4 {
                    let corr = stats::pearson_correlation(&l_mags, &r_mags);
                    if corr.is_finite() && corr < -0.8 {
                        anomalous_freqs.push((freq, corr));
                    }
                }
            }

            if !anomalous_freqs.is_empty() {
                let conf = (0.5 + anomalous_freqs.len() as f64 * 0.15).min(0.9);
                max_confidence = max_confidence.max(conf);
                let freq_list: Vec<String> = anomalous_freqs
                    .iter()
                    .map(|(f, c)| format!("{f:.0}Hz(r={c:.2})"))
                    .collect();
                result.details.push(format!(
                    "Anti-correlated channels at: {}",
                    freq_list.join(", ")
                ));
            }
        }

        // Test 3: Side-channel energy ratio
        let side_rms: f64 =
            (side.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / side.len() as f64).sqrt();
        let mid_rms: f64 =
            (mid.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / mid.len() as f64).sqrt();

        if mid_rms > 1e-10 {
            let global_ms = side_rms / mid_rms;
            if global_ms > 0.8 {
                let conf = ((global_ms - 0.5) * 1.5).clamp(0.3, 0.8);
                max_confidence = max_confidence.max(conf);
                result.details.push(format!(
                    "High side-channel energy: M/S ratio {global_ms:.3}"
                ));
            }
        }
    } else {
        // Mono: check for narrowband spectral insertions (spatial encoding markers)
        let channel: Vec<f32> = buffer.channel(0).to_vec();
        let sr = buffer.sample_rate;

        let nperseg = 4096;
        let noverlap = nperseg * 3 / 4;
        let (spectrogram, _) = stft::stft(&channel, nperseg, noverlap);

        if spectrogram.len() < 4 {
            return result;
        }

        let n_freqs = spectrogram[0].len();
        let freq_res = sr as f64 / nperseg as f64;

        let mut avg_spectrum = vec![0.0f64; n_freqs];
        for frame in &spectrogram {
            for (i, val) in frame.iter().enumerate() {
                avg_spectrum[i] += val.norm() as f64;
            }
        }
        for val in &mut avg_spectrum {
            *val /= spectrogram.len() as f64;
        }

        let spectral_median = {
            let mut sorted = avg_spectrum.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            sorted[sorted.len() / 2]
        };

        let mut narrow_peaks = 0usize;
        for bin in 2..n_freqs.saturating_sub(2) {
            let val = avg_spectrum[bin];
            let neighbors = (avg_spectrum[bin - 2]
                + avg_spectrum[bin - 1]
                + avg_spectrum[bin + 1]
                + avg_spectrum[bin + 2])
                / 4.0;
            if val > neighbors * 3.0 && val > spectral_median * 10.0 {
                let freq = bin as f64 * freq_res;
                if (200.0..20000.0).contains(&freq) {
                    narrow_peaks += 1;
                }
            }
        }

        if narrow_peaks >= 3 {
            let conf = (narrow_peaks as f64 * 0.12).min(0.85);
            max_confidence = max_confidence.max(conf);
            result.details.push(format!(
                "Narrowband spectral insertions: {narrow_peaks} suspicious peaks"
            ));
        }
    }

    if max_confidence > 0.3 {
        result.detected = true;
    }

    result.confidence = max_confidence;
    result
}
