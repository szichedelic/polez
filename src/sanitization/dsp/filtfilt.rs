//! Zero-phase forward-backward digital filtering.
//!
//! Equivalent to `scipy.signal.filtfilt`, applying a biquad filter in both
//! directions to eliminate phase distortion while doubling the filter order.

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sanitization::dsp::biquad::butterworth_lowpass;
    use proptest::prelude::*;

    #[test]
    fn test_filtfilt_preserves_length() {
        let signal: Vec<f32> = (0..100).map(|i| (i as f32 * 0.1).sin()).collect();
        let coeffs = butterworth_lowpass(5000.0, 44100);
        let result = filtfilt(&signal, &coeffs);
        assert_eq!(result.len(), signal.len());
    }

    #[test]
    fn test_filtfilt_zero_phase() {
        // Filtfilt should produce zero phase shift. Verify the peak of a
        // filtered sine stays at the same position as the original.
        let sr = 44100;
        let signal: Vec<f32> = (0..4410)
            .map(|i| (2.0 * std::f32::consts::PI * 200.0 * i as f32 / sr as f32).sin())
            .collect();
        let coeffs = butterworth_lowpass(5000.0, sr);
        let result = filtfilt(&signal, &coeffs);

        // Find first peak in both
        let orig_peak = signal[100..200]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        let filt_peak = result[100..200]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert_eq!(orig_peak, filt_peak, "filtfilt should not shift phase");
    }

    #[test]
    fn test_filtfilt_short_signal() {
        let short = vec![1.0, 2.0, 3.0];
        let coeffs = butterworth_lowpass(1000.0, 44100);
        let result = filtfilt(&short, &coeffs);
        assert_eq!(result.len(), 3);
    }

    proptest! {
        #[test]
        fn prop_filtfilt_preserves_length(len in 10usize..5000) {
            let signal: Vec<f32> = (0..len)
                .map(|i| (i as f32 * 0.1).sin())
                .collect();
            let coeffs = butterworth_lowpass(5000.0, 44100);
            let result = filtfilt(&signal, &coeffs);
            prop_assert_eq!(result.len(), signal.len());
        }

        #[test]
        fn prop_filtfilt_output_bounded(amplitude in 0.01f32..1.0) {
            let signal: Vec<f32> = (0..4410)
                .map(|i| amplitude * (2.0 * std::f32::consts::PI * 200.0 * i as f32 / 44100.0).sin())
                .collect();
            let coeffs = butterworth_lowpass(5000.0, 44100);
            let result = filtfilt(&signal, &coeffs);
            let max_out = result.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            // Double-filtered LP should not exceed input amplitude (with small tolerance)
            prop_assert!(max_out <= amplitude * 1.2,
                "filtfilt output {max_out} exceeds input amplitude {amplitude}");
        }
    }
}
