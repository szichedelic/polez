use rand::Rng;
use rand_distr::{Distribution, Normal};

use super::dsp::{stats, stft};
use crate::audio::AudioBuffer;
use crate::config::FingerprintRemovalConfig;
use crate::error::Result;

/// Human-like target ranges for statistical normalization.
const TARGET_KURTOSIS: (f64, f64) = (1.5, 4.0);

/// Fingerprint removal - normalizes AI-generated statistical patterns.
pub struct FingerprintRemover;

impl FingerprintRemover {
    /// Run fingerprint removal methods gated by config toggles.
    pub fn remove(
        buffer: &mut AudioBuffer,
        paranoid: bool,
        config: &FingerprintRemovalConfig,
    ) -> Result<()> {
        let sr = buffer.sample_rate;

        for ch in 0..buffer.num_channels() {
            let mut channel: Vec<f32> = buffer.channel(ch).to_vec();

            if config.statistical_normalization {
                normalize_statistics(&mut channel);
            }
            if config.temporal_randomization {
                temporal_randomization(&mut channel, sr, paranoid);
            }
            if config.phase_randomization {
                phase_randomization(&mut channel, paranoid);
            }
            if config.micro_timing_perturbation {
                micro_timing_perturbation(&mut channel, sr);
            }
            if config.human_imperfections {
                add_human_imperfections(&mut channel, sr);
            }

            let mut ch_view = buffer.channel_mut(ch);
            for (i, &val) in channel.iter().enumerate().take(ch_view.len()) {
                ch_view[i] = val;
            }
        }

        Ok(())
    }
}

/// Adjust statistical properties toward human-like distributions.
fn normalize_statistics(channel: &mut [f32]) {
    if channel.len() < 100 {
        return;
    }

    let mut rng = rand::thread_rng();

    // Subtle nonlinear shaping nudges kurtosis toward natural human-audio ranges,
    // disrupting the unnaturally uniform amplitude distributions common in AI audio.
    let current_kurtosis = stats::kurtosis(channel);
    if current_kurtosis < TARGET_KURTOSIS.0 || current_kurtosis > TARGET_KURTOSIS.1 {
        let _target_k = rng.gen_range(TARGET_KURTOSIS.0..TARGET_KURTOSIS.1);
        let adjustment_strength = 0.01;

        if current_kurtosis > TARGET_KURTOSIS.1 {
            // Too peaked: soft-expand tails slightly
            for sample in channel.iter_mut() {
                let s = *sample;
                *sample = s + adjustment_strength as f32 * s * s * s.signum();
            }
        } else {
            // Too flat: soft-compress toward center
            for sample in channel.iter_mut() {
                let s = *sample;
                *sample = s * (1.0 - adjustment_strength as f32 * s.abs());
            }
        }
    }
}

/// Add temporal jitter to disrupt timing-based fingerprints.
fn temporal_randomization(channel: &mut [f32], _sr: u32, paranoid: bool) {
    let jitter_std = if paranoid { 0.15 } else { 0.1 };
    let mut rng = rand::thread_rng();
    let normal = Normal::new(0.0, jitter_std).unwrap();

    let n = channel.len();
    if n < 10 {
        return;
    }

    let mut output = vec![0.0f32; n];
    for (i, out) in output.iter_mut().enumerate() {
        let offset = normal.sample(&mut rng);
        let src = (i as f64 + offset).clamp(0.0, (n - 1) as f64);
        let idx0 = src.floor() as usize;
        let idx1 = (idx0 + 1).min(n - 1);
        let frac = src - idx0 as f64;
        *out = (channel[idx0] as f64 * (1.0 - frac) + channel[idx1] as f64 * frac) as f32;
    }

    channel.copy_from_slice(&output);
}

/// Randomize phase in frequency domain to break phase-based fingerprints.
fn phase_randomization(channel: &mut [f32], paranoid: bool) {
    let noise_std = if paranoid { 0.015 } else { 0.01 };
    let mut rng = rand::thread_rng();
    let normal = Normal::new(0.0, noise_std).unwrap();

    let n = channel.len();
    let mut spectrum = stft::real_fft(channel);

    for val in &mut spectrum {
        let mag = val.norm();
        let phase = val.arg();
        let new_phase = phase + normal.sample(&mut rng) as f32;
        *val = num_complex::Complex::from_polar(mag, new_phase);
    }

    let reconstructed = stft::real_ifft(&spectrum, n);
    let copy_len = channel.len().min(reconstructed.len());
    channel[..copy_len].copy_from_slice(&reconstructed[..copy_len]);
}

/// Shift samples by ~1ms to break timing synchronization.
fn micro_timing_perturbation(channel: &mut [f32], sr: u32) {
    let shift_samples = (sr as f64 * 0.001) as usize; // ~1ms
    if shift_samples == 0 || shift_samples >= channel.len() {
        return;
    }

    let mut rng = rand::thread_rng();
    let actual_shift = rng.gen_range(0..shift_samples);

    if actual_shift > 0 {
        let n = channel.len();
        let mut shifted = vec![0.0f32; n];
        shifted[actual_shift..].copy_from_slice(&channel[..n - actual_shift]);
        for (i, val) in shifted.iter_mut().enumerate().take(actual_shift) {
            *val = channel[0] * (i as f32 / actual_shift as f32);
        }
        channel.copy_from_slice(&shifted);
    }
}

/// Add subtle human-like imperfections: velocity variation, drift, tiny distortion.
fn add_human_imperfections(channel: &mut [f32], _sr: u32) {
    let mut rng = rand::thread_rng();

    let velocity_std = 0.002;
    let velocity_normal = Normal::new(0.0, velocity_std).unwrap();

    let drift_std = 0.0001;
    let drift_normal = Normal::new(0.0, drift_std).unwrap();
    let mut drift = 0.0f64;

    let distortion_amount = 0.0001f32;

    for sample in channel.iter_mut() {
        let velocity = 1.0 + velocity_normal.sample(&mut rng) as f32;
        *sample *= velocity;

        drift += drift_normal.sample(&mut rng);
        drift *= 0.9999; // Slow decay prevents drift from becoming a DC offset
        *sample += drift as f32;

        // Soft even-harmonic distortion mimics analog saturation
        *sample += distortion_amount * (*sample) * (*sample).abs();
    }
}
