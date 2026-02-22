use std::path::{Path, PathBuf};
use std::time::Instant;

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
    flags: AdvancedFlags,
    fp_config: FingerprintRemovalConfig,
    output_format: Option<AudioFormat>,
}

impl SanitizationPipeline {
    pub fn new(
        mode: SanitizationMode,
        paranoid: bool,
        flags: AdvancedFlags,
        fp_config: FingerprintRemovalConfig,
        output_format: Option<AudioFormat>,
    ) -> Self {
        Self {
            mode,
            paranoid,
            flags,
            fp_config,
            output_format,
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

    /// Run the sanitization pipeline.
    pub fn run(&self, input: &Path, output: &Path) -> Result<SanitizationResult> {
        let start = Instant::now();

        let metadata_removed = MetadataCleaner::strip_to(input, output)?;

        let (mut buffer, source_format) = audio::load_audio(output)?;
        let original_rms = buffer.rms();

        let (patterns_found, patterns_suppressed) = match self.mode {
            SanitizationMode::Fast => (0, 0),
            SanitizationMode::Standard => self.run_standard(&mut buffer)?,
            SanitizationMode::Preserving => self.run_preserving(&mut buffer)?,
            SanitizationMode::Aggressive => self.run_aggressive(&mut buffer)?,
        };

        // Paranoid multi-pass runs spectral cleaning extra times; fingerprint/stealth run once
        // to avoid compounding artifacts that would degrade perceptual quality.
        if self.paranoid && self.mode != SanitizationMode::Fast {
            for _pass in 0..2 {
                SpectralCleaner::clean(&mut buffer, self.paranoid, &self.flags)?;
            }
        }

        // Adaptive notch runs last — STFT operations in earlier passes would undo its work.
        let (mut patterns_found, mut patterns_suppressed) = (patterns_found, patterns_suppressed);
        if self.flags.adaptive_notch && self.mode != SanitizationMode::Fast {
            let notched = SpectralCleaner::adaptive_notch_pass(&mut buffer, self.paranoid)?;
            patterns_found += notched;
            patterns_suppressed += notched;
        }

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

        let out_format = self.output_format.unwrap_or(source_format);
        audio::save_audio(&buffer, output, out_format)?;

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

    fn run_standard(&self, buffer: &mut AudioBuffer) -> Result<(usize, usize)> {
        let (found, suppressed) = SpectralCleaner::clean(buffer, self.paranoid, &self.flags)?;
        FingerprintRemover::remove(buffer, self.paranoid, &self.fp_config)?;
        Ok((found, suppressed))
    }

    fn run_preserving(&self, buffer: &mut AudioBuffer) -> Result<(usize, usize)> {
        let (found, suppressed) = SpectralCleaner::clean(buffer, self.paranoid, &self.flags)?;
        FingerprintRemover::remove(buffer, self.paranoid, &self.fp_config)?;
        StealthOps::apply(buffer, &self.flags, self.paranoid)?;
        Ok((found, suppressed))
    }

    fn run_aggressive(&self, buffer: &mut AudioBuffer) -> Result<(usize, usize)> {
        let (found, suppressed) = SpectralCleaner::clean(buffer, true, &self.flags)?;
        FingerprintRemover::remove(buffer, true, &self.fp_config)?;
        StealthOps::apply(buffer, &self.flags, true)?;
        Ok((found, suppressed))
    }
}
