use std::collections::HashMap;

use serde::Serialize;

use crate::audio::AudioBuffer;
use crate::sanitization::dsp::{hilbert, stats, stft};

/// Results from watermark detection.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WatermarkResult {
    pub detected: Vec<WatermarkDetection>,
    pub method_results: HashMap<String, MethodResult>,
    pub overall_confidence: f64,
    pub watermark_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct WatermarkDetection {
    pub method: String,
    pub confidence: f64,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MethodResult {
    pub detected: bool,
    pub confidence: f64,
    pub details: Vec<String>,
}

/// Known watermark carrier frequencies (Hz).
const WATERMARK_FREQS: [f64; 4] = [18000.0, 19000.0, 20000.0, 21000.0];

/// Watermark detector - runs multiple detection methods.
pub struct WatermarkDetector;

impl WatermarkDetector {
    /// Run all detection methods on the audio buffer.
    pub fn detect_all(buffer: &AudioBuffer) -> WatermarkResult {
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
        ];

        for (name, method) in &methods {
            let mr = method(&channel, sr);
            if mr.detected {
                result.detected.push(WatermarkDetection {
                    method: name.to_string(),
                    confidence: mr.confidence,
                    description: mr.details.first().cloned().unwrap_or_default(),
                });
                result.watermark_count += 1;
            }
            result.method_results.insert(name.to_string(), mr);
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
