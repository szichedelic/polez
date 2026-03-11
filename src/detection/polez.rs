//! EXPERIMENTAL AI watermark detection — NOT validated against real data.
//!
//! This detector uses heuristic signals that have NOT been calibrated against
//! known AI-watermarked audio (Suno, Udio, MusicGen, etc.).  Results should be
//! treated as speculative and may have high false-positive rates on normal audio.
//!
//! Detection signals:
//! 1. Ultrasonic energy ratio (near-Nyquist vs reference band)
//! 2. Bit plane bias across 8 LSB planes
//! 3. LSB autocorrelation at fixed periods

use serde::Serialize;

use crate::audio::AudioBuffer;

/// Result of EXPERIMENTAL AI watermark analysis.
///
/// These results have not been validated against real AI-watermarked audio and
/// may produce false positives on normal audio.
#[derive(Debug, Clone, Serialize)]
pub struct PolezDetectionResult {
    /// Overall probability this is AI-generated (0.0 - 1.0).
    /// NOT calibrated — treat as a heuristic score, not a true probability.
    pub detection_probability: f64,
    /// Confidence in the detection (0.0 - 1.0)
    pub confidence: f64,
    /// Individual signal scores
    pub signals: PolezSignals,
    /// Human-readable verdict
    pub verdict: &'static str,
    /// Whether this detector has been validated against real data.
    pub validated: bool,
}

/// Individual detection signals.
#[derive(Debug, Clone, Serialize)]
pub struct PolezSignals {
    /// Ratio of energy in 23-24kHz vs 15-20kHz (AI watermark: >0.1, Human: <0.02)
    pub ultrasonic_ratio: f64,
    /// Ultrasonic score contribution (0-1)
    pub ultrasonic_score: f64,

    /// Average deviation of bit planes from 0.5 (AI watermark: >0.02, Human: <0.01)
    pub bit_plane_bias: f64,
    /// Number of bit planes with significant bias (AI watermark: 6-8, Human: 0-2)
    pub biased_planes: u8,
    /// Bit plane score contribution (0-1)
    pub bit_plane_score: f64,

    /// Maximum autocorrelation value at tested periods
    pub max_autocorr: f64,
    /// Period with strongest autocorrelation
    pub autocorr_period: usize,
    /// Autocorrelation score contribution (0-1)
    pub autocorr_score: f64,
}

/// AI watermark detector.
pub struct PolezDetector;

impl PolezDetector {
    /// Analyze audio for AI watermark signatures.
    pub fn detect(buffer: &AudioBuffer) -> PolezDetectionResult {
        let samples = buffer.to_mono_samples();
        let sample_rate = buffer.sample_rate;

        // Signal 1: Ultrasonic energy analysis
        let (ultrasonic_ratio, ultrasonic_score) = Self::analyze_ultrasonic(&samples, sample_rate);

        // Signal 2: Bit plane bias analysis
        let (bit_plane_bias, biased_planes, bit_plane_score) = Self::analyze_bit_planes(&samples);

        // Signal 3: Autocorrelation analysis
        let (max_autocorr, autocorr_period, autocorr_score) =
            Self::analyze_autocorrelation(&samples);

        // Weighted combination of signals
        // Ultrasonic is strongest indicator, followed by bit planes, then autocorr
        let weights = (0.45, 0.35, 0.20); // ultrasonic, bit_plane, autocorr

        let raw_probability =
            ultrasonic_score * weights.0 + bit_plane_score * weights.1 + autocorr_score * weights.2;

        // Confidence based on signal agreement
        let scores = [ultrasonic_score, bit_plane_score, autocorr_score];
        let mean = scores.iter().sum::<f64>() / 3.0;
        let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / 3.0;
        let confidence = 1.0 - variance.sqrt(); // Higher agreement = higher confidence

        // Calibrate probability (apply sigmoid-like curve for better separation)
        let detection_probability = Self::calibrate_probability(raw_probability);

        let verdict = match (detection_probability, confidence) {
            (p, c) if p > 0.8 && c > 0.6 => {
                "EXPERIMENTAL: Heuristic signals elevated — unvalidated, may be false positive"
            }
            (p, _) if p > 0.7 => {
                "EXPERIMENTAL: Some heuristic signals present — unvalidated, treat with caution"
            }
            (p, _) if p > 0.5 => {
                "EXPERIMENTAL: Weak heuristic signals — likely normal audio characteristics"
            }
            _ => "No heuristic signals detected",
        };

        PolezDetectionResult {
            detection_probability,
            confidence,
            signals: PolezSignals {
                ultrasonic_ratio,
                ultrasonic_score,
                bit_plane_bias,
                biased_planes,
                bit_plane_score,
                max_autocorr,
                autocorr_period,
                autocorr_score,
            },
            verdict,
            validated: false,
        }
    }

    /// Analyze ultrasonic frequency content near Nyquist vs a reference band.
    ///
    /// At 48kHz+, examines 23-24 kHz vs 15-20 kHz.
    /// At 44.1kHz, scales bands to 20-21.5 kHz vs 13-18 kHz (below Nyquist of 22.05 kHz).
    /// Below 44.1kHz, returns zero (insufficient bandwidth).
    fn analyze_ultrasonic(samples: &[f32], sample_rate: u32) -> (f64, f64) {
        let sample_rate = sample_rate as f64;
        let nyquist = sample_rate / 2.0;

        // Need at least 44.1kHz for meaningful near-Nyquist analysis
        if sample_rate < 44100.0 {
            return (0.0, 0.0);
        }

        // Scale frequency bands to stay below Nyquist
        let (ultra_lo, ultra_hi, ref_lo, ref_hi) = if nyquist >= 24000.0 {
            (23000.0, 24000.0, 15000.0, 20000.0)
        } else {
            // 44.1kHz: Nyquist = 22050 Hz — use bands safely below it
            (20000.0, 21500.0, 13000.0, 18000.0)
        };

        // Take a chunk from the middle of the audio (avoid silence at start/end)
        let chunk_size = 65536.min(samples.len());
        let start = (samples.len() / 2).saturating_sub(chunk_size / 2);
        let chunk: Vec<f64> = samples[start..start + chunk_size]
            .iter()
            .map(|s| *s as f64)
            .collect();

        // Apply Hann window
        let windowed: Vec<f64> = chunk
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let w =
                    0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / chunk_size as f64).cos());
                s * w
            })
            .collect();

        // Compute FFT magnitudes using DFT for specific frequency ranges
        let freq_resolution = sample_rate / chunk_size as f64;

        // Calculate energy in bands
        let mut ultrasonic_energy = 0.0;
        let mut reference_energy = 0.0;

        let ultrasonic_bins = (
            (ultra_lo / freq_resolution) as usize,
            (ultra_hi / freq_resolution) as usize,
        );
        let reference_bins = (
            (ref_lo / freq_resolution) as usize,
            (ref_hi / freq_resolution) as usize,
        );

        // Compute DFT for specific bins (more efficient than full FFT)
        for k in reference_bins.0..=reference_bins.1.min(chunk_size / 2) {
            let (re, im) = Self::dft_bin(&windowed, k);
            reference_energy += (re * re + im * im).sqrt();
        }

        for k in ultrasonic_bins.0..=ultrasonic_bins.1.min(chunk_size / 2) {
            let (re, im) = Self::dft_bin(&windowed, k);
            ultrasonic_energy += (re * re + im * im).sqrt();
        }

        // Normalize by number of bins
        let ultrasonic_bins_count = (ultrasonic_bins.1 - ultrasonic_bins.0 + 1) as f64;
        let reference_bins_count = (reference_bins.1 - reference_bins.0 + 1) as f64;

        ultrasonic_energy /= ultrasonic_bins_count;
        reference_energy /= reference_bins_count;

        // Calculate ratio
        let ratio = if reference_energy > 1e-10 {
            ultrasonic_energy / reference_energy
        } else {
            0.0
        };

        // Score: AI watermark typically has ratio > 0.1, human < 0.02
        // Map to 0-1 score with threshold around 0.05
        let score = Self::sigmoid((ratio - 0.05) * 30.0);

        (ratio, score)
    }

    /// Compute single DFT bin.
    fn dft_bin(samples: &[f64], k: usize) -> (f64, f64) {
        let n = samples.len();
        let mut re = 0.0;
        let mut im = 0.0;
        for (i, sample) in samples.iter().enumerate() {
            let angle = 2.0 * std::f64::consts::PI * k as f64 * i as f64 / n as f64;
            re += sample * angle.cos();
            im -= sample * angle.sin();
        }
        (re, im)
    }

    /// Analyze bit plane bias.
    fn analyze_bit_planes(samples: &[f32]) -> (f64, u8, f64) {
        // Skip silence at start
        let start = samples.iter().position(|s| s.abs() > 0.001).unwrap_or(0);

        let analysis_samples: Vec<i16> = samples[start..]
            .iter()
            .take(50000)
            .map(|s| (*s * 32767.0) as i16)
            .collect();

        if analysis_samples.len() < 1000 {
            return (0.0, 0, 0.0);
        }

        let mut total_bias = 0.0;
        let mut biased_planes = 0u8;

        for bit in 0..8 {
            let ones: usize = analysis_samples
                .iter()
                .map(|s| ((s >> bit) & 1) as usize)
                .sum();
            let ratio = ones as f64 / analysis_samples.len() as f64;
            let bias = (ratio - 0.5).abs();

            total_bias += bias;

            // Count as biased if deviation > 1%
            if bias > 0.01 {
                biased_planes += 1;
            }
        }

        let avg_bias = total_bias / 8.0;

        // Score: AI watermark typically has 6-8 biased planes, human has 0-2
        // And higher average bias
        let plane_score = (biased_planes as f64 / 8.0).min(1.0);
        let bias_score = Self::sigmoid((avg_bias - 0.01) * 100.0);

        let combined_score = plane_score * 0.6 + bias_score * 0.4;

        (avg_bias, biased_planes, combined_score)
    }

    /// Analyze autocorrelation for periodic patterns.
    fn analyze_autocorrelation(samples: &[f32]) -> (f64, usize, f64) {
        // Skip silence
        let start = samples.iter().position(|s| s.abs() > 0.001).unwrap_or(0);

        let analysis_samples: Vec<i16> = samples[start..]
            .iter()
            .take(10000)
            .map(|s| (*s * 32767.0) as i16)
            .collect();

        if analysis_samples.len() < 2048 {
            return (0.0, 0, 0.0);
        }

        // Extract LSB
        let lsb: Vec<f64> = analysis_samples
            .iter()
            .map(|s| (s & 1) as f64 - 0.5)
            .collect();

        // Test specific periods
        let test_periods = [2, 4, 8, 16, 32, 64, 128, 256, 512, 1024];
        let mut max_autocorr = 0.0;
        let mut best_period = 0;

        for &lag in &test_periods {
            if lag >= lsb.len() {
                continue;
            }

            let mut sum = 0.0;
            for i in 0..(lsb.len() - lag) {
                sum += lsb[i] * lsb[i + lag];
            }
            let corr = (sum / (lsb.len() - lag) as f64 * 4.0).abs();

            if corr > max_autocorr {
                max_autocorr = corr;
                best_period = lag;
            }
        }

        // Score: AI watermark typically has autocorr > 0.05 at period 2
        let score = Self::sigmoid((max_autocorr - 0.03) * 30.0);

        (max_autocorr, best_period, score)
    }

    /// Sigmoid function for smooth scoring.
    fn sigmoid(x: f64) -> f64 {
        1.0 / (1.0 + (-x).exp())
    }

    /// Calibrate raw probability for better separation.
    fn calibrate_probability(raw: f64) -> f64 {
        // Apply slight S-curve to push values away from 0.5
        Self::sigmoid((raw - 0.5) * 4.0)
    }
}
