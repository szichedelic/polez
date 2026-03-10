use ndarray::Array2;

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
