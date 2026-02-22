use super::biquad::{biquad_process, BiquadCoefficients};

/// Zero-phase forward-backward filtering (equivalent to scipy.signal.filtfilt).
/// Applies the filter forward, reverses, applies again, reverses back.
/// This eliminates phase distortion.
pub fn filtfilt(signal: &[f32], coeffs: &BiquadCoefficients) -> Vec<f32> {
    if signal.len() < 6 {
        return signal.to_vec();
    }

    // Pad signal to reduce edge effects (3x filter order = 6 samples)
    let pad_len = 6.min(signal.len() - 1);
    let mut padded = Vec::with_capacity(signal.len() + 2 * pad_len);

    // Reflect-pad start
    for i in (1..=pad_len).rev() {
        padded.push(2.0 * signal[0] - signal[i]);
    }
    padded.extend_from_slice(signal);
    // Reflect-pad end
    let last = signal.len() - 1;
    for i in 1..=pad_len {
        padded.push(2.0 * signal[last] - signal[last - i]);
    }

    // Forward pass
    let forward = biquad_process(&padded, coeffs);

    // Reverse
    let mut reversed: Vec<f32> = forward.into_iter().rev().collect();

    // Backward pass
    reversed = biquad_process(&reversed, coeffs);

    // Reverse back and remove padding
    reversed.reverse();
    reversed[pad_len..pad_len + signal.len()].to_vec()
}
