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
pub fn biquad_process(signal: &[f32], coeffs: &BiquadCoefficients) -> Vec<f32> {
    let n = signal.len();
    let mut output = vec![0.0f32; n];
    let mut z1 = 0.0_f64;
    let mut z2 = 0.0_f64;

    for i in 0..n {
        let x = signal[i] as f64;
        let y = coeffs.b0 * x + z1;
        z1 = coeffs.b1 * x - coeffs.a1 * y + z2;
        z2 = coeffs.b2 * x - coeffs.a2 * y;
        output[i] = y as f32;
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
