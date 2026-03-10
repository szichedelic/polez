use std::f64::consts::PI;

/// Biquad filter coefficients in Direct Form I: H(z) = (b0 + b1*z^-1 + b2*z^-2) / (1 + a1*z^-1 + a2*z^-2)
#[derive(Debug, Clone, Copy)]
pub struct BiquadCoefficients {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

/// Design a 2nd-order Butterworth low-pass filter.
pub fn butterworth_lowpass(cutoff_hz: f64, sample_rate: u32) -> BiquadCoefficients {
    let w0 = 2.0 * PI * cutoff_hz / sample_rate as f64;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0_f64).sqrt(); // Q = 1/sqrt(2) for Butterworth

    let b0 = (1.0 - cos_w0) / 2.0;
    let b1 = 1.0 - cos_w0;
    let b2 = (1.0 - cos_w0) / 2.0;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha;

    BiquadCoefficients {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// Design a 2nd-order Butterworth high-pass filter.
pub fn butterworth_highpass(cutoff_hz: f64, sample_rate: u32) -> BiquadCoefficients {
    let w0 = 2.0 * PI * cutoff_hz / sample_rate as f64;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0_f64).sqrt();

    let b0 = (1.0 + cos_w0) / 2.0;
    let b1 = -(1.0 + cos_w0);
    let b2 = (1.0 + cos_w0) / 2.0;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha;

    BiquadCoefficients {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// Design a notch (band-reject) filter.
pub fn notch_filter(freq_hz: f64, q: f64, sample_rate: u32) -> BiquadCoefficients {
    let w0 = 2.0 * PI * freq_hz / sample_rate as f64;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    let b0 = 1.0;
    let b1 = -2.0 * cos_w0;
    let b2 = 1.0;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha;

    BiquadCoefficients {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// Design a first-order allpass filter.
pub fn allpass_filter(freq_hz: f64, q: f64, sample_rate: u32) -> BiquadCoefficients {
    let w0 = 2.0 * PI * freq_hz / sample_rate as f64;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    let b0 = 1.0 - alpha;
    let b1 = -2.0 * cos_w0;
    let b2 = 1.0 + alpha;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha;

    BiquadCoefficients {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// Design a high-shelf filter (RBJ cookbook).
pub fn high_shelf(freq_hz: f64, gain_db: f64, sample_rate: u32) -> BiquadCoefficients {
    let a_lin = 10.0_f64.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq_hz / sample_rate as f64;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / 2.0
        * ((a_lin + 1.0 / a_lin) * (1.0 / std::f64::consts::FRAC_1_SQRT_2 - 1.0) + 2.0).sqrt();
    let two_sqrt_a_alpha = 2.0 * a_lin.sqrt() * alpha;

    let b0 = a_lin * ((a_lin + 1.0) + (a_lin - 1.0) * cos_w0 + two_sqrt_a_alpha);
    let b1 = -2.0 * a_lin * ((a_lin - 1.0) + (a_lin + 1.0) * cos_w0);
    let b2 = a_lin * ((a_lin + 1.0) + (a_lin - 1.0) * cos_w0 - two_sqrt_a_alpha);
    let a0 = (a_lin + 1.0) - (a_lin - 1.0) * cos_w0 + two_sqrt_a_alpha;
    let a1 = 2.0 * ((a_lin - 1.0) - (a_lin + 1.0) * cos_w0);
    let a2 = (a_lin + 1.0) - (a_lin - 1.0) * cos_w0 - two_sqrt_a_alpha;

    BiquadCoefficients {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// Design a peaking EQ filter.
pub fn peaking_eq(freq_hz: f64, gain_db: f64, q: f64, sample_rate: u32) -> BiquadCoefficients {
    let a_lin = 10.0_f64.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq_hz / sample_rate as f64;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    let b0 = 1.0 + alpha * a_lin;
    let b1 = -2.0 * cos_w0;
    let b2 = 1.0 - alpha * a_lin;
    let a0 = 1.0 + alpha / a_lin;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha / a_lin;

    BiquadCoefficients {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// Design a bandpass filter.
pub fn bandpass(freq_hz: f64, q: f64, sample_rate: u32) -> BiquadCoefficients {
    let w0 = 2.0 * PI * freq_hz / sample_rate as f64;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    let b0 = alpha;
    let b1 = 0.0;
    let b2 = -alpha;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha;

    BiquadCoefficients {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// Apply a biquad filter to a signal (Direct Form II Transposed).
///
/// The inner loop uses local coefficient copies to help the compiler keep
/// them in registers and avoid repeated memory loads.
#[inline]
pub fn biquad_process(signal: &[f32], coeffs: &BiquadCoefficients) -> Vec<f32> {
    let n = signal.len();
    let mut output = vec![0.0f32; n];

    // Local copies help the optimizer keep these in registers
    let b0 = coeffs.b0;
    let b1 = coeffs.b1;
    let b2 = coeffs.b2;
    let a1 = coeffs.a1;
    let a2 = coeffs.a2;
    let mut z1 = 0.0_f64;
    let mut z2 = 0.0_f64;

    for (out, &inp) in output.iter_mut().zip(signal.iter()) {
        let x = inp as f64;
        let y = b0 * x + z1;
        z1 = b1 * x - a1 * y + z2;
        z2 = b2 * x - a2 * y;
        *out = y as f32;
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn sine_signal(freq: f32, sr: u32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin())
            .collect()
    }

    fn signal_rms(signal: &[f32]) -> f64 {
        let sum_sq: f64 = signal.iter().map(|&s| (s as f64).powi(2)).sum();
        (sum_sq / signal.len() as f64).sqrt()
    }

    #[test]
    fn test_lowpass_attenuates_high_freq() {
        let sr = 44100;
        let low = sine_signal(200.0, sr, 4410);
        let high = sine_signal(10000.0, sr, 4410);
        let mixed: Vec<f32> = low.iter().zip(&high).map(|(a, b)| a + b).collect();

        let coeffs = butterworth_lowpass(1000.0, sr);
        let filtered = biquad_process(&mixed, &coeffs);

        // After LP at 1kHz, high freq should be attenuated significantly
        let high_only = biquad_process(&high, &coeffs);
        assert!(signal_rms(&high_only) < signal_rms(&high) * 0.1);

        // Low freq should pass through mostly unchanged
        let low_only = biquad_process(&low, &coeffs);
        assert!(signal_rms(&low_only) > signal_rms(&low) * 0.8);

        // Output should exist
        assert_eq!(filtered.len(), mixed.len());
    }

    #[test]
    fn test_highpass_attenuates_low_freq() {
        let sr = 44100;
        let low = sine_signal(100.0, sr, 4410);
        let coeffs = butterworth_highpass(5000.0, sr);
        let filtered = biquad_process(&low, &coeffs);
        assert!(signal_rms(&filtered) < signal_rms(&low) * 0.1);
    }

    #[test]
    fn test_notch_removes_target_freq() {
        let sr = 44100;
        let target = sine_signal(1000.0, sr, 4410);
        let coeffs = notch_filter(1000.0, 10.0, sr);
        let filtered = biquad_process(&target, &coeffs);
        assert!(signal_rms(&filtered) < signal_rms(&target) * 0.15);
    }

    #[test]
    fn test_coefficients_finite() {
        let coeffs = butterworth_lowpass(5000.0, 44100);
        assert!(coeffs.b0.is_finite());
        assert!(coeffs.b1.is_finite());
        assert!(coeffs.b2.is_finite());
        assert!(coeffs.a1.is_finite());
        assert!(coeffs.a2.is_finite());
    }

    #[test]
    fn test_empty_signal() {
        let coeffs = butterworth_lowpass(1000.0, 44100);
        assert!(biquad_process(&[], &coeffs).is_empty());
    }

    proptest! {
        #[test]
        fn prop_biquad_output_bounded(amplitude in 0.01f32..1.0, freq in 100.0f32..10000.0) {
            let sr = 44100u32;
            let signal: Vec<f32> = (0..4410)
                .map(|i| amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin())
                .collect();
            let coeffs = butterworth_lowpass(5000.0, sr);
            let filtered = biquad_process(&signal, &coeffs);
            // A stable filter should not amplify beyond input amplitude (for lowpass)
            let max_out = filtered.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            // Allow small overshoot from transient response (1.5x)
            prop_assert!(max_out <= amplitude * 1.5,
                "Output {max_out} exceeds 1.5x input amplitude {amplitude}");
        }

        #[test]
        fn prop_biquad_preserves_length(len in 10usize..5000) {
            let signal: Vec<f32> = (0..len)
                .map(|i| (i as f32 * 0.1).sin())
                .collect();
            let coeffs = butterworth_lowpass(5000.0, 44100);
            let filtered = biquad_process(&signal, &coeffs);
            prop_assert_eq!(filtered.len(), signal.len());
        }

        #[test]
        fn prop_biquad_coefficients_finite(cutoff in 100.0f64..20000.0) {
            let coeffs = butterworth_lowpass(cutoff, 44100);
            prop_assert!(coeffs.b0.is_finite());
            prop_assert!(coeffs.b1.is_finite());
            prop_assert!(coeffs.b2.is_finite());
            prop_assert!(coeffs.a1.is_finite());
            prop_assert!(coeffs.a2.is_finite());
        }

        #[test]
        fn prop_allpass_preserves_magnitude(freq in 200.0f32..5000.0) {
            let sr = 44100u32;
            let signal: Vec<f32> = (0..4410)
                .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin())
                .collect();
            let coeffs = allpass_filter(1000.0, 0.707, sr);
            let filtered = biquad_process(&signal, &coeffs);
            let in_rms = signal_rms(&signal);
            let out_rms = signal_rms(&filtered);
            // Allpass should preserve energy (within 10% tolerance for edge effects)
            let ratio = out_rms / in_rms;
            prop_assert!(ratio > 0.9 && ratio < 1.1,
                "Allpass RMS ratio {ratio} out of range for freq {freq}");
        }
    }
}
