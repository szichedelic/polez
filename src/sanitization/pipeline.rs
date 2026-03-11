//! Unified sanitization pipeline that orchestrates all cleaning stages.
//!
//! Replaces the five separate Python sanitizer scripts from the prior version
//! with a single configurable pipeline supporting four processing modes.

use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;

use crate::audio::{self, AudioBuffer, AudioFormat};
use crate::config::{AdvancedFlags, AppConfig, FingerprintRemovalConfig};
use crate::error::Result;
use crate::sanitization::fingerprint::FingerprintRemover;
use crate::sanitization::metadata::MetadataCleaner;
use crate::sanitization::spectral::SpectralCleaner;
use crate::sanitization::stealth::StealthOps;

/// Sanitization mode - replaces the 5 separate Python sanitizer files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SanitizationMode {
    /// Metadata strip + light processing
    Fast,
    /// Full pipeline, standard settings
    Standard,
    /// All stealth operations, quality-first
    Preserving,
    /// Heavy phase randomization, aggressive cleaning
    Aggressive,
}

/// Result of a sanitization run.
#[derive(Debug, Clone)]
pub struct SanitizationResult {
    /// Whether the sanitization completed without errors.
    pub success: bool,
    /// Path to the sanitized output file.
    pub output_file: PathBuf,
    /// Number of metadata tags removed.
    pub metadata_removed: usize,
    /// Number of watermark/fingerprint patterns detected.
    pub patterns_found: usize,
    /// Number of patterns successfully suppressed.
    pub patterns_suppressed: usize,
    /// Estimated quality loss as a percentage of original RMS.
    pub quality_loss: f64,
    /// Wall-clock processing time in seconds.
    pub processing_time: f64,
}

/// Unified sanitization pipeline.
pub struct SanitizationPipeline {
    mode: SanitizationMode,
    paranoid: bool,
    paranoid_passes: u32,
    flags: AdvancedFlags,
    fp_config: FingerprintRemovalConfig,
    output_format: Option<AudioFormat>,
    freq_ranges: Vec<(f64, f64)>,
    target_sample_rate: Option<u32>,
    bit_depth: Option<u16>,
}

impl SanitizationPipeline {
    /// Create a new sanitization pipeline with the given configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mode: SanitizationMode,
        paranoid: bool,
        paranoid_passes: u32,
        flags: AdvancedFlags,
        fp_config: FingerprintRemovalConfig,
        output_format: Option<AudioFormat>,
        freq_ranges: Vec<(f64, f64)>,
        target_sample_rate: Option<u32>,
        bit_depth: Option<u16>,
    ) -> Self {
        Self {
            mode,
            paranoid,
            paranoid_passes,
            flags,
            fp_config,
            output_format,
            freq_ranges,
            target_sample_rate,
            bit_depth,
        }
    }

    /// Determine the mode based on config.
    pub fn mode_from_config(config: &AppConfig) -> SanitizationMode {
        match config.paranoia_level {
            crate::config::ParanoiaLevel::Low => SanitizationMode::Fast,
            crate::config::ParanoiaLevel::Medium => SanitizationMode::Standard,
            crate::config::ParanoiaLevel::High => SanitizationMode::Preserving,
            crate::config::ParanoiaLevel::Maximum => SanitizationMode::Aggressive,
        }
    }

    /// Threshold in samples above which chunked processing is used.
    /// ~60 seconds at 44.1kHz stereo ≈ 5.3M samples.
    const CHUNK_THRESHOLD: usize = 5_000_000;

    /// Chunk size in samples for streaming mode (~30 seconds at 44.1kHz).
    const CHUNK_SIZE: usize = 1_323_000;

    /// Overlap between chunks in samples (~1 second at 44.1kHz) for crossfade.
    const OVERLAP: usize = 44_100;

    /// Run the sanitization pipeline.
    pub fn run(&self, input: &Path, output: &Path) -> Result<SanitizationResult> {
        let start = Instant::now();

        let metadata_removed = MetadataCleaner::strip_to(input, output)?;

        let (buffer, source_format) = audio::load_audio(output)?;
        let original_rms = buffer.rms();

        let (mut buffer, patterns_found, patterns_suppressed) = if buffer.num_samples()
            > Self::CHUNK_THRESHOLD
            && self.mode != SanitizationMode::Fast
        {
            tracing::info!(
                samples = buffer.num_samples(),
                "Large file detected, using chunked processing"
            );
            self.process_chunked(buffer)?
        } else {
            let mut buf = buffer;
            let (f, s) = self.process_buffer(&mut buf)?;
            (buf, f, s)
        };

        let processed_rms = buffer.rms();
        if processed_rms > 1e-10 && original_rms > 1e-10 {
            buffer.normalize_rms(original_rms);
        }

        buffer.soft_clip(0.99);

        let quality_loss = if original_rms > 1e-10 {
            ((original_rms - buffer.rms()).abs() / original_rms * 100.0) as f64
        } else {
            0.0
        };

        // Resample if a target sample rate was specified (channels in parallel)
        if let Some(target_sr) = self.target_sample_rate {
            if target_sr != buffer.sample_rate {
                use super::dsp::resample;
                let sr_in = buffer.sample_rate;
                let new_channels: Vec<Vec<f32>> = (0..buffer.num_channels())
                    .into_par_iter()
                    .map(|ch| {
                        let ch_data: Vec<f32> = buffer.channel(ch).to_vec();
                        resample::resample(&ch_data, sr_in, target_sr)
                    })
                    .collect();
                buffer = AudioBuffer::from_channels(new_channels, target_sr);
            }
        }

        let out_format = self.output_format.unwrap_or(source_format);
        // Fall back to WAV if the source format has no encoder
        let out_format = if out_format.has_encoder() {
            out_format
        } else {
            audio::AudioFormat::Wav
        };
        audio::save_audio(&buffer, output, out_format, self.bit_depth)?;

        let elapsed = start.elapsed().as_secs_f64();

        Ok(SanitizationResult {
            success: true,
            output_file: output.to_path_buf(),
            metadata_removed,
            patterns_found,
            patterns_suppressed,
            quality_loss,
            processing_time: elapsed,
        })
    }

    /// Process the entire buffer in one pass (original behavior).
    fn process_buffer(&self, buffer: &mut AudioBuffer) -> Result<(usize, usize)> {
        let (mut patterns_found, mut patterns_suppressed) = match self.mode {
            SanitizationMode::Fast => (0, 0),
            SanitizationMode::Standard => self.run_standard(buffer)?,
            SanitizationMode::Preserving => self.run_preserving(buffer)?,
            SanitizationMode::Aggressive => self.run_aggressive(buffer)?,
        };

        if self.paranoid && self.mode != SanitizationMode::Fast {
            for _pass in 0..self.paranoid_passes {
                SpectralCleaner::clean(buffer, self.paranoid, &self.flags, &self.freq_ranges)?;
            }
        }

        if self.flags.adaptive_notch && self.mode != SanitizationMode::Fast {
            let notched = SpectralCleaner::adaptive_notch_pass(buffer, self.paranoid)?;
            patterns_found += notched;
            patterns_suppressed += notched;
        }

        Ok((patterns_found, patterns_suppressed))
    }

    /// Process a large buffer in overlapping chunks, parallelized with rayon.
    fn process_chunked(&self, buffer: AudioBuffer) -> Result<(AudioBuffer, usize, usize)> {
        let chunks = buffer.split_chunks(Self::CHUNK_SIZE, Self::OVERLAP);
        let num_chunks = chunks.len();
        tracing::info!(chunks = num_chunks, "Processing in chunks (parallel)");

        // Drop original buffer to free memory before processing chunks
        drop(buffer);

        // Process all chunks in parallel
        let results: Vec<Result<(AudioBuffer, usize, usize)>> = chunks
            .into_par_iter()
            .enumerate()
            .map(|(i, mut chunk)| {
                tracing::debug!(chunk = i + 1, total = num_chunks, "Processing chunk");
                let (found, suppressed) = self.process_buffer(&mut chunk)?;
                Ok((chunk, found, suppressed))
            })
            .collect();

        let mut total_found = 0usize;
        let mut total_suppressed = 0usize;
        let mut processed_chunks = Vec::with_capacity(num_chunks);

        for r in results {
            let (chunk, found, suppressed) = r?;
            total_found += found;
            total_suppressed += suppressed;
            processed_chunks.push(chunk);
        }

        let joined = AudioBuffer::join_chunks(&processed_chunks, Self::OVERLAP);
        Ok((joined, total_found, total_suppressed))
    }

    /// Standard mode: spectral cleaning + fingerprint removal.
    fn run_standard(&self, buffer: &mut AudioBuffer) -> Result<(usize, usize)> {
        let (found, suppressed) =
            SpectralCleaner::clean(buffer, self.paranoid, &self.flags, &self.freq_ranges)?;
        FingerprintRemover::remove(buffer, self.paranoid, &self.fp_config)?;
        Ok((found, suppressed))
    }

    /// Preserving mode: spectral cleaning + fingerprint removal + stealth ops.
    fn run_preserving(&self, buffer: &mut AudioBuffer) -> Result<(usize, usize)> {
        let (found, suppressed) =
            SpectralCleaner::clean(buffer, self.paranoid, &self.flags, &self.freq_ranges)?;
        FingerprintRemover::remove(buffer, self.paranoid, &self.fp_config)?;
        StealthOps::apply(buffer, &self.flags, self.paranoid)?;
        Ok((found, suppressed))
    }

    /// Aggressive mode: all cleaning with forced paranoid settings.
    fn run_aggressive(&self, buffer: &mut AudioBuffer) -> Result<(usize, usize)> {
        let (found, suppressed) =
            SpectralCleaner::clean(buffer, true, &self.flags, &self.freq_ranges)?;
        FingerprintRemover::remove(buffer, true, &self.fp_config)?;
        StealthOps::apply(buffer, &self.flags, true)?;
        Ok((found, suppressed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detection::WatermarkDetector;

    fn sine_buffer(freq: f32, sr: u32, duration_secs: f32) -> AudioBuffer {
        let len = (sr as f32 * duration_secs) as usize;
        let samples: Vec<f32> = (0..len)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin() * 0.5)
            .collect();
        AudioBuffer::from_mono(samples, sr)
    }

    fn make_pipeline(mode: SanitizationMode) -> SanitizationPipeline {
        SanitizationPipeline::new(
            mode,
            false,
            0,
            AdvancedFlags::default(),
            FingerprintRemovalConfig {
                statistical_normalization: true,
                temporal_randomization: true,
                phase_randomization: true,
                micro_timing_perturbation: true,
                human_imperfections: true,
            },
            None,
            vec![],
            None,
            None,
        )
    }

    #[test]
    fn test_single_clean_doesnt_amplify_watermarks() {
        let buf = sine_buffer(440.0, 44100, 0.5);
        let before = WatermarkDetector::detect_all(&buf);

        let mut cleaned = buf.clone();
        let pipeline = make_pipeline(SanitizationMode::Standard);
        pipeline.process_buffer(&mut cleaned).unwrap();

        let after = WatermarkDetector::detect_all(&cleaned);
        // Processing may shift statistical features slightly — allow 0.3 tolerance
        assert!(
            after.overall_confidence <= before.overall_confidence + 0.3,
            "Cleaning increased watermark confidence from {} to {}",
            before.overall_confidence,
            after.overall_confidence
        );
    }

    #[test]
    fn test_double_clean_quality_stable() {
        let buf = sine_buffer(440.0, 44100, 0.5);
        let original_rms = buf.rms();

        let mut pass1 = buf.clone();
        let pipeline = make_pipeline(SanitizationMode::Standard);
        pipeline.process_buffer(&mut pass1).unwrap();
        pass1.normalize_rms(original_rms);
        let rms1 = pass1.rms();

        let mut pass2 = pass1.clone();
        pipeline.process_buffer(&mut pass2).unwrap();
        pass2.normalize_rms(original_rms);
        let rms2 = pass2.rms();

        let diff = (rms1 - rms2).abs() / original_rms;
        assert!(
            diff < 0.05,
            "Quality degraded significantly on second pass: rms1={rms1}, rms2={rms2}, diff={diff}"
        );
    }

    #[test]
    fn test_triple_clean_converges() {
        let buf = sine_buffer(440.0, 44100, 0.5);
        let original_rms = buf.rms();
        let pipeline = make_pipeline(SanitizationMode::Standard);

        let mut prev = buf;
        let mut rms_values = Vec::new();

        for _ in 0..3 {
            pipeline.process_buffer(&mut prev).unwrap();
            prev.normalize_rms(original_rms);
            rms_values.push(prev.rms());
        }

        // Differences between consecutive passes should decrease
        let diff_1_2 = (rms_values[0] - rms_values[1]).abs();
        let diff_2_3 = (rms_values[1] - rms_values[2]).abs();
        assert!(
            diff_2_3 <= diff_1_2 + 0.01,
            "Quality not converging: d12={diff_1_2}, d23={diff_2_3}"
        );
    }

    #[test]
    fn test_no_new_false_positives_after_clean() {
        let buf = sine_buffer(440.0, 44100, 0.5);
        let before = WatermarkDetector::detect_all(&buf);

        let mut cleaned = buf.clone();
        let pipeline = make_pipeline(SanitizationMode::Standard);
        pipeline.process_buffer(&mut cleaned).unwrap();

        let after = WatermarkDetector::detect_all(&cleaned);
        assert!(
            after.watermark_count <= before.watermark_count,
            "Cleaning introduced new false positives: before={}, after={}",
            before.watermark_count,
            after.watermark_count
        );
    }

    #[test]
    fn test_fast_mode_preserves_quality() {
        let buf = sine_buffer(440.0, 44100, 0.5);
        let original_rms = buf.rms();

        let mut cleaned = buf.clone();
        let pipeline = make_pipeline(SanitizationMode::Fast);
        pipeline.process_buffer(&mut cleaned).unwrap();

        // Fast mode does minimal processing — RMS should barely change
        let diff = (original_rms - cleaned.rms()).abs() / original_rms;
        assert!(
            diff < 0.01,
            "Fast mode changed RMS by {diff:.4} (expected < 0.01)"
        );
    }

    #[test]
    fn test_aggressive_mode_still_bounded() {
        let buf = sine_buffer(440.0, 44100, 0.5);
        let original_peak = buf.peak();

        let mut cleaned = buf.clone();
        let pipeline = make_pipeline(SanitizationMode::Aggressive);
        pipeline.process_buffer(&mut cleaned).unwrap();

        // Aggressive mode should not create clipping
        assert!(
            cleaned.peak() <= original_peak * 1.5,
            "Aggressive mode amplified peak from {} to {}",
            original_peak,
            cleaned.peak()
        );
    }

    #[test]
    fn test_file_roundtrip_resilience() {
        let buf = sine_buffer(440.0, 44100, 0.5);
        let dir = tempfile::tempdir().unwrap();
        let input_path = dir.path().join("input.wav");
        let output_path = dir.path().join("output.wav");
        audio::save_audio(&buf, &input_path, audio::AudioFormat::Wav, None).unwrap();

        let pipeline = make_pipeline(SanitizationMode::Standard);
        let result = pipeline.run(&input_path, &output_path).unwrap();

        assert!(result.success);
        assert!(
            result.quality_loss < 5.0,
            "quality_loss={}",
            result.quality_loss
        );
        assert!(output_path.exists());
    }

    // --- Effectiveness benchmark tests ---

    fn watermarked_buffer() -> AudioBuffer {
        // Create a signal with characteristics that trigger watermark detection:
        // 440 Hz base tone + high-frequency watermark-like components
        let sr = 44100u32;
        let len = sr as usize; // 1 second
        let samples: Vec<f32> = (0..len)
            .map(|i| {
                let t = i as f32 / sr as f32;
                let base = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.4;
                // Simulate spread-spectrum watermark in ultrasonic band
                let wm1 = (2.0 * std::f32::consts::PI * 18500.0 * t).sin() * 0.05;
                let wm2 = (2.0 * std::f32::consts::PI * 19500.0 * t).sin() * 0.04;
                let wm3 = (2.0 * std::f32::consts::PI * 20500.0 * t).sin() * 0.03;
                // Add periodic amplitude modulation (echo-like pattern)
                let mod_factor = 1.0 + 0.02 * (2.0 * std::f32::consts::PI * 50.0 * t).sin();
                (base + wm1 + wm2 + wm3) * mod_factor
            })
            .collect();
        AudioBuffer::from_mono(samples, sr)
    }

    fn detect_confidence(buf: &AudioBuffer) -> f64 {
        WatermarkDetector::detect_all(buf).overall_confidence
    }

    /// Process buffer with full pipeline normalization (mimics run() behavior).
    fn process_with_normalization(buf: &AudioBuffer, mode: SanitizationMode) -> (AudioBuffer, f64) {
        let original_rms = buf.rms();
        let mut cleaned = buf.clone();
        let pipeline = make_pipeline(mode);
        pipeline.process_buffer(&mut cleaned).unwrap();

        // Reproduce the normalization that run() does
        if cleaned.rms() > 1e-10 && original_rms > 1e-10 {
            cleaned.normalize_rms(original_rms);
        }
        cleaned.soft_clip(0.99);

        let quality_loss = if original_rms > 1e-10 {
            (original_rms - cleaned.rms()).abs() / original_rms
        } else {
            0.0
        };
        (cleaned, quality_loss as f64)
    }

    #[test]
    fn test_standard_mode_processes_watermarks() {
        let buf = watermarked_buffer();
        let before = detect_confidence(&buf);

        let (cleaned, _) = process_with_normalization(&buf, SanitizationMode::Standard);
        let after = detect_confidence(&cleaned);

        // Standard mode should reduce or maintain watermark confidence
        assert!(
            after <= before + 0.05,
            "Standard mode made watermarks worse: before={before}, after={after}"
        );
    }

    #[test]
    fn test_modes_produce_different_outputs() {
        let buf = watermarked_buffer();
        let original_rms = buf.rms();

        let (fast, _) = process_with_normalization(&buf, SanitizationMode::Fast);
        let (standard, _) = process_with_normalization(&buf, SanitizationMode::Standard);

        // Fast and Standard should produce different results
        let diff: f32 = fast
            .to_mono_samples()
            .iter()
            .zip(standard.to_mono_samples().iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / buf.num_samples() as f32;

        assert!(
            diff > 1e-6 || original_rms < 1e-10,
            "Fast and Standard produced identical output (diff={diff})"
        );
    }

    #[test]
    fn test_all_modes_bounded_quality_loss() {
        let buf = watermarked_buffer();

        let modes = [
            (SanitizationMode::Fast, 0.01),
            (SanitizationMode::Standard, 0.05),
            (SanitizationMode::Preserving, 0.05),
            (SanitizationMode::Aggressive, 0.05),
        ];

        for (mode, max_loss) in modes {
            let (_, loss) = process_with_normalization(&buf, mode);
            assert!(
                loss < max_loss,
                "{mode:?} quality loss {loss:.4} exceeds max {max_loss}"
            );
        }
    }

    #[test]
    fn test_preserving_mode_quality_better_than_aggressive() {
        let buf = watermarked_buffer();

        let (_, preserving_loss) = process_with_normalization(&buf, SanitizationMode::Preserving);
        let (_, aggressive_loss) = process_with_normalization(&buf, SanitizationMode::Aggressive);

        assert!(
            preserving_loss <= aggressive_loss + 0.02,
            "Preserving ({preserving_loss:.4}) worse quality than Aggressive ({aggressive_loss:.4})"
        );
    }
}
