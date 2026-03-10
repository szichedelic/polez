use num_complex::Complex;
use rustfft::FftPlanner;
use std::f32::consts::PI;

/// Compute the Short-Time Fourier Transform.
///
/// The signal is zero-padded by `nperseg/2` samples on each side before
/// framing. This ensures every output sample of the corresponding `istft` has
/// full overlap coverage from multiple frames, preventing edge amplification
/// when spectral bins are modified. The matching `istft` call strips the same
/// padding automatically via the `original_len` return value.
///
/// Returns (spectrogram, original_len):
///   - spectrogram: Vec of frequency frames, each with nperseg/2 + 1 complex values
///   - original_len: unpadded signal length, required by `istft` to strip padding
pub fn stft(signal: &[f32], nperseg: usize, noverlap: usize) -> (Vec<Vec<Complex<f32>>>, usize) {
    let hop = nperseg - noverlap;
    if signal.len() < nperseg || hop == 0 {
        return (vec![], signal.len());
    }

    let original_len = signal.len();
    let pad = nperseg / 2;

    // Zero-pad on both ends so that every sample is covered by a fully-overlapping
    // set of frames. Without this, the first and last `pad` samples of the ISTFT
    // output have window_sum values near zero, causing division blow-up when bins
    // are modified.
    let mut padded = vec![0.0f32; original_len + 2 * pad];
    padded[pad..pad + original_len].copy_from_slice(signal);

    let window = hann_window(nperseg);
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(nperseg);
    let n_freqs = nperseg / 2 + 1;

    let num_frames = (padded.len() - nperseg) / hop + 1;
    let mut spectrogram = Vec::with_capacity(num_frames);

    for frame_idx in 0..num_frames {
        let start = frame_idx * hop;
        let mut buf: Vec<Complex<f32>> = (0..nperseg)
            .map(|i| Complex::new(padded[start + i] * window[i], 0.0))
            .collect();

        fft.process(&mut buf);
        spectrogram.push(buf[..n_freqs].to_vec());
    }

    (spectrogram, original_len)
}

/// Compute the Inverse Short-Time Fourier Transform using overlap-add.
///
/// `original_len` must be the unpadded signal length returned by `stft`.
/// The function reconstructs the full padded output and strips the
/// `nperseg/2` zero-pad regions from both ends, returning exactly
/// `original_len` samples.
pub fn istft(
    spectrogram: &[Vec<Complex<f32>>],
    nperseg: usize,
    noverlap: usize,
    original_len: usize,
) -> Vec<f32> {
    if spectrogram.is_empty() {
        return vec![];
    }

    let hop = nperseg - noverlap;
    let num_frames = spectrogram.len();
    let output_len = (num_frames - 1) * hop + nperseg;

    let window = hann_window(nperseg);
    let mut planner = FftPlanner::new();
    let ifft = planner.plan_fft_inverse(nperseg);

    let mut output = vec![0.0f32; output_len];
    let mut window_sum = vec![0.0f32; output_len];

    for (frame_idx, frame) in spectrogram.iter().enumerate() {
        // Reconstruct full spectrum from positive frequencies (conjugate symmetry)
        let mut buf = vec![Complex::new(0.0f32, 0.0); nperseg];
        let n_freqs = frame.len();

        for (i, &val) in frame.iter().enumerate() {
            buf[i] = val;
        }
        // Mirror conjugate for negative frequencies
        for i in 1..nperseg - n_freqs + 1 {
            buf[nperseg - i] = frame[i].conj();
        }

        ifft.process(&mut buf);

        // Normalize IFFT output (rustfft doesn't normalize)
        let scale = 1.0 / nperseg as f32;
        let start = frame_idx * hop;

        for i in 0..nperseg {
            output[start + i] += buf[i].re * scale * window[i];
            window_sum[start + i] += window[i] * window[i];
        }
    }

    // Normalize by window overlap sum. With zero-padding applied in `stft`,
    // every sample in the [pad..pad+original_len] region has full overlap
    // coverage, so window_sum is bounded well above zero throughout.
    for i in 0..output_len {
        if window_sum[i] > 1e-10 {
            output[i] /= window_sum[i];
        }
    }

    // Strip the zero-padding applied in `stft` and return exactly original_len samples.
    let pad = nperseg / 2;
    let start = pad.min(output_len);
    let end = (pad + original_len).min(output_len);
    output[start..end].to_vec()
}

/// Generate a Hann window of given length.
pub fn hann_window(length: usize) -> Vec<f32> {
    (0..length)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / length as f32).cos()))
        .collect()
}

/// Compute a real FFT of a signal, returning positive-frequency complex values.
pub fn real_fft(signal: &[f32]) -> Vec<Complex<f32>> {
    let n = signal.len();
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);

    let mut buf: Vec<Complex<f32>> = signal.iter().map(|&s| Complex::new(s, 0.0)).collect();
    fft.process(&mut buf);

    buf[..n / 2 + 1].to_vec()
}

/// Compute inverse real FFT from positive-frequency complex values.
pub fn real_ifft(spectrum: &[Complex<f32>], output_len: usize) -> Vec<f32> {
    let mut planner = FftPlanner::new();
    let ifft = planner.plan_fft_inverse(output_len);

    let mut buf = vec![Complex::new(0.0f32, 0.0); output_len];
    let n_freqs = spectrum.len();

    for (i, &val) in spectrum.iter().enumerate() {
        buf[i] = val;
    }
    for i in 1..output_len - n_freqs + 1 {
        buf[output_len - i] = spectrum[i].conj();
    }

    ifft.process(&mut buf);

    let scale = 1.0 / output_len as f32;
    buf.iter().map(|c| c.re * scale).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::f32::consts::PI;

    #[test]
    fn test_stft_roundtrip_identity() {
        let sr = 48000.0;
        let len = 48000;
        let signal: Vec<f32> = (0..len)
            .map(|i| {
                let t = i as f32 / sr;
                0.25 * (2.0 * PI * 440.0 * t).sin()
                    + 0.25 * (2.0 * PI * 5000.0 * t).sin()
                    + 0.25 * (2.0 * PI * 15000.0 * t).sin()
                    + 0.25 * (2.0 * PI * 20000.0 * t).sin()
            })
            .collect();

        let nperseg = 2048;
        let noverlap = nperseg * 3 / 4;

        let (spec, orig_len) = stft(&signal, nperseg, noverlap);
        let reconstructed = istft(&spec, nperseg, noverlap, orig_len);

        assert_eq!(reconstructed.len(), signal.len());

        let mut max_err: f32 = 0.0;
        for i in 0..signal.len() {
            max_err = max_err.max((signal[i] - reconstructed[i]).abs());
        }
        assert!(
            max_err < 1e-5,
            "STFT roundtrip max error too large: {max_err}"
        );
    }

    proptest! {
        #[test]
        fn prop_stft_roundtrip_preserves_length(len in 4096usize..16384) {
            let signal: Vec<f32> = (0..len)
                .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
                .collect();
            let nperseg = 2048;
            let noverlap = nperseg * 3 / 4;
            let (spec, orig_len) = stft(&signal, nperseg, noverlap);
            let reconstructed = istft(&spec, nperseg, noverlap, orig_len);
            prop_assert_eq!(reconstructed.len(), signal.len());
        }

        #[test]
        fn prop_stft_roundtrip_low_error(freq in 100.0f32..10000.0) {
            let len = 8192;
            let signal: Vec<f32> = (0..len)
                .map(|i| (2.0 * PI * freq * i as f32 / 44100.0).sin())
                .collect();
            let nperseg = 2048;
            let noverlap = nperseg * 3 / 4;
            let (spec, orig_len) = stft(&signal, nperseg, noverlap);
            let reconstructed = istft(&spec, nperseg, noverlap, orig_len);
            let max_err = signal.iter().zip(&reconstructed)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            prop_assert!(max_err < 1e-4, "Max error {max_err} for freq {freq}");
        }

        #[test]
        fn prop_real_fft_ifft_roundtrip(len in 512usize..4096) {
            // Ensure even length for real FFT symmetry
            let len = len & !1;
            let signal: Vec<f32> = (0..len)
                .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin() * 0.5)
                .collect();
            let spectrum = real_fft(&signal);
            let reconstructed = real_ifft(&spectrum, len);
            let max_err = signal.iter().zip(&reconstructed)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            prop_assert!(max_err < 1e-4, "Real FFT roundtrip error {max_err}");
        }
    }
}
