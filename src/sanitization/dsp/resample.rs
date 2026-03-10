use rubato::{FftFixedInOut, Resampler};

/// Resample a mono signal from one sample rate to another using rubato.
pub fn resample(signal: &[f32], sr_in: u32, sr_out: u32) -> Vec<f32> {
    if sr_in == sr_out || signal.is_empty() {
        return signal.to_vec();
    }

    let ratio = sr_out as f64 / sr_in as f64;

    // Use FFT-based resampler for quality
    let chunk_size = 1024;
    let resampler = FftFixedInOut::<f32>::new(sr_in as usize, sr_out as usize, chunk_size, 1);
    if resampler.is_err() {
        // Fallback to linear interpolation if rubato fails
        return linear_resample(signal, ratio);
    }
    let mut resampler = resampler.unwrap();

    let input_frames_needed = resampler.input_frames_next();
    let mut output = Vec::with_capacity((signal.len() as f64 * ratio) as usize + 1024);

    // Process in chunks
    let mut pos = 0;
    while pos + input_frames_needed <= signal.len() {
        let chunk = vec![signal[pos..pos + input_frames_needed].to_vec()];
        if let Ok(result) = resampler.process(&chunk, None) {
            if let Some(ch) = result.first() {
                output.extend_from_slice(ch);
            }
        }
        pos += input_frames_needed;
    }

    // Handle remaining samples with zero-padding
    if pos < signal.len() {
        let remaining = signal.len() - pos;
        let mut padded = signal[pos..].to_vec();
        padded.resize(input_frames_needed, 0.0);
        let chunk = vec![padded];
        if let Ok(result) = resampler.process(&chunk, None) {
            if let Some(ch) = result.first() {
                // Only take the proportional amount of output
                let out_samples = (remaining as f64 * ratio) as usize;
                let take = out_samples.min(ch.len());
                output.extend_from_slice(&ch[..take]);
            }
        }
    }

    output
}

/// Simple linear interpolation resampling as a fallback.
pub fn linear_resample(signal: &[f32], ratio: f64) -> Vec<f32> {
    let out_len = (signal.len() as f64 * ratio) as usize;
    let mut output = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_idx = i as f64 / ratio;
        let idx0 = src_idx.floor() as usize;
        let idx1 = (idx0 + 1).min(signal.len() - 1);
        let frac = src_idx - idx0 as f64;

        let sample = signal[idx0] as f64 * (1.0 - frac) + signal[idx1] as f64 * frac;
        output.push(sample as f32);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn signal_energy(signal: &[f32]) -> f64 {
        signal.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / signal.len() as f64
    }

    proptest! {
        #[test]
        fn prop_resample_identity_same_rate(len in 1024usize..8192) {
            let signal: Vec<f32> = (0..len)
                .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
                .collect();
            let result = resample(&signal, 44100, 44100);
            prop_assert_eq!(result.len(), signal.len());
            let max_diff = signal.iter().zip(&result)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            prop_assert!(max_diff < 1e-6, "Same-rate resample changed signal: {max_diff}");
        }

        #[test]
        fn prop_resample_preserves_energy(sr_out in prop::sample::select(vec![22050u32, 32000, 48000, 96000])) {
            let sr_in = 44100u32;
            let len = 4096;
            let signal: Vec<f32> = (0..len)
                .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr_in as f32).sin())
                .collect();
            let result = resample(&signal, sr_in, sr_out);
            let in_energy = signal_energy(&signal);
            let out_energy = signal_energy(&result);
            // Energy per sample should be roughly preserved (within 20% tolerance)
            let ratio = out_energy / in_energy;
            prop_assert!(ratio > 0.8 && ratio < 1.2,
                "Energy ratio {ratio} out of range for {sr_in} -> {sr_out}");
        }

        #[test]
        fn prop_resample_correct_length(sr_out in prop::sample::select(vec![22050u32, 48000, 96000])) {
            let sr_in = 44100u32;
            let len = 4096;
            let signal: Vec<f32> = (0..len)
                .map(|i| (i as f32 * 0.01).sin())
                .collect();
            let result = resample(&signal, sr_in, sr_out);
            let expected_len = (len as f64 * sr_out as f64 / sr_in as f64) as usize;
            // Allow tolerance on output length due to chunking
            let diff = (result.len() as i64 - expected_len as i64).unsigned_abs();
            prop_assert!(diff < expected_len as u64 / 20 + 1024,
                "Output len {} far from expected {} for {} -> {}", result.len(), expected_len, sr_in, sr_out);
        }
    }
}
