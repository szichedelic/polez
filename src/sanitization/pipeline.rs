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
    pub success: bool,
    pub output_file: PathBuf,
    pub metadata_removed: usize,
    pub patterns_found: usize,
    pub patterns_suppressed: usize,
    pub quality_loss: f64,
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

    fn run_standard(&self, buffer: &mut AudioBuffer) -> Result<(usize, usize)> {
        let (found, suppressed) =
            SpectralCleaner::clean(buffer, self.paranoid, &self.flags, &self.freq_ranges)?;
        FingerprintRemover::remove(buffer, self.paranoid, &self.fp_config)?;
        Ok((found, suppressed))
    }

    fn run_preserving(&self, buffer: &mut AudioBuffer) -> Result<(usize, usize)> {
        let (found, suppressed) =
            SpectralCleaner::clean(buffer, self.paranoid, &self.flags, &self.freq_ranges)?;
        FingerprintRemover::remove(buffer, self.paranoid, &self.fp_config)?;
        StealthOps::apply(buffer, &self.flags, self.paranoid)?;
        Ok((found, suppressed))
    }

    fn run_aggressive(&self, buffer: &mut AudioBuffer) -> Result<(usize, usize)> {
        let (found, suppressed) =
            SpectralCleaner::clean(buffer, true, &self.flags, &self.freq_ranges)?;
        FingerprintRemover::remove(buffer, true, &self.fp_config)?;
        StealthOps::apply(buffer, &self.flags, true)?;
        Ok((found, suppressed))
    }
}
