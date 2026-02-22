use std::path::Path;

use sha2::{Digest, Sha256};

use crate::audio;
use crate::detection::{MetadataScanner, WatermarkDetector};
use crate::error::Result;
use crate::sanitization::dsp::stats;
use crate::sanitization::dsp::stft;

/// Verification result comparing before/after sanitization.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub original_threats: usize,
    pub remaining_threats: usize,
    pub removal_effectiveness: f64,
    pub hash_different: bool,
    pub original_hash: String,
    pub cleaned_hash: String,
    pub snr_db: f64,
    pub spectral_similarity: f64,
    pub quality_score: f64,
}

/// Verify sanitization by comparing original and cleaned files.
pub fn verify(original: &Path, cleaned: &Path) -> Result<VerificationResult> {
    // 1. File hashes
    let orig_hash = file_hash(original)?;
    let clean_hash = file_hash(cleaned)?;

    // 2. Metadata scan on both
    let orig_scan = MetadataScanner::scan(original)?;
    let clean_scan = MetadataScanner::scan(cleaned)?;

    // 3. Load audio from both
    let (orig_audio, _) = audio::load_audio(original)?;
    let (clean_audio, _) = audio::load_audio(cleaned)?;

    // 4. Run watermark detection on both
    let orig_watermarks = WatermarkDetector::detect_all(&orig_audio);
    let clean_watermarks = WatermarkDetector::detect_all(&clean_audio);

    // 5. Count threats
    let original_threats =
        orig_scan.tags.len() + orig_scan.suspicious_chunks.len() + orig_watermarks.watermark_count;

    let remaining_threats = clean_scan.tags.len()
        + clean_scan.suspicious_chunks.len()
        + clean_watermarks.watermark_count;

    // 6. Calculate effectiveness
    let effectiveness = if original_threats > 0 {
        ((original_threats.saturating_sub(remaining_threats)) as f64 / original_threats as f64)
            * 100.0
    } else {
        100.0
    };

    // 7. Calculate SNR
    let orig_mono = orig_audio.to_mono();
    let clean_mono = clean_audio.to_mono();
    let orig_ch: Vec<f32> = orig_mono.channel(0).to_vec();
    let clean_ch: Vec<f32> = clean_mono.channel(0).to_vec();
    let snr_db = calculate_snr(&orig_ch, &clean_ch);

    // 8. Calculate spectral similarity
    let spectral_similarity = calculate_spectral_similarity(&orig_ch, &clean_ch);

    // 9. Quality preservation score
    let quality_score = quality_preservation_score(snr_db, spectral_similarity);

    Ok(VerificationResult {
        original_threats,
        remaining_threats,
        removal_effectiveness: effectiveness,
        hash_different: orig_hash != clean_hash,
        original_hash: orig_hash,
        cleaned_hash: clean_hash,
        snr_db,
        spectral_similarity,
        quality_score,
    })
}

/// Calculate Signal-to-Noise Ratio between original and cleaned audio.
fn calculate_snr(original: &[f32], cleaned: &[f32]) -> f64 {
    let min_len = original.len().min(cleaned.len());
    if min_len == 0 {
        return 0.0;
    }

    let mut signal_power = 0.0_f64;
    let mut noise_power = 0.0_f64;

    for i in 0..min_len {
        let s = original[i] as f64;
        let n = (original[i] - cleaned[i]) as f64;
        signal_power += s * s;
        noise_power += n * n;
    }

    signal_power /= min_len as f64;
    noise_power /= min_len as f64;

    if noise_power > 1e-20 {
        10.0 * (signal_power / noise_power).log10()
    } else {
        f64::INFINITY
    }
}

/// Calculate spectral similarity via Pearson correlation of FFT magnitudes.
fn calculate_spectral_similarity(original: &[f32], cleaned: &[f32]) -> f64 {
    let min_len = original.len().min(cleaned.len());
    if min_len < 64 {
        return 0.0;
    }

    let orig_fft = stft::real_fft(&original[..min_len]);
    let clean_fft = stft::real_fft(&cleaned[..min_len]);

    let orig_mag: Vec<f64> = orig_fft.iter().map(|c| c.norm() as f64).collect();
    let clean_mag: Vec<f64> = clean_fft.iter().map(|c| c.norm() as f64).collect();

    stats::pearson_correlation(&orig_mag, &clean_mag)
}

/// Compute a quality preservation score from SNR and spectral similarity.
pub fn quality_preservation_score(snr_db: f64, spectral_similarity: f64) -> f64 {
    let snr_score = if snr_db > 40.0 {
        1.0
    } else if snr_db > 20.0 {
        0.8
    } else if snr_db > 10.0 {
        0.6
    } else {
        0.4
    };

    snr_score * (1.0 + spectral_similarity) / 2.0
}

/// Determine verdict text and color based on effectiveness and SNR.
pub fn verdict(effectiveness: f64, snr_db: f64) -> (&'static str, &'static str) {
    match (effectiveness, snr_db) {
        (e, s) if e > 90.0 && s > 30.0 => ("EXCELLENT", "green"),
        (e, s) if e > 70.0 && s > 20.0 => ("GOOD", "yellow"),
        (e, s) if e > 50.0 && s > 10.0 => ("ACCEPTABLE", "yellow"),
        _ => ("POOR", "red"),
    }
}

fn file_hash(path: &Path) -> Result<String> {
    let data = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(format!("{:x}", hasher.finalize()))
}
