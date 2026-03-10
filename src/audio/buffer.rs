use ndarray::Array2;
use zeroize::Zeroize;

/// Central audio data type. Samples are f32 in range [-1.0, 1.0].
/// Shape: (num_samples, num_channels)
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: Array2<f32>,
    pub sample_rate: u32,
}

impl AudioBuffer {
    pub fn new(samples: Array2<f32>, sample_rate: u32) -> Self {
        Self {
            samples,
            sample_rate,
        }
    }

    /// Create a mono buffer from a Vec of f32 samples.
    pub fn from_mono(data: Vec<f32>, sample_rate: u32) -> Self {
        let len = data.len();
        let samples = Array2::from_shape_vec((len, 1), data).expect("failed to create mono buffer");
        Self {
            samples,
            sample_rate,
        }
    }

    /// Create a stereo buffer from interleaved samples.
    pub fn from_interleaved(data: &[f32], channels: usize, sample_rate: u32) -> Self {
        let num_samples = data.len() / channels;
        let mut samples = Array2::zeros((num_samples, channels));
        for (i, chunk) in data.chunks_exact(channels).enumerate() {
            for (ch, &val) in chunk.iter().enumerate() {
                samples[[i, ch]] = val;
            }
        }
        Self {
            samples,
            sample_rate,
        }
    }

    pub fn num_samples(&self) -> usize {
        self.samples.nrows()
    }

    pub fn num_channels(&self) -> usize {
        self.samples.ncols()
    }

    pub fn duration_secs(&self) -> f64 {
        self.num_samples() as f64 / self.sample_rate as f64
    }

    pub fn is_mono(&self) -> bool {
        self.num_channels() == 1
    }

    pub fn is_stereo(&self) -> bool {
        self.num_channels() == 2
    }

    /// Get a read-only view of a single channel.
    pub fn channel(&self, idx: usize) -> ndarray::ArrayView1<'_, f32> {
        self.samples.column(idx)
    }

    /// Get a mutable view of a single channel.
    pub fn channel_mut(&mut self, idx: usize) -> ndarray::ArrayViewMut1<'_, f32> {
        self.samples.column_mut(idx)
    }

    /// Convert to mono by averaging channels.
    pub fn to_mono(&self) -> AudioBuffer {
        if self.is_mono() {
            return self.clone();
        }
        let num_samples = self.num_samples();
        let channels = self.num_channels() as f32;
        let mut mono = Vec::with_capacity(num_samples);
        for i in 0..num_samples {
            let sum: f32 = (0..self.num_channels())
                .map(|ch| self.samples[[i, ch]])
                .sum();
            mono.push(sum / channels);
        }
        AudioBuffer::from_mono(mono, self.sample_rate)
    }

    /// Get mono samples as a Vec<f32>.
    pub fn to_mono_samples(&self) -> Vec<f32> {
        if self.is_mono() {
            self.samples.column(0).to_vec()
        } else {
            let num_samples = self.num_samples();
            let channels = self.num_channels() as f32;
            let mut mono = Vec::with_capacity(num_samples);
            for i in 0..num_samples {
                let sum: f32 = (0..self.num_channels())
                    .map(|ch| self.samples[[i, ch]])
                    .sum();
                mono.push(sum / channels);
            }
            mono
        }
    }

    /// Get interleaved samples for writing.
    pub fn to_interleaved(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.num_samples() * self.num_channels());
        for i in 0..self.num_samples() {
            for ch in 0..self.num_channels() {
                out.push(self.samples[[i, ch]]);
            }
        }
        out
    }

    /// RMS level across all channels.
    pub fn rms(&self) -> f32 {
        let total: f32 = self.samples.iter().map(|&s| s * s).sum();
        let count = self.samples.len() as f32;
        (total / count).sqrt()
    }

    /// Peak absolute value.
    pub fn peak(&self) -> f32 {
        self.samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max)
    }

    /// Normalize RMS to target level.
    pub fn normalize_rms(&mut self, target_rms: f32) {
        let current = self.rms();
        if current > 1e-10 {
            let gain = target_rms / current;
            self.samples.mapv_inplace(|s| s * gain);
        }
    }

    /// Soft clip samples above threshold using tanh.
    pub fn soft_clip(&mut self, threshold: f32) {
        self.samples.mapv_inplace(|s| {
            if s.abs() > threshold {
                threshold * (s / threshold).tanh()
            } else {
                s
            }
        });
    }

    /// Hard clip to [-1.0, 1.0].
    pub fn hard_clip(&mut self) {
        self.samples.mapv_inplace(|s| s.clamp(-1.0, 1.0));
    }

    /// Split buffer into overlapping chunks for chunked processing.
    /// Returns chunks and the overlap size in samples.
    pub fn split_chunks(&self, chunk_samples: usize, overlap_samples: usize) -> Vec<AudioBuffer> {
        let total = self.num_samples();
        let step = chunk_samples.saturating_sub(overlap_samples).max(1);
        let mut chunks = Vec::new();
        let mut start = 0;

        while start < total {
            let end = (start + chunk_samples).min(total);
            let slice = self.samples.slice(ndarray::s![start..end, ..]);
            let chunk_arr = slice.to_owned();
            chunks.push(AudioBuffer::new(chunk_arr, self.sample_rate));
            if end >= total {
                break;
            }
            start += step;
        }

        chunks
    }

    /// Join overlapping chunks with crossfade (overlap-add).
    pub fn join_chunks(chunks: &[AudioBuffer], overlap_samples: usize) -> AudioBuffer {
        if chunks.is_empty() {
            return AudioBuffer::from_mono(vec![], 44100);
        }
        if chunks.len() == 1 {
            return chunks[0].clone();
        }

        let sample_rate = chunks[0].sample_rate;
        let channels = chunks[0].num_channels();

        // Calculate total output length
        let first_len = chunks[0].num_samples();
        let total: usize = first_len
            + chunks[1..]
                .iter()
                .map(|c| c.num_samples().saturating_sub(overlap_samples))
                .sum::<usize>();

        let mut output = Array2::<f32>::zeros((total, channels));
        let mut pos = 0;

        for (i, chunk) in chunks.iter().enumerate() {
            let n = chunk.num_samples();
            if i == 0 {
                output
                    .slice_mut(ndarray::s![0..n, ..])
                    .assign(&chunk.samples);
                pos = n;
            } else {
                let overlap = overlap_samples.min(n).min(pos);
                // Crossfade the overlap region
                for j in 0..overlap {
                    let fade_out = (overlap - j) as f32 / overlap as f32;
                    let fade_in = j as f32 / overlap as f32;
                    let out_idx = pos - overlap + j;
                    for ch in 0..channels {
                        output[[out_idx, ch]] =
                            output[[out_idx, ch]] * fade_out + chunk.samples[[j, ch]] * fade_in;
                    }
                }
                // Copy the non-overlapping part
                let remaining = n.saturating_sub(overlap);
                if remaining > 0 {
                    output
                        .slice_mut(ndarray::s![pos..pos + remaining, ..])
                        .assign(&chunk.samples.slice(ndarray::s![overlap..n, ..]));
                    pos += remaining;
                }
            }
        }

        AudioBuffer::new(output, sample_rate)
    }
}

impl Drop for AudioBuffer {
    fn drop(&mut self) {
        if let Some(slice) = self.samples.as_slice_mut() {
            slice.zeroize();
        } else {
            // Non-contiguous layout: zero element-by-element
            for val in self.samples.iter_mut() {
                val.zeroize();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_mono(len: usize, freq: f32, sr: u32) -> AudioBuffer {
        let data: Vec<f32> = (0..len)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin())
            .collect();
        AudioBuffer::from_mono(data, sr)
    }

    #[test]
    fn test_from_mono() {
        let buf = AudioBuffer::from_mono(vec![0.1, 0.2, 0.3], 44100);
        assert_eq!(buf.num_samples(), 3);
        assert_eq!(buf.num_channels(), 1);
        assert!(buf.is_mono());
        assert!(!buf.is_stereo());
        assert_eq!(buf.sample_rate, 44100);
    }

    #[test]
    fn test_from_interleaved_stereo() {
        let interleaved = vec![0.1, -0.1, 0.2, -0.2, 0.3, -0.3];
        let buf = AudioBuffer::from_interleaved(&interleaved, 2, 48000);
        assert_eq!(buf.num_samples(), 3);
        assert_eq!(buf.num_channels(), 2);
        assert!(buf.is_stereo());
        assert!((buf.channel(0)[0] - 0.1).abs() < 1e-6);
        assert!((buf.channel(1)[0] - -0.1).abs() < 1e-6);
    }

    #[test]
    fn test_duration_secs() {
        let buf = AudioBuffer::from_mono(vec![0.0; 44100], 44100);
        assert!((buf.duration_secs() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_to_mono_from_stereo() {
        let interleaved = vec![0.4, 0.6, 0.2, 0.8];
        let buf = AudioBuffer::from_interleaved(&interleaved, 2, 44100);
        let mono = buf.to_mono();
        assert!(mono.is_mono());
        assert_eq!(mono.num_samples(), 2);
        assert!((mono.channel(0)[0] - 0.5).abs() < 1e-6);
        assert!((mono.channel(0)[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_to_mono_samples() {
        let buf = AudioBuffer::from_mono(vec![0.1, 0.2, 0.3], 44100);
        let samples = buf.to_mono_samples();
        assert_eq!(samples.len(), 3);
        assert!((samples[1] - 0.2).abs() < 1e-6);
    }

    #[test]
    fn test_to_interleaved_roundtrip() {
        let original = vec![0.1, -0.1, 0.2, -0.2];
        let buf = AudioBuffer::from_interleaved(&original, 2, 44100);
        let result = buf.to_interleaved();
        for (a, b) in original.iter().zip(result.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_rms() {
        // Constant signal: RMS should equal the value
        let buf = AudioBuffer::from_mono(vec![0.5; 100], 44100);
        assert!((buf.rms() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_peak() {
        let buf = AudioBuffer::from_mono(vec![0.1, -0.9, 0.3, 0.5], 44100);
        assert!((buf.peak() - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_rms() {
        let mut buf = sine_mono(4410, 440.0, 44100);
        buf.normalize_rms(0.1);
        assert!((buf.rms() - 0.1).abs() < 1e-4);
    }

    #[test]
    fn test_hard_clip() {
        let mut buf = AudioBuffer::from_mono(vec![1.5, -2.0, 0.5], 44100);
        buf.hard_clip();
        assert!((buf.channel(0)[0] - 1.0).abs() < 1e-6);
        assert!((buf.channel(0)[1] - -1.0).abs() < 1e-6);
        assert!((buf.channel(0)[2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_soft_clip() {
        let mut buf = AudioBuffer::from_mono(vec![2.0, 0.3, -2.0], 44100);
        buf.soft_clip(0.8);
        assert!(buf.channel(0)[0] < 1.0);
        assert!(buf.channel(0)[0] > 0.7);
        assert!((buf.channel(0)[1] - 0.3).abs() < 1e-6); // below threshold, unchanged
    }

    #[test]
    fn test_channel_mut() {
        let mut buf = AudioBuffer::from_mono(vec![0.0; 10], 44100);
        {
            let mut ch = buf.channel_mut(0);
            ch[5] = 0.42;
        }
        assert!((buf.channel(0)[5] - 0.42).abs() < 1e-6);
    }

    #[test]
    fn test_split_and_join_chunks() {
        let original =
            AudioBuffer::from_mono((0..1000).map(|i| i as f32 / 1000.0).collect(), 44100);
        let chunks = original.split_chunks(300, 50);
        assert!(chunks.len() >= 3);

        let joined = AudioBuffer::join_chunks(&chunks, 50);
        assert_eq!(joined.num_samples(), original.num_samples());
    }

    #[test]
    fn test_join_single_chunk() {
        let buf = AudioBuffer::from_mono(vec![0.1, 0.2], 44100);
        let joined = AudioBuffer::join_chunks(std::slice::from_ref(&buf), 0);
        assert_eq!(joined.num_samples(), 2);
    }

    #[test]
    fn test_join_empty_chunks() {
        let joined = AudioBuffer::join_chunks(&[], 0);
        assert_eq!(joined.num_samples(), 0);
    }
}
