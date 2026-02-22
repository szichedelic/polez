use num_complex::Complex;
use rustfft::FftPlanner;

/// Compute the Hilbert transform of a real signal, returning the analytic signal.
/// analytic[i] = signal[i] + j * hilbert(signal[i])
pub fn hilbert(signal: &[f32]) -> Vec<Complex<f32>> {
    let n = signal.len();
    if n == 0 {
        return vec![];
    }

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);
    let ifft = planner.plan_fft_inverse(n);

    // Forward FFT
    let mut buf: Vec<Complex<f32>> = signal.iter().map(|&s| Complex::new(s, 0.0)).collect();
    fft.process(&mut buf);

    // Apply Hilbert multiplier:
    // h[0] = 1, h[n/2] = 1 (if even), h[1..n/2] = 2, h[n/2+1..] = 0
    // DC component unchanged
    if n > 1 {
        for val in buf.iter_mut().take(n / 2).skip(1) {
            *val *= 2.0;
        }
        // Nyquist unchanged if even
        for val in buf.iter_mut().skip(n / 2 + 1) {
            *val = Complex::new(0.0, 0.0);
        }
    }

    // Inverse FFT
    ifft.process(&mut buf);

    // Normalize
    let scale = 1.0 / n as f32;
    for val in &mut buf {
        *val *= scale;
    }

    buf
}

/// Extract the amplitude envelope of a signal using the Hilbert transform.
pub fn envelope(signal: &[f32]) -> Vec<f32> {
    hilbert(signal).iter().map(|c| c.norm()).collect()
}
