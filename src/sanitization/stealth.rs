//! Advanced stealth DSP operations for audio fingerprint disruption.
//!
//! Contains 20 flag-gated operations ranging from phase noise injection and
//! resampling warps to transient micro-shifts and comb masking. Each operation
//! uses paranoid/normal parameter pairs for intensity control.

use num_complex::Complex;
use rand::Rng;
use rand_distr::{Distribution, Normal};
use std::f32::consts::PI;

use super::dsp::{biquad, filtfilt, resample, stats, stft};
use crate::audio::AudioBuffer;
use crate::config::AdvancedFlags;
use crate::error::Result;

/// Stealth operations - advanced audio modifications ported from preserving_sanitizer.py.
/// Each operation is flag-gated and uses paranoid/normal parameter pairs.
pub struct StealthOps;

impl StealthOps {
    /// Apply all enabled stealth operations in the correct order.
    pub fn apply(buffer: &mut AudioBuffer, flags: &AdvancedFlags, paranoid: bool) -> Result<()> {
        let sr = buffer.sample_rate;

        for ch in 0..buffer.num_channels() {
            let mut channel: Vec<f32> = buffer.channel(ch).to_vec();

            gentle_spectral_phase_noise(&mut channel, sr, paranoid);
            add_hf_noise_and_dither(&mut channel, sr, paranoid);
            apply_humanization(&mut channel, sr, paranoid);
            apply_micro_resample_warp(&mut channel, sr, paranoid);
            apply_analog_warmth(&mut channel, paranoid);
            apply_gentle_bandlimit(&mut channel, sr, paranoid);
            add_micro_ambience(&mut channel, paranoid);
            apply_clarity_tilt(&mut channel, sr, paranoid);

            if flags.phase_noise {
                apply_phase_noise_fft(&mut channel, paranoid);
            }
            if flags.resample_nudge {
                apply_resample_nudge(&mut channel, sr, paranoid);
            }
            if flags.gated_resample_nudge {
                apply_rms_gated_resample_nudge(&mut channel, sr, paranoid);
            }
            if flags.phase_swirl {
                apply_phase_swirl(&mut channel, paranoid);
            }
            if flags.masked_hf_phase {
                apply_masked_hf_phase_noise(&mut channel, sr, paranoid);
            }
            if flags.hf_decorrelate {
                apply_hf_decorrelate(&mut channel, sr, paranoid);
            }
            if flags.phase_dither {
                apply_subblock_phase_dither(&mut channel, paranoid);
            }
            if flags.comb_mask {
                apply_dynamic_comb_mask(&mut channel, sr, paranoid);
            }
            if flags.transient_shift {
                apply_transient_micro_shift(&mut channel, sr, paranoid);
            }
            if flags.micro_eq_flutter {
                apply_micro_eq_modulation(&mut channel, sr, paranoid);
            }
            if flags.refined_transient {
                apply_refined_transient_shift(&mut channel, sr, paranoid);
            }
            if flags.adaptive_transient {
                apply_adaptive_transient_shift(&mut channel, sr, paranoid);
            }

            let mut ch_view = buffer.channel_mut(ch);
            for (i, &val) in channel.iter().enumerate().take(ch_view.len()) {
                ch_view[i] = val;
            }
        }

        Ok(())
    }
}

/// Op 1: Gentle spectral phase noise.
/// Adds small random phase perturbations in the STFT domain below a frequency cutoff.
fn gentle_spectral_phase_noise(channel: &mut [f32], sr: u32, paranoid: bool) {
    let phase_noise_rad: f32 = if paranoid { 0.12 } else { 0.08 };
    let cutoff_hz: f64 = if paranoid { 8000.0 } else { 10000.0 };

    let nperseg = 2048.min(channel.len() / 4).max(256);
    let noverlap = nperseg * 3 / 4;
    let (mut spectrogram, orig_len) = stft::stft(channel, nperseg, noverlap);
    if spectrogram.is_empty() {
        return;
    }

    let freq_resolution = sr as f64 / nperseg as f64;
    let cutoff_bin = (cutoff_hz / freq_resolution) as usize;
    let mut rng = rand::thread_rng();

    for frame in &mut spectrogram {
        for bin in 0..cutoff_bin.min(frame.len()) {
            let mag = frame[bin].norm();
            let phase = frame[bin].arg();
            let noise = rng.gen_range(-phase_noise_rad..phase_noise_rad);
            frame[bin] = Complex::from_polar(mag, phase + noise);
        }
    }

    let reconstructed = stft::istft(&spectrogram, nperseg, noverlap, orig_len);
    let copy_len = channel.len().min(reconstructed.len());
    channel[..copy_len].copy_from_slice(&reconstructed[..copy_len]);
}

/// Op 2: Add high-frequency noise and dither.
fn add_hf_noise_and_dither(channel: &mut [f32], sr: u32, paranoid: bool) {
    let noise_level: f64 = if paranoid { 1.8e-7 } else { 9e-8 };
    let dither_level: f64 = if paranoid { 4e-6 } else { 2e-6 };
    let mut rng = rand::thread_rng();

    let hf_noise: Vec<f32> = (0..channel.len())
        .map(|_| rng.gen_range(-noise_level..noise_level) as f32)
        .collect();

    let coeffs = biquad::butterworth_highpass(10000.0, sr);
    let filtered_noise = biquad::biquad_process(&hf_noise, &coeffs);

    // TPDF dither: sum of two uniform distributions approximates triangular PDF
    for (i, sample) in channel.iter_mut().enumerate() {
        let dither = (rng.gen_range(-dither_level..dither_level)
            + rng.gen_range(-dither_level..dither_level)) as f32;
        *sample += filtered_noise.get(i).unwrap_or(&0.0) + dither;
    }
}

/// Op 3: Apply humanization (wow/flutter simulation).
fn apply_humanization(channel: &mut [f32], sr: u32, paranoid: bool) {
    let wow_freq: f64 = if paranoid { 0.21 } else { 0.15 };
    let depth_samples: f64 = if paranoid { 8.0 } else { 5.0 };
    let gain_variation: f64 = if paranoid { 0.012 } else { 0.008 };

    let n = channel.len();
    let mut output = channel.to_vec();

    for (i, out) in output.iter_mut().enumerate() {
        let t = i as f64 / sr as f64;

        let wow = (2.0 * std::f64::consts::PI * wow_freq * t).sin() * depth_samples;
        let src_idx = i as f64 + wow;
        let src_clamped = src_idx.clamp(0.0, (n - 1) as f64);
        let idx0 = src_clamped.floor() as usize;
        let idx1 = (idx0 + 1).min(n - 1);
        let frac = src_clamped - idx0 as f64;

        let interpolated = channel[idx0] as f64 * (1.0 - frac) + channel[idx1] as f64 * frac;
        let gain = 1.0 + gain_variation * (2.0 * std::f64::consts::PI * 0.5 * t).sin();
        *out = (interpolated * gain) as f32;
    }

    channel.copy_from_slice(&output);
}

/// Op 4: Micro resample warp.
fn apply_micro_resample_warp(channel: &mut [f32], sr: u32, paranoid: bool) {
    let warp_percent: f64 = if paranoid { 0.22 } else { 0.15 };
    let warp_ratio = 1.0 + warp_percent / 100.0;
    let intermediate_sr = (sr as f64 * warp_ratio) as u32;

    let warped = resample::resample(channel, sr, intermediate_sr);
    let restored = resample::resample(&warped, intermediate_sr, sr);

    let copy_len = channel.len().min(restored.len());
    channel[..copy_len].copy_from_slice(&restored[..copy_len]);
}

/// Op 5: Analog warmth (subtle soft saturation).
fn apply_analog_warmth(channel: &mut [f32], paranoid: bool) {
    let drive: f32 = if paranoid { 1.07 } else { 1.04 };

    for sample in channel.iter_mut() {
        *sample = (*sample * drive).tanh();
    }
}

/// Op 6: Gentle band-limiting.
fn apply_gentle_bandlimit(channel: &mut [f32], sr: u32, paranoid: bool) {
    let cutoff: f64 = if paranoid { 19000.0 } else { 20000.0 };
    if cutoff >= sr as f64 / 2.0 * 0.95 {
        return;
    }

    let coeffs = biquad::butterworth_lowpass(cutoff, sr);
    let filtered = filtfilt::filtfilt(channel, &coeffs);
    channel.copy_from_slice(&filtered);
}

/// Op 7: Add micro ambient room tone.
fn add_micro_ambience(channel: &mut [f32], paranoid: bool) {
    let level: f64 = if paranoid { 3e-7 } else { 1.5e-7 };
    let mut rng = rand::thread_rng();
    let normal = Normal::new(0.0, level).unwrap();

    for sample in channel.iter_mut() {
        *sample += normal.sample(&mut rng) as f32;
    }
}

/// Op 8: Clarity tilt (subtle high-shelf EQ).
fn apply_clarity_tilt(channel: &mut [f32], sr: u32, paranoid: bool) {
    let gain_db: f64 = if paranoid { -0.3 } else { -0.15 };
    let freq: f64 = 8000.0;

    let coeffs = biquad::high_shelf(freq, gain_db, sr);
    let filtered = filtfilt::filtfilt(channel, &coeffs);
    channel.copy_from_slice(&filtered);
}

/// Op 9: FFT-domain phase noise injection.
fn apply_phase_noise_fft(channel: &mut [f32], paranoid: bool) {
    let noise_rad: f64 = if paranoid { 0.08 } else { 0.05 };
    let mut rng = rand::thread_rng();
    let normal = Normal::new(0.0, noise_rad).unwrap();

    let n = channel.len();
    let mut spectrum = stft::real_fft(channel);

    for val in &mut spectrum {
        let mag = val.norm();
        let phase = val.arg();
        let noise = normal.sample(&mut rng) as f32;
        *val = Complex::from_polar(mag, phase + noise);
    }

    let reconstructed = stft::real_ifft(&spectrum, n);
    let copy_len = channel.len().min(reconstructed.len());
    channel[..copy_len].copy_from_slice(&reconstructed[..copy_len]);
}

/// Op 10: Resample nudge - tiny sample rate shift.
fn apply_resample_nudge(channel: &mut [f32], sr: u32, paranoid: bool) {
    let nudge_percent: f64 = if paranoid { 0.06 } else { 0.035 };
    let nudge_ratio = 1.0 + nudge_percent / 100.0;
    let nudge_sr = (sr as f64 * nudge_ratio) as u32;

    let nudged = resample::resample(channel, sr, nudge_sr);
    let restored = resample::resample(&nudged, nudge_sr, sr);

    let copy_len = channel.len().min(restored.len());
    channel[..copy_len].copy_from_slice(&restored[..copy_len]);
}

/// Op 11: RMS-gated resample nudge (only applies to loud sections).
fn apply_rms_gated_resample_nudge(channel: &mut [f32], sr: u32, paranoid: bool) {
    let frame_size = 1024;
    let threshold_rms: f64 = 0.01;
    let nudge_percent: f64 = if paranoid { 0.04 } else { 0.025 };

    let n = channel.len();
    let mut output = channel.to_vec();

    let mut pos = 0;
    while pos + frame_size <= n {
        let frame = &channel[pos..pos + frame_size];
        let rms = stats::rms_energy(frame);

        if rms > threshold_rms {
            let nudge_ratio = 1.0 + nudge_percent / 100.0;
            let nudge_sr = (sr as f64 * nudge_ratio) as u32;
            let nudged = resample::resample(frame, sr, nudge_sr);
            let restored = resample::resample(&nudged, nudge_sr, sr);
            let copy_len = frame_size.min(restored.len());
            output[pos..pos + copy_len].copy_from_slice(&restored[..copy_len]);
        }

        pos += frame_size;
    }

    channel.copy_from_slice(&output);
}

/// Op 12: Phase swirl - allpass cascade for subtle phase rotation.
fn apply_phase_swirl(channel: &mut [f32], paranoid: bool) {
    let alphas: &[f64] = if paranoid {
        &[0.016, -0.014]
    } else {
        &[0.012, -0.01]
    };

    let frequencies = [2000.0, 5000.0];
    for (i, &alpha) in alphas.iter().enumerate() {
        if i >= frequencies.len() {
            break;
        }
        let freq = frequencies[i];
        let q = 0.7 + alpha.abs() * 10.0;
        let coeffs = biquad::allpass_filter(freq, q, 44100);
        let processed = biquad::biquad_process(channel, &coeffs);
        let blend = alpha.abs() as f32;
        for (j, sample) in channel.iter_mut().enumerate() {
            if let Some(&p) = processed.get(j) {
                *sample = *sample * (1.0 - blend) + p * blend;
            }
        }
    }
}

/// Op 13: Masked HF phase noise (only above a cutoff frequency).
fn apply_masked_hf_phase_noise(channel: &mut [f32], sr: u32, paranoid: bool) {
    let start_hz: f64 = if paranoid { 14500.0 } else { 15500.0 };
    let noise_rad: f32 = if paranoid { 0.15 } else { 0.10 };

    let nperseg = 2048.min(channel.len() / 4).max(256);
    let noverlap = nperseg * 3 / 4;
    let (mut spectrogram, orig_len) = stft::stft(channel, nperseg, noverlap);
    if spectrogram.is_empty() {
        return;
    }

    let freq_resolution = sr as f64 / nperseg as f64;
    let start_bin = (start_hz / freq_resolution) as usize;
    let mut rng = rand::thread_rng();

    for frame in &mut spectrogram {
        for val in frame.iter_mut().skip(start_bin) {
            let mag = val.norm();
            let phase = val.arg();
            let noise = rng.gen_range(-noise_rad..noise_rad);
            *val = Complex::from_polar(mag, phase + noise);
        }
    }

    let reconstructed = stft::istft(&spectrogram, nperseg, noverlap, orig_len);
    let copy_len = channel.len().min(reconstructed.len());
    channel[..copy_len].copy_from_slice(&reconstructed[..copy_len]);
}

/// Op 14: HF decorrelation between bands.
fn apply_hf_decorrelate(channel: &mut [f32], sr: u32, paranoid: bool) {
    let band_low: f64 = if paranoid { 12000.0 } else { 13000.0 };
    let band_high: f64 = if paranoid { 16000.0 } else { 17000.0 };

    let nperseg = 2048.min(channel.len() / 4).max(256);
    let noverlap = nperseg * 3 / 4;
    let (mut spectrogram, orig_len) = stft::stft(channel, nperseg, noverlap);
    if spectrogram.is_empty() {
        return;
    }

    let freq_resolution = sr as f64 / nperseg as f64;
    let low_bin = (band_low / freq_resolution) as usize;
    let high_bin = (band_high / freq_resolution) as usize;
    let mut rng = rand::thread_rng();

    for frame in &mut spectrogram {
        for bin in low_bin..high_bin.min(frame.len()) {
            let mag = frame[bin].norm();
            let new_phase = rng.gen_range(-PI..PI);
            frame[bin] = Complex::from_polar(mag, new_phase);
        }
    }

    let reconstructed = stft::istft(&spectrogram, nperseg, noverlap, orig_len);
    let copy_len = channel.len().min(reconstructed.len());
    channel[..copy_len].copy_from_slice(&reconstructed[..copy_len]);
}

/// Op 15: Sub-block phase dither.
fn apply_subblock_phase_dither(channel: &mut [f32], paranoid: bool) {
    let block_size = 512;
    let dither_rad: f32 = if paranoid { 0.04 } else { 0.02 };
    let mut rng = rand::thread_rng();

    let n = channel.len();
    let mut pos = 0;

    while pos + block_size <= n {
        let block = &channel[pos..pos + block_size];
        let mut spectrum = stft::real_fft(block);

        for val in &mut spectrum {
            let mag = val.norm();
            let phase = val.arg();
            let noise = rng.gen_range(-dither_rad..dither_rad);
            *val = Complex::from_polar(mag, phase + noise);
        }

        let reconstructed = stft::real_ifft(&spectrum, block_size);
        let copy_len = block_size.min(reconstructed.len());
        channel[pos..pos + copy_len].copy_from_slice(&reconstructed[..copy_len]);

        pos += block_size;
    }
}

/// Op 16: Dynamic comb masking above a base frequency.
fn apply_dynamic_comb_mask(channel: &mut [f32], sr: u32, paranoid: bool) {
    let base_hz: f64 = if paranoid { 15000.0 } else { 17000.0 };
    let notch_q: f64 = 10.0;
    let mut rng = rand::thread_rng();

    // Apply several narrow notches at random harmonics above base_hz
    let num_notches = if paranoid { 5 } else { 3 };
    for _ in 0..num_notches {
        let freq = base_hz + rng.gen_range(0.0..3000.0);
        if freq >= sr as f64 / 2.0 * 0.95 {
            continue;
        }
        let coeffs = biquad::notch_filter(freq, notch_q, sr);
        let filtered = biquad::biquad_process(channel, &coeffs);
        channel.copy_from_slice(&filtered);
    }
}

/// Op 17: Transient micro-shift.
/// Detects transients and applies sub-sample shifts to disrupt timing fingerprints.
fn apply_transient_micro_shift(channel: &mut [f32], sr: u32, paranoid: bool) {
    let shift_ms: f64 = if paranoid { 0.1 } else { 0.08 };
    let shift_samples = (shift_ms * sr as f64 / 1000.0).round() as usize;
    if shift_samples == 0 || channel.len() < shift_samples * 4 {
        return;
    }

    let onsets = detect_onsets(channel, sr);
    let mut rng = rand::thread_rng();

    for onset in &onsets {
        let shift = rng.gen_range(0..shift_samples.max(1));
        let direction: i32 = if rng.gen_bool(0.5) { 1 } else { -1 };
        let actual_shift = (direction * shift as i32).max(0) as usize;

        // Shift a small region around the onset
        let region_start = onset.saturating_sub(shift_samples * 2);
        let region_end = (*onset + shift_samples * 4).min(channel.len());

        if region_end <= region_start + actual_shift + 1 {
            continue;
        }

        let region: Vec<f32> = channel[region_start..region_end].to_vec();
        let region_len = region.len();
        for i in actual_shift..region_len {
            let blend = (i - actual_shift) as f32 / region_len as f32;
            channel[region_start + i] = region[i] * (1.0 - blend * 0.01)
                + region[i.saturating_sub(actual_shift)] * blend * 0.01;
        }
    }
}

/// Op 18: Micro EQ modulation (subtle time-varying EQ).
fn apply_micro_eq_modulation(channel: &mut [f32], sr: u32, paranoid: bool) {
    let mod_depth_db: f64 = if paranoid { 0.015 } else { 0.01 };
    let mod_freq: f64 = 0.3; // Hz

    let n = channel.len();
    let block_size = 4096.min(n);
    let mut pos = 0;

    while pos + block_size <= n {
        let t = pos as f64 / sr as f64;
        let gain_db = mod_depth_db * (2.0 * std::f64::consts::PI * mod_freq * t).sin();

        let coeffs = biquad::peaking_eq(3000.0, gain_db, 1.0, sr);
        let block = &channel[pos..pos + block_size];
        let processed = biquad::biquad_process(block, &coeffs);
        channel[pos..pos + block_size].copy_from_slice(&processed);

        pos += block_size;
    }
}

/// Op 19: Refined transient shift with onset-detection gating.
fn apply_refined_transient_shift(channel: &mut [f32], sr: u32, paranoid: bool) {
    let shift_ms: f64 = if paranoid { 0.12 } else { 0.08 };
    let shift_samples = (shift_ms * sr as f64 / 1000.0).round() as usize;
    if shift_samples == 0 {
        return;
    }

    let onsets = detect_onsets(channel, sr);
    let mut rng = rand::thread_rng();
    let normal = Normal::new(0.0, shift_samples as f64 / 3.0).unwrap();

    for onset in &onsets {
        let shift = (normal.sample(&mut rng).abs() as usize).min(shift_samples);
        if *onset + shift >= channel.len() {
            continue;
        }

        // Subtle cross-fade around onset
        let fade_len = shift * 2;
        let start = onset.saturating_sub(fade_len);
        let end = (*onset + fade_len).min(channel.len());

        for i in start..end {
            let progress = (i - start) as f32 / (end - start) as f32;
            let offset = (shift as f32 * (1.0 - progress)).round() as usize;
            if i + offset < channel.len() {
                channel[i] = channel[i] * 0.99 + channel[i + offset] * 0.01;
            }
        }
    }
}

/// Op 20: Adaptive transient shift (onset-strength-gated).
fn apply_adaptive_transient_shift(channel: &mut [f32], sr: u32, paranoid: bool) {
    let max_shift_ms: f64 = if paranoid { 0.15 } else { 0.10 };
    let max_shift = (max_shift_ms * sr as f64 / 1000.0).round() as usize;
    if max_shift == 0 {
        return;
    }

    let frame_size = (sr as usize / 100).max(64);
    let hop = frame_size / 2;
    let mut energies: Vec<f64> = Vec::new();

    let mut pos = 0;
    while pos + frame_size <= channel.len() {
        energies.push(stats::rms_energy(&channel[pos..pos + frame_size]));
        pos += hop;
    }

    if energies.len() < 3 {
        return;
    }

    let strengths: Vec<f64> = energies
        .windows(2)
        .map(|w| (w[1] - w[0]).max(0.0))
        .collect();
    let max_strength = strengths.iter().cloned().fold(0.0_f64, f64::max);
    if max_strength < 1e-10 {
        return;
    }

    for (i, &strength) in strengths.iter().enumerate() {
        let normalized = strength / max_strength;
        if normalized < 0.3 {
            continue;
        }

        let shift = (max_shift as f64 * normalized).round() as usize;
        let sample_pos = i * hop;

        if sample_pos + shift + hop < channel.len() {
            for j in 0..hop.min(channel.len() - sample_pos - shift) {
                let blend = normalized as f32 * 0.01;
                channel[sample_pos + j] = channel[sample_pos + j] * (1.0 - blend)
                    + channel[sample_pos + j + shift] * blend;
            }
        }
    }
}

/// Simple onset detection: find sample indices where energy jumps.
fn detect_onsets(channel: &[f32], sr: u32) -> Vec<usize> {
    let frame_size = (sr as usize / 100).max(64);
    let hop = frame_size / 2;
    let mut energies: Vec<f64> = Vec::new();

    let mut pos = 0;
    while pos + frame_size <= channel.len() {
        energies.push(stats::rms_energy(&channel[pos..pos + frame_size]));
        pos += hop;
    }

    if energies.len() < 3 {
        return vec![];
    }

    let flux: Vec<f64> = energies
        .windows(2)
        .map(|w| (w[1] - w[0]).max(0.0))
        .collect();
    let max_flux = flux.iter().cloned().fold(0.0_f64, f64::max);
    let threshold = max_flux * 0.3;

    let peaks = stats::find_peaks(&flux, threshold, (sr as usize / hop / 10).max(1));
    peaks.iter().map(|p| p.index * hop).collect()
}
