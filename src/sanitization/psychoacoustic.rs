use num_complex::Complex;

/// Simplified psychoacoustic masking model based on ISO 11172-3 / MPEG-1 Layer III.
///
/// Computes a masking threshold per frequency bin from the signal's own spectral
/// content. Components below this threshold are perceptually inaudible and can be
/// modified without audible artifacts.
pub struct MaskingModel {
    sample_rate: f64,
    fft_size: usize,
}

/// Absolute threshold of hearing in dB SPL, sampled at bark-band center frequencies.
/// Approximation of the ISO 226 equal-loudness contour at threshold.
const ATH_FREQS: [(f64, f64); 18] = [
    (50.0, 37.0),
    (100.0, 25.0),
    (200.0, 11.0),
    (400.0, 4.0),
    (630.0, 2.0),
    (800.0, 1.0),
    (1000.0, 0.0),
    (1270.0, -1.0),
    (1600.0, -1.5),
    (2000.0, -2.0),
    (2500.0, -3.0),
    (3150.0, -4.5),
    (4000.0, -5.0),
    (5000.0, -3.0),
    (6300.0, 0.0),
    (8000.0, 5.0),
    (10000.0, 15.0),
    (16000.0, 40.0),
];

impl MaskingModel {
    pub fn new(sample_rate: f64, fft_size: usize) -> Self {
        Self {
            sample_rate,
            fft_size,
        }
    }

    /// Compute a masking threshold for each frequency bin of a spectrogram frame.
    ///
    /// Returns a vector of threshold magnitudes (linear scale). If a bin's magnitude
    /// is below its threshold, that component is inaudible and safe to modify.
    pub fn compute_threshold(&self, frame: &[Complex<f32>]) -> Vec<f32> {
        let num_bins = frame.len();
        let freq_resolution = self.sample_rate / self.fft_size as f64;

        // Convert frame to power spectrum in dB
        let power_db: Vec<f64> = frame
            .iter()
            .map(|c| {
                let mag = c.norm() as f64;
                if mag > 1e-20 {
                    20.0 * mag.log10()
                } else {
                    -400.0
                }
            })
            .collect();

        let mut threshold_db = vec![-200.0_f64; num_bins];

        // 1. Absolute threshold of hearing
        for (bin, thresh) in threshold_db.iter_mut().enumerate().take(num_bins) {
            let freq = bin as f64 * freq_resolution;
            *thresh = interpolate_ath(freq);
        }

        // 2. Simultaneous masking from tonal components
        // Find local spectral peaks (potential maskers)
        let maskers = find_maskers(&power_db, freq_resolution);

        // 3. For each masker, compute its masking spread and raise the threshold
        for &(masker_bin, masker_db) in &maskers {
            let masker_freq = masker_bin as f64 * freq_resolution;
            let masker_bark = hz_to_bark(masker_freq);

            for (bin, thresh) in threshold_db.iter_mut().enumerate().take(num_bins) {
                let bin_freq = bin as f64 * freq_resolution;
                let bin_bark = hz_to_bark(bin_freq);
                let delta_bark = bin_bark - masker_bark;

                // Spreading function (simplified from ISO 11172-3)
                let spread = spreading_function(delta_bark, masker_db);

                // Masking offset: tonal maskers provide less masking than noise
                // Typical offset is 14.5 + bark for tonal, 5.5 for noise-like
                let offset = 14.5 + masker_bark;
                let masked_level = masker_db - offset + spread;

                if masked_level > *thresh {
                    *thresh = masked_level;
                }
            }
        }

        // Convert thresholds back to linear magnitude
        threshold_db
            .iter()
            .map(|&db| {
                let linear = 10.0_f64.powf(db / 20.0);
                linear as f32
            })
            .collect()
    }

    /// Check if a bin's magnitude is below the masking threshold (inaudible).
    pub fn is_masked(magnitude: f32, threshold: f32) -> bool {
        magnitude <= threshold
    }
}

/// Convert frequency in Hz to Bark scale (Zwicker & Terhardt approximation).
fn hz_to_bark(freq: f64) -> f64 {
    13.0 * (0.00076 * freq).atan() + 3.5 * (freq / 7500.0).powi(2).atan()
}

/// Interpolate absolute threshold of hearing at a given frequency.
fn interpolate_ath(freq: f64) -> f64 {
    if freq <= ATH_FREQS[0].0 {
        return ATH_FREQS[0].1;
    }
    if freq >= ATH_FREQS[ATH_FREQS.len() - 1].0 {
        // Above 16kHz, threshold rises steeply
        let last = ATH_FREQS[ATH_FREQS.len() - 1];
        return last.1 + (freq - last.0) * 0.005;
    }

    for i in 0..ATH_FREQS.len() - 1 {
        let (f0, db0) = ATH_FREQS[i];
        let (f1, db1) = ATH_FREQS[i + 1];
        if freq >= f0 && freq < f1 {
            let t = (freq - f0) / (f1 - f0);
            return db0 + t * (db1 - db0);
        }
    }

    ATH_FREQS[ATH_FREQS.len() - 1].1
}

/// Simplified spreading function in Bark domain.
///
/// Models how a masker at one frequency raises the hearing threshold at
/// neighboring frequencies. Asymmetric: masking spreads further upward
/// in frequency than downward.
fn spreading_function(delta_bark: f64, masker_level: f64) -> f64 {
    if delta_bark < -3.0 {
        // Far below masker: steep lower slope
        -17.0 * (delta_bark + 3.0)
    } else if delta_bark < -1.0 {
        // Near below masker
        (delta_bark + 1.0) * (-6.0)
    } else if delta_bark <= 0.0 {
        // Just below masker
        0.0
    } else if delta_bark <= 1.0 {
        // Just above masker
        -1.5 * delta_bark
    } else if delta_bark <= 8.0 {
        // Upper slope depends on masker level (louder maskers spread further)
        let slope = -27.0 + 0.37 * masker_level.clamp(-20.0, 80.0);
        slope * (delta_bark - 1.0) - 1.5
    } else {
        -200.0
    }
}

/// Find spectral peaks that act as maskers.
///
/// A peak must be a local maximum and above a minimum level to qualify.
fn find_maskers(power_db: &[f64], freq_resolution: f64) -> Vec<(usize, f64)> {
    let mut maskers = Vec::new();
    let min_masker_level = -60.0; // Only consider peaks above -60 dB

    for bin in 2..power_db.len().saturating_sub(2) {
        let freq = bin as f64 * freq_resolution;
        // Skip DC and near-Nyquist
        if freq < 20.0 {
            continue;
        }

        let val = power_db[bin];
        if val < min_masker_level {
            continue;
        }

        // Local maximum check (within ±2 bins)
        if val > power_db[bin - 1]
            && val > power_db[bin + 1]
            && val > power_db[bin - 2]
            && val > power_db[bin + 2]
        {
            // Must be significantly above neighbors (at least 7dB)
            let neighbor_avg =
                (power_db[bin - 2] + power_db[bin - 1] + power_db[bin + 1] + power_db[bin + 2])
                    / 4.0;
            if val - neighbor_avg > 7.0 {
                maskers.push((bin, val));
            }
        }
    }

    maskers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hz_to_bark() {
        // 1kHz should be around bark 8.5
        let bark = hz_to_bark(1000.0);
        assert!(bark > 7.0 && bark < 10.0, "1kHz bark = {bark}");
    }

    #[test]
    fn test_ath_interpolation() {
        // At 1kHz, ATH should be ~0 dB
        let ath = interpolate_ath(1000.0);
        assert!((ath - 0.0).abs() < 1.0, "ATH at 1kHz = {ath}");

        // At 50Hz, should be high (~37 dB)
        let ath_low = interpolate_ath(50.0);
        assert!(ath_low > 30.0, "ATH at 50Hz = {ath_low}");
    }

    #[test]
    fn test_spreading_symmetric() {
        // Spreading at masker position should be 0
        let spread = spreading_function(0.0, 60.0);
        assert!((spread - 0.0).abs() < 0.01, "Spread at 0 bark = {spread}");

        // Spreading above masker should be negative
        let spread_above = spreading_function(2.0, 60.0);
        assert!(spread_above < 0.0, "Spread above masker = {spread_above}");
    }

    #[test]
    fn test_threshold_shape() {
        let model = MaskingModel::new(44100.0, 2048);

        // Create a frame with a strong 1kHz tone
        let freq_res = 44100.0 / 2048.0;
        let tone_bin = (1000.0 / freq_res) as usize;
        let num_bins = 1025;

        let mut frame = vec![Complex::new(0.001_f32, 0.0); num_bins];
        frame[tone_bin] = Complex::new(1.0, 0.0); // Strong tone

        let threshold = model.compute_threshold(&frame);

        // Threshold near the tone should be elevated
        assert!(threshold.len() == num_bins);
        // The threshold at the tone's neighbors should be higher than far away
        let near_tone = threshold[tone_bin + 2];
        let far_away = threshold[(5000.0 / freq_res) as usize];
        assert!(
            near_tone > far_away,
            "Near-tone threshold ({near_tone}) should be > far-away ({far_away})"
        );
    }

    #[test]
    fn test_is_masked() {
        assert!(MaskingModel::is_masked(0.01, 0.05));
        assert!(!MaskingModel::is_masked(0.1, 0.05));
    }
}
