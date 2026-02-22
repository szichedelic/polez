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
fn linear_resample(signal: &[f32], ratio: f64) -> Vec<f32> {
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
