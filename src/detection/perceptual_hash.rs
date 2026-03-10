//! Chroma-based perceptual audio hashing and comparison.
//!
//! Generates compact fingerprints from audio content that are robust to amplitude
//! scaling, and compares them using Hamming distance for similarity scoring.

use serde::Serialize;

use crate::audio::AudioBuffer;
use crate::sanitization::dsp::stft;

/// Number of chroma bins (one per semitone in an octave).
const CHROMA_BINS: usize = 12;

/// Result of perceptual hashing for a single file.
#[derive(Debug, Clone, Serialize)]
pub struct PerceptualHash {
    /// Packed hash words (32 bits each) derived from chroma frame comparisons.
    pub hash: Vec<u32>,
    /// Duration of the source audio in seconds.
    pub duration_secs: f64,
    /// Sample rate of the source audio.
    pub sample_rate: u32,
}

/// Result of comparing two perceptual hashes.
#[derive(Debug, Clone, Serialize)]
pub struct HashComparison {
    /// Path or identifier of the first file.
    pub file_a: String,
    /// Path or identifier of the second file.
    pub file_b: String,
    /// Similarity score between the two hashes (0.0 to 1.0).
    pub similarity: f64,
    /// Number of hash words in the first file's hash.
    pub hash_length_a: usize,
    /// Number of hash words in the second file's hash.
    pub hash_length_b: usize,
}

/// Compute a perceptual hash for an audio buffer.
///
/// The algorithm:
/// 1. Convert to mono
/// 2. Compute STFT with fixed window size
/// 3. Map FFT bins to 12 chroma bins (musical pitch classes)
/// 4. For each frame, generate a 12-bit hash by comparing each chroma bin
///    to its value in the previous frame (1 if energy increased, 0 otherwise)
/// 5. Pack consecutive frames into u32 words
pub fn compute_hash(buffer: &AudioBuffer) -> PerceptualHash {
    let mono = buffer.to_mono();
    let channel: Vec<f32> = mono.channel(0).to_vec();
    let sr = buffer.sample_rate;

    let nperseg = 4096;
    let noverlap = nperseg * 3 / 4;

    if channel.len() < nperseg * 2 {
        return PerceptualHash {
            hash: Vec::new(),
            duration_secs: buffer.duration_secs(),
            sample_rate: sr,
        };
    }

    let (spectrogram, _) = stft::stft(&channel, nperseg, noverlap);

    if spectrogram.is_empty() {
        return PerceptualHash {
            hash: Vec::new(),
            duration_secs: buffer.duration_secs(),
            sample_rate: sr,
        };
    }

    // Compute chroma features for each frame
    let n_freqs = spectrogram[0].len();
    let freq_resolution = sr as f64 / nperseg as f64;
    let chroma_frames = compute_chroma(&spectrogram, n_freqs, freq_resolution);

    // Generate hash bits by comparing adjacent chroma frames
    let mut hash_bits: Vec<u8> = Vec::new();
    for i in 1..chroma_frames.len() {
        for bin in 0..CHROMA_BINS {
            if chroma_frames[i][bin] > chroma_frames[i - 1][bin] {
                hash_bits.push(1);
            } else {
                hash_bits.push(0);
            }
        }
    }

    // Pack into u32 words (32 bits each)
    let mut hash: Vec<u32> = Vec::new();
    for chunk in hash_bits.chunks(32) {
        let mut word: u32 = 0;
        for (j, &bit) in chunk.iter().enumerate() {
            word |= (bit as u32) << j;
        }
        hash.push(word);
    }

    PerceptualHash {
        hash,
        duration_secs: buffer.duration_secs(),
        sample_rate: sr,
    }
}

/// Compute similarity between two perceptual hashes (0.0 to 1.0).
///
/// Uses normalized Hamming distance over the overlapping portion of both hashes.
pub fn compare_hashes(a: &PerceptualHash, b: &PerceptualHash) -> f64 {
    if a.hash.is_empty() || b.hash.is_empty() {
        return 0.0;
    }

    let len = a.hash.len().min(b.hash.len());
    let mut matching_bits: u32 = 0;
    let mut total_bits: u32 = 0;

    for i in 0..len {
        let xor = a.hash[i] ^ b.hash[i];
        let differing = xor.count_ones();
        matching_bits += 32 - differing;
        total_bits += 32;
    }

    if total_bits == 0 {
        return 0.0;
    }

    matching_bits as f64 / total_bits as f64
}

/// Map STFT magnitude spectrum to 12 chroma bins.
///
/// Each bin accumulates energy from all octaves of the corresponding pitch class.
/// Frequencies below 65 Hz (C2) or above Nyquist are ignored.
fn compute_chroma(
    spectrogram: &[Vec<num_complex::Complex<f32>>],
    n_freqs: usize,
    freq_resolution: f64,
) -> Vec<[f64; CHROMA_BINS]> {
    let mut chroma_frames = Vec::with_capacity(spectrogram.len());

    // Reference frequency: A4 = 440 Hz
    let a4 = 440.0f64;

    for frame in spectrogram {
        let mut chroma = [0.0f64; CHROMA_BINS];

        for (bin, val) in frame.iter().enumerate().take(n_freqs).skip(1) {
            let freq = bin as f64 * freq_resolution;
            if !(65.0..=8000.0).contains(&freq) {
                continue;
            }

            // Map frequency to chroma bin using equal temperament
            // semitone = 12 * log2(freq / A4) mod 12
            let semitone = (12.0 * (freq / a4).log2()) % 12.0;
            let chroma_bin = ((semitone + 12.0) % 12.0) as usize % CHROMA_BINS;

            let magnitude = val.norm() as f64;
            chroma[chroma_bin] += magnitude * magnitude;
        }

        // Normalize chroma vector
        let max_val = chroma.iter().cloned().fold(0.0f64, f64::max);
        if max_val > 1e-10 {
            for val in &mut chroma {
                *val /= max_val;
            }
        }

        chroma_frames.push(chroma);
    }

    chroma_frames
}

/// Format a hash as a hex string for display.
pub fn hash_to_hex(hash: &[u32]) -> String {
    hash.iter().map(|w| format!("{w:08x}")).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn sine_buffer(freq: f32, sr: u32, duration: f32) -> AudioBuffer {
        let len = (sr as f32 * duration) as usize;
        let data: Vec<f32> = (0..len)
            .map(|i| 0.5 * (2.0 * PI * freq * i as f32 / sr as f32).sin())
            .collect();
        AudioBuffer::from_mono(data, sr)
    }

    fn multi_tone(sr: u32, duration: f32) -> AudioBuffer {
        let len = (sr as f32 * duration) as usize;
        let data: Vec<f32> = (0..len)
            .map(|i| {
                let t = i as f32 / sr as f32;
                0.3 * (2.0 * PI * 440.0 * t).sin()
                    + 0.2 * (2.0 * PI * 880.0 * t).sin()
                    + 0.1 * (2.0 * PI * 1320.0 * t).sin()
            })
            .collect();
        AudioBuffer::from_mono(data, sr)
    }

    #[test]
    fn test_same_signal_high_similarity() {
        let buf = multi_tone(44100, 2.0);
        let hash_a = compute_hash(&buf);
        let hash_b = compute_hash(&buf);
        let similarity = compare_hashes(&hash_a, &hash_b);
        assert!(
            similarity > 0.99,
            "Same signal should have >0.99 similarity, got {similarity}"
        );
    }

    #[test]
    fn test_different_signals_low_similarity() {
        let buf_a = sine_buffer(440.0, 44100, 2.0);
        let buf_b = sine_buffer(1000.0, 44100, 2.0);
        let hash_a = compute_hash(&buf_a);
        let hash_b = compute_hash(&buf_b);
        let similarity = compare_hashes(&hash_a, &hash_b);
        assert!(
            similarity < 0.8,
            "Different signals should have <0.8 similarity, got {similarity}"
        );
    }

    #[test]
    fn test_scaled_signal_high_similarity() {
        let buf_a = multi_tone(44100, 2.0);
        // Scale amplitude — should still be similar (chroma is normalized)
        let len = (44100.0 * 2.0) as usize;
        let data: Vec<f32> = (0..len)
            .map(|i| {
                let t = i as f32 / 44100.0;
                0.5 * (0.3 * (2.0 * PI * 440.0 * t).sin()
                    + 0.2 * (2.0 * PI * 880.0 * t).sin()
                    + 0.1 * (2.0 * PI * 1320.0 * t).sin())
            })
            .collect();
        let buf_b = AudioBuffer::from_mono(data, 44100);
        let hash_a = compute_hash(&buf_a);
        let hash_b = compute_hash(&buf_b);
        let similarity = compare_hashes(&hash_a, &hash_b);
        assert!(
            similarity > 0.9,
            "Scaled signal should have >0.9 similarity, got {similarity}"
        );
    }

    #[test]
    fn test_short_signal_empty_hash() {
        let buf = AudioBuffer::from_mono(vec![0.0; 100], 44100);
        let hash = compute_hash(&buf);
        assert!(hash.hash.is_empty());
    }

    #[test]
    fn test_empty_hashes_zero_similarity() {
        let a = PerceptualHash {
            hash: Vec::new(),
            duration_secs: 0.0,
            sample_rate: 44100,
        };
        let b = PerceptualHash {
            hash: Vec::new(),
            duration_secs: 0.0,
            sample_rate: 44100,
        };
        assert_eq!(compare_hashes(&a, &b), 0.0);
    }

    #[test]
    fn test_hash_to_hex_format() {
        let hash = vec![0xdeadbeef_u32, 0x12345678];
        let hex = hash_to_hex(&hash);
        assert_eq!(hex, "deadbeef12345678");
    }
}
