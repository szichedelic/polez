//! FFT-based spectral cleaning for watermark removal.
//!
//! Detects and suppresses watermarks embedded in high-frequency bands using
//! STFT analysis, notch filtering, spectral smoothing, and psychoacoustic masking.

use num_complex::Complex;
use rand::Rng;

use super::dsp::{biquad, filtfilt, stft};
use super::psychoacoustic::MaskingModel;
use crate::audio::AudioBuffer;
use crate::config::AdvancedFlags;
use crate::error::Result;

/// Watermark frequency bands to target: (center_hz, bandwidth_hz)
const WATERMARK_BANDS: [(f64, f64); 4] = [
    (18000.0, 500.0),
    (19000.0, 500.0),
    (20000.0, 500.0),
    (21000.0, 500.0),
];

/// Known sync tone frequencies to remove.
const SYNC_TONE_RANGE: (f64, f64) = (1000.0, 15000.0);

/// Spectral cleaning - FFT-based watermark removal.
pub struct SpectralCleaner;

impl SpectralCleaner {
    /// Run all spectral cleaning methods. Returns (patterns_found, patterns_suppressed).
    ///
    /// Computes STFT once per channel, applies all spectral modifications to the
    /// shared spectrogram, then reconstructs via a single ISTFT. This avoids
    /// redundant FFT work across periodic disruption, spectral smoothing, and
    /// spread-spectrum attenuation.
    /// Run all spectral cleaning methods. Returns (patterns_found, patterns_suppressed).
    ///
    /// When `freq_ranges` is non-empty, only bins within those Hz ranges are modified.
    /// An empty `freq_ranges` means full-spectrum cleaning (default behavior).
    pub fn clean(
        buffer: &mut AudioBuffer,
        paranoid: bool,
        _flags: &AdvancedFlags,
        freq_ranges: &[(f64, f64)],
    ) -> Result<(usize, usize)> {
        let mut found = 0;
        let mut suppressed = 0;

        let sr = buffer.sample_rate;

        for ch in 0..buffer.num_channels() {
            let channel: Vec<f32> = buffer.channel(ch).to_vec();

            // Detection pass uses different overlap, kept separate
            let (f1, s1) = remove_high_frequency_watermarks(&channel, sr);
            let mut cleaned = if s1 > 0 {
                notch_watermark_bands(&channel, sr)
            } else {
                channel.clone()
            };
            found += f1;
            suppressed += s1;

            // Single STFT for all spectral modification passes
            let nperseg = 2048.min(cleaned.len() / 4).max(256);
            let noverlap = nperseg * 3 / 4;
            let (mut spectrogram, orig_len) = stft::stft(&cleaned, nperseg, noverlap);

            if !spectrogram.is_empty() {
                let freq_resolution = sr as f64 / nperseg as f64;
                let high_freq_start = (15000.0 / freq_resolution) as usize;

                // Compute psychoacoustic masking thresholds per frame
                let masking = MaskingModel::new(sr as f64, nperseg);
                let thresholds: Vec<Vec<f32>> = spectrogram
                    .iter()
                    .map(|frame| masking.compute_threshold(frame))
                    .collect();

                let (f2, s2) = apply_periodic_disruption(
                    &mut spectrogram,
                    high_freq_start,
                    paranoid,
                    freq_ranges,
                    freq_resolution,
                    &thresholds,
                );
                found += f2;
                suppressed += s2;

                apply_spectral_smoothing(
                    &mut spectrogram,
                    high_freq_start,
                    freq_ranges,
                    freq_resolution,
                    &thresholds,
                );
                apply_spread_spectrum_attenuation(
                    &mut spectrogram,
                    high_freq_start,
                    freq_ranges,
                    freq_resolution,
                    &thresholds,
                );

                // Single ISTFT reconstruction
                let reconstructed = stft::istft(&spectrogram, nperseg, noverlap, orig_len);
                let copy_len = cleaned.len().min(reconstructed.len());
                cleaned[..copy_len].copy_from_slice(&reconstructed[..copy_len]);
            }

            // Noise shaping operates in time domain, no STFT needed
            adaptive_noise_shaping(&mut cleaned, sr, paranoid);

            let mut ch_view = buffer.channel_mut(ch);
            for (i, &val) in cleaned.iter().enumerate().take(ch_view.len()) {
                ch_view[i] = val;
            }
        }

        Ok((found, suppressed))
    }

    /// Apply adaptive ultrasonic notch filter as a final post-processing step.
    /// Must run after all other spectral cleaning and multi-pass processing to
    /// prevent STFT operations from undoing the notch filter's work.
    ///
    /// Two-stage approach:
    /// 1. Scan for anomalous peaks and apply targeted LP + notch filters
    /// 2. Unconditionally apply a gentle ultrasonic shelf above 21kHz to ensure
    ///    the 23-24kHz watermark band can't survive relative to 15-20kHz
    pub fn adaptive_notch_pass(buffer: &mut AudioBuffer, paranoid: bool) -> Result<usize> {
        let sr = buffer.sample_rate;
        let nyquist = sr as f64 / 2.0;
        let mut total_peaks = 0;

        for ch in 0..buffer.num_channels() {
            let channel: Vec<f32> = buffer.channel(ch).to_vec();

            // Stage 1: Scan-based targeted removal
            let peaks = scan_ultrasonic_peaks(&channel, sr);
            let mut cleaned = if !peaks.is_empty() {
                total_peaks += peaks.len();
                apply_adaptive_notches(&channel, sr, &peaks, paranoid)
            } else {
                channel
            };

            // Stage 2: single LP at 22.5kHz
            if nyquist > 22500.0 {
                let lp_coeffs = biquad::butterworth_lowpass(22500.0, sr);
                cleaned = filtfilt::filtfilt(&cleaned, &lp_coeffs);
                total_peaks = total_peaks.max(1);
            }

            let mut ch_view = buffer.channel_mut(ch);
            for (i, &val) in cleaned.iter().enumerate().take(ch_view.len()) {
                ch_view[i] = val;
            }
        }

        Ok(total_peaks)
    }
}

/// Check if a frequency bin falls within the user-specified frequency ranges.
/// If no ranges are specified (empty slice), all bins are in range (full spectrum).
fn bin_in_range(bin: usize, freq_resolution: f64, freq_ranges: &[(f64, f64)]) -> bool {
    if freq_ranges.is_empty() {
        return true;
    }
    let freq = bin as f64 * freq_resolution;
    freq_ranges.iter().any(|&(lo, hi)| freq >= lo && freq <= hi)
}

/// Detect and count watermarks in known frequency bands.
fn remove_high_frequency_watermarks(channel: &[f32], sr: u32) -> (usize, usize) {
    let mut found = 0;
    let mut suppressed = 0;

    let nperseg = 2048.min(channel.len() / 4).max(256);
    let noverlap = nperseg / 2;
    let (spectrogram, _) = stft::stft(channel, nperseg, noverlap);

    if spectrogram.is_empty() {
        return (0, 0);
    }

    let freq_resolution = sr as f64 / nperseg as f64;

    for &(center, _bw) in &WATERMARK_BANDS {
        if center >= sr as f64 / 2.0 {
            continue;
        }
        let bin = (center / freq_resolution) as usize;
        if bin >= spectrogram[0].len() {
            continue;
        }

        let band_power: f64 = spectrogram
            .iter()
            .map(|f| f[bin].norm() as f64)
            .sum::<f64>()
            / spectrogram.len() as f64;
        let total_power: f64 = spectrogram
            .iter()
            .map(|f| f.iter().map(|c| c.norm() as f64).sum::<f64>())
            .sum::<f64>()
            / spectrogram.len() as f64
            / spectrogram[0].len() as f64;

        if band_power > total_power * 2.0 {
            found += 1;
            suppressed += 1;
        }
    }

    (found, suppressed)
}

/// Apply notch filters at known watermark carrier frequencies.
fn notch_watermark_bands(channel: &[f32], sr: u32) -> Vec<f32> {
    let mut output = channel.to_vec();

    for &(center, _bw) in &WATERMARK_BANDS {
        if center >= sr as f64 / 2.0 * 0.95 {
            continue;
        }
        let coeffs = biquad::notch_filter(center, 30.0, sr);
        output = filtfilt::filtfilt(&output, &coeffs);
    }

    output
}

/// Disrupt periodic patterns by randomizing phase in high-frequency bins.
/// Operates on an already-computed spectrogram (no STFT/ISTFT).
/// Only modifies bins whose magnitude is below the psychoacoustic masking
/// threshold, preserving perceptually important components.
fn apply_periodic_disruption(
    spectrogram: &mut [Vec<Complex<f32>>],
    high_freq_start: usize,
    paranoid: bool,
    freq_ranges: &[(f64, f64)],
    freq_resolution: f64,
    thresholds: &[Vec<f32>],
) -> (usize, usize) {
    let mut rng = rand::thread_rng();
    let phase_noise = if paranoid { 0.05 } else { 0.02 };
    let mut found = 0;

    for (frame_idx, frame) in spectrogram.iter_mut().enumerate() {
        let thresh = &thresholds[frame_idx];
        for (bin, val) in frame.iter_mut().enumerate().skip(high_freq_start) {
            if !bin_in_range(bin, freq_resolution, freq_ranges) {
                continue;
            }
            let mag = val.norm();
            // In paranoid mode, modify all bins; otherwise respect masking threshold
            if !paranoid && bin < thresh.len() && !MaskingModel::is_masked(mag, thresh[bin]) {
                continue;
            }
            let phase = val.arg();
            let new_phase = phase + rng.gen_range(-phase_noise..phase_noise) as f32;
            *val = Complex::from_polar(mag, new_phase);
            found += 1;
        }
    }

    (found, found)
}

/// Apply spectral smoothing to reduce sharp watermark features.
/// Operates on an already-computed spectrogram (no STFT/ISTFT).
///
/// Only smooths bins above 15kHz where watermarks live. Uses magnitude-only
/// averaging (preserving original phase) to avoid the massive signal
/// cancellation that complex-domain averaging causes.
/// Respects psychoacoustic masking: bins above the masking threshold are skipped.
fn apply_spectral_smoothing(
    spectrogram: &mut [Vec<Complex<f32>>],
    high_freq_start: usize,
    freq_ranges: &[(f64, f64)],
    freq_resolution: f64,
    thresholds: &[Vec<f32>],
) {
    let window = 5;
    let half = window / 2;

    for (frame_idx, frame) in spectrogram.iter_mut().enumerate() {
        let thresh = &thresholds[frame_idx];
        let original: Vec<Complex<f32>> = frame.clone();
        let start = high_freq_start.max(half);
        for i in start..frame.len().saturating_sub(half) {
            if !bin_in_range(i, freq_resolution, freq_ranges) {
                continue;
            }
            // Skip perceptually important bins
            if i < thresh.len() && !MaskingModel::is_masked(original[i].norm(), thresh[i]) {
                continue;
            }
            let avg_mag = original[i - half..=i + half]
                .iter()
                .map(|c| c.norm())
                .sum::<f32>()
                / window as f32;
            let phase = original[i].arg();
            frame[i] = Complex::from_polar(avg_mag, phase);
        }
    }
}

/// Adaptive noise shaping - add shaped noise to mask watermarks.
fn adaptive_noise_shaping(channel: &mut [f32], sr: u32, paranoid: bool) {
    let noise_level: f32 = if paranoid { 1.8e-8 } else { 9e-9 };
    let mut rng = rand::thread_rng();

    let coeffs = biquad::butterworth_highpass(8000.0, sr);

    let noise: Vec<f32> = (0..channel.len())
        .map(|_| rng.gen_range(-noise_level..noise_level))
        .collect();

    let shaped = biquad::biquad_process(&noise, &coeffs);

    for (s, &n) in channel.iter_mut().zip(shaped.iter()) {
        *s += n;
    }
}

/// Attenuate spread-spectrum patterns in high-frequency bins.
/// Operates on an already-computed spectrogram (no STFT/ISTFT).
/// Respects psychoacoustic masking: only attenuates bins below the threshold.
fn apply_spread_spectrum_attenuation(
    spectrogram: &mut [Vec<Complex<f32>>],
    high_freq_start: usize,
    freq_ranges: &[(f64, f64)],
    freq_resolution: f64,
    thresholds: &[Vec<f32>],
) {
    for (frame_idx, frame) in spectrogram.iter_mut().enumerate() {
        let thresh = &thresholds[frame_idx];
        for (bin, val) in frame.iter_mut().enumerate().skip(high_freq_start) {
            if !bin_in_range(bin, freq_resolution, freq_ranges) {
                continue;
            }
            // Skip perceptually important bins
            if bin < thresh.len() && !MaskingModel::is_masked(val.norm(), thresh[bin]) {
                continue;
            }
            *val *= 0.8;
        }
    }
}

/// An anomalous ultrasonic energy peak found by the scanner.
struct UltrasonicPeak {
    center_hz: f64,
    #[allow(dead_code)]
    ratio: f64,
}

/// Compute a single DFT bin magnitude for a windowed chunk.
fn dft_bin(samples: &[f64], k: usize) -> f64 {
    let n = samples.len();
    let mut re = 0.0;
    let mut im = 0.0;
    for (i, &s) in samples.iter().enumerate() {
        let angle = 2.0 * std::f64::consts::PI * k as f64 * i as f64 / n as f64;
        re += s * angle.cos();
        im -= s * angle.sin();
    }
    (re * re + im * im).sqrt()
}

/// Scan a channel for anomalous ultrasonic energy peaks using DFT analysis.
///
/// Fits a log-linear model to the 15-20kHz rolloff, then flags ultrasonic bins
/// (>20kHz) where measured energy exceeds predicted energy by 1.3x. This catches
/// both narrow spikes and broadband AI watermarks in the 23-24kHz range.
fn scan_ultrasonic_peaks(channel: &[f32], sr: u32) -> Vec<UltrasonicPeak> {
    let nyquist = sr as f64 / 2.0;
    if nyquist < 20000.0 {
        return vec![];
    }

    let fft_size: usize = 65536.min(channel.len());
    if fft_size < 4096 {
        return vec![];
    }

    let start = (channel.len() - fft_size) / 2;
    let chunk = &channel[start..start + fft_size];

    let windowed: Vec<f64> = chunk
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let w = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / fft_size as f64).cos());
            s as f64 * w
        })
        .collect();

    let freq_resolution = sr as f64 / fft_size as f64;
    let bin_width_hz = 500.0;

    let scan_start_hz = 15000.0;
    let mut bins: Vec<(f64, f64)> = Vec::new(); // (center_freq, avg_energy)
    let mut freq = scan_start_hz;

    while freq + bin_width_hz <= nyquist {
        let bin_lo = (freq / freq_resolution) as usize;
        let bin_hi = ((freq + bin_width_hz) / freq_resolution) as usize;
        if bin_hi >= fft_size / 2 {
            break;
        }
        let bin_count = bin_hi - bin_lo + 1;
        let energy: f64 = (bin_lo..=bin_hi)
            .map(|k| dft_bin(&windowed, k))
            .sum::<f64>()
            / bin_count as f64;
        let center = freq + bin_width_hz / 2.0;
        bins.push((center, energy));
        freq += bin_width_hz;
    }

    if bins.len() < 6 {
        return vec![];
    }

    // Fit log-linear regression on first 10 bins (15-20kHz) to model natural
    // HF rolloff: ln(energy) = slope * freq + intercept
    let fit_n = 10.min(bins.len());
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_xy = 0.0;
    let mut fit_count = 0;

    for &(f, e) in bins.iter().take(fit_n) {
        if e > 1e-15 {
            let x = f;
            let y = e.ln();
            sum_x += x;
            sum_y += y;
            sum_xx += x * x;
            sum_xy += x * y;
            fit_count += 1;
        }
    }

    if fit_count < 4 {
        return vec![];
    }

    let n = fit_count as f64;
    let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
    let intercept = (sum_y - slope * sum_x) / n;

    // Flag bins above 20kHz where energy exceeds 1.3x the predicted rolloff.
    // A ratio of 1.3x catches broadband AI watermarks (~1.5x at 21.5-23.5kHz).
    let mut peaks = Vec::new();
    for &(center, energy) in &bins {
        if center < 20000.0 || energy < 1e-15 {
            continue;
        }
        if center >= nyquist * 0.95 {
            continue;
        }
        let predicted = (slope * center + intercept).exp();
        if predicted < 1e-15 {
            continue;
        }
        let ratio = energy / predicted;
        if ratio > 1.3 {
            peaks.push(UltrasonicPeak {
                center_hz: center,
                ratio,
            });
        }
    }

    peaks
}

/// Apply surgical removal of detected ultrasonic anomalies.
///
/// Strategy: apply a low-pass filter at the lowest anomalous frequency to kill
/// the entire elevated ultrasonic region, then apply individual wide notch
/// filters at each peak center for additional suppression. This handles both
/// narrow spikes and broadband watermarks.
fn apply_adaptive_notches(
    channel: &[f32],
    sr: u32,
    peaks: &[UltrasonicPeak],
    paranoid: bool,
) -> Vec<f32> {
    if peaks.is_empty() {
        return channel.to_vec();
    }

    let nyquist = sr as f64 / 2.0;
    let mut output = channel.to_vec();

    // Find lowest anomalous frequency and place a steep low-pass below it.
    // Use cutoff 1kHz below the lowest peak to ensure the rolloff reaches the
    // anomalous band with sufficient attenuation.
    let lowest_peak = peaks.iter().map(|p| p.center_hz).fold(f64::MAX, f64::min);
    let lp_cutoff = (lowest_peak - 1000.0).max(18000.0);

    if lp_cutoff < nyquist * 0.95 {
        let lp_coeffs = biquad::butterworth_lowpass(lp_cutoff, sr);
        // Cascade multiple passes for steep rolloff (each filtfilt = 4th order)
        let passes = if paranoid { 3 } else { 2 };
        for _ in 0..passes {
            output = filtfilt::filtfilt(&output, &lp_coeffs);
        }
    }

    // Additionally, notch each peak with wide bandwidth (low Q) for extra suppression
    let q = if paranoid { 3.0 } else { 5.0 };
    for peak in peaks {
        if peak.center_hz >= nyquist * 0.95 {
            continue;
        }
        let coeffs = biquad::notch_filter(peak.center_hz, q, sr);
        output = filtfilt::filtfilt(&output, &coeffs);
    }

    output
}
