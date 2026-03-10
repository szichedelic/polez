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

        for (name, method) in &methods {
            if let Some(filter) = filter {
                if !filter.iter().any(|f| f == name) {
                    continue;
                }
            }
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

        // Phase coherence needs the full buffer (stereo analysis)
        let run_phase_coherence = match filter {
            Some(f) => f.iter().any(|name| name == "phase_coherence"),
            None => true,
        };
        if run_phase_coherence {
            let mr = detect_phase_coherence(buffer);
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

    // Test 4: Runs test — natural LSBs should have random run lengths
    let mut runs = 1usize;
    for i in 1..lsb.len() {
        if lsb[i] != lsb[i - 1] {
            runs += 1;
        }
    }
    let expected_runs = 1.0 + 2.0 * ones as f64 * (n - ones) as f64 / n as f64;
    let runs_z = (runs as f64 - expected_runs).abs() / (expected_runs * 0.5).sqrt().max(1.0);

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
    // Compute energy at each sample position, then look for periodic dips at frame boundaries
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
            // Energy discontinuity at frame boundary
            let diff = (channel[boundary] as f64 - channel[boundary - 1] as f64).abs();
            boundary_diffs.push(diff);

            // Compare with interior sample differences
            let mid = f * frame_size + frame_size / 2;
            if mid + 1 < channel.len() {
                let mid_diff = (channel[mid] as f64 - channel[mid - 1] as f64).abs();
                interior_diffs.push(mid_diff);
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
    // SBR copies lower frequency content to fill upper bands, creating correlation
    if !spectrogram.is_empty() {
        let n_freqs = spectrogram[0].len();
        let mid_bin = n_freqs / 2;
        let quarter_bin = n_freqs / 4;

        if mid_bin > quarter_bin && quarter_bin > 0 {
            let mut correlation_sum = 0.0f64;
            let mut count = 0usize;

            for frame in spectrogram.iter().take(50) {
                let low_band: Vec<f64> = frame[quarter_bin..mid_bin]
                    .iter()
                    .map(|c| c.norm() as f64)
                    .collect();
                let high_band: Vec<f64> = frame[mid_bin..mid_bin + (mid_bin - quarter_bin)]
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
/// Detects watermarks embedded via phase manipulation between channels or time segments.
/// For stereo: analyzes inter-channel phase difference statistics.
/// For mono: analyzes temporal phase consistency across segments.
fn detect_phase_coherence(buffer: &AudioBuffer) -> MethodResult {
    let mut result = MethodResult::default();
    let mut max_confidence: f64 = 0.0;
    let sr = buffer.sample_rate;

    if buffer.num_samples() < 8192 {
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
