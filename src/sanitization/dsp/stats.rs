//! Statistical analysis functions for audio signal processing.
//!
//! Provides descriptive statistics (mean, std dev, skewness, kurtosis),
//! spectral features (centroid, flatness, rolloff), and signal analysis
//! utilities (zero-crossing rate, RMS, autocorrelation, peak detection).

/// Compute the mean of a slice.
pub fn mean(data: &[f32]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.iter().map(|&x| x as f64).sum::<f64>() / data.len() as f64
}

/// Compute the standard deviation of a slice.
pub fn std_dev(data: &[f32]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let m = mean(data);
    let variance =
        data.iter().map(|&x| (x as f64 - m).powi(2)).sum::<f64>() / (data.len() - 1) as f64;
    variance.sqrt()
}

/// Compute skewness (third standardized moment).
pub fn skewness(data: &[f32]) -> f64 {
    let n = data.len() as f64;
    if n < 3.0 {
        return 0.0;
    }
    let m = mean(data);
    let s = std_dev(data);
    if s < 1e-10 {
        return 0.0;
    }
    let sum_cubed: f64 = data.iter().map(|&x| ((x as f64 - m) / s).powi(3)).sum();
    sum_cubed / n
}

/// Compute kurtosis (fourth standardized moment, excess kurtosis).
pub fn kurtosis(data: &[f32]) -> f64 {
    let n = data.len() as f64;
    if n < 4.0 {
        return 0.0;
    }
    let m = mean(data);
    let s = std_dev(data);
    if s < 1e-10 {
        return 0.0;
    }
    let sum_fourth: f64 = data.iter().map(|&x| ((x as f64 - m) / s).powi(4)).sum();
    sum_fourth / n - 3.0 // excess kurtosis (normal = 0)
}

/// Compute Shannon entropy from a probability distribution.
pub fn entropy(probabilities: &[f64]) -> f64 {
    -probabilities
        .iter()
        .filter(|&&p| p > 0.0)
        .map(|&p| p * p.ln())
        .sum::<f64>()
}

/// Compute a histogram of data values and return normalized probabilities.
pub fn histogram(data: &[f32], num_bins: usize) -> Vec<f64> {
    if data.is_empty() || num_bins == 0 {
        return vec![];
    }

    let min_val = data.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_val = data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = (max_val - min_val) as f64;

    if range < 1e-10 {
        let mut bins = vec![0.0; num_bins];
        bins[0] = 1.0;
        return bins;
    }

    let mut bins = vec![0u64; num_bins];
    let bin_width = range / num_bins as f64;

    for &val in data {
        let bin = ((val as f64 - min_val as f64) / bin_width) as usize;
        let bin = bin.min(num_bins - 1);
        bins[bin] += 1;
    }

    let total = data.len() as f64;
    bins.iter().map(|&count| count as f64 / total).collect()
}

/// Compute the spectral centroid (center of mass of the spectrum).
pub fn spectral_centroid(magnitude: &[f32]) -> f64 {
    let total: f64 = magnitude.iter().map(|&m| m as f64).sum();
    if total < 1e-10 {
        return 0.0;
    }
    let weighted: f64 = magnitude
        .iter()
        .enumerate()
        .map(|(i, &m)| i as f64 * m as f64)
        .sum();
    weighted / total
}

/// Compute spectral flatness (Wiener entropy). Range [0, 1].
/// 1 = white noise, 0 = pure tone.
pub fn spectral_flatness(magnitude: &[f32]) -> f64 {
    let n = magnitude.len() as f64;
    if n < 1.0 {
        return 0.0;
    }

    let arithmetic_mean: f64 = magnitude.iter().map(|&m| m as f64).sum::<f64>() / n;
    if arithmetic_mean < 1e-10 {
        return 0.0;
    }

    let log_sum: f64 = magnitude.iter().map(|&m| (m as f64 + 1e-10).ln()).sum();
    let geometric_mean = (log_sum / n).exp();

    geometric_mean / arithmetic_mean
}

/// Compute spectral rolloff: the frequency bin below which `percentile` fraction of energy lives.
pub fn spectral_rolloff(magnitude: &[f32], percentile: f64) -> usize {
    let total_energy: f64 = magnitude.iter().map(|&m| (m as f64).powi(2)).sum();
    let threshold = total_energy * percentile;

    let mut cumulative = 0.0;
    for (i, &m) in magnitude.iter().enumerate() {
        cumulative += (m as f64).powi(2);
        if cumulative >= threshold {
            return i;
        }
    }
    magnitude.len().saturating_sub(1)
}

/// Compute the zero-crossing rate of a signal.
pub fn zero_crossing_rate(signal: &[f32]) -> f64 {
    if signal.len() < 2 {
        return 0.0;
    }
    let crossings = signal
        .windows(2)
        .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
        .count();
    crossings as f64 / (signal.len() - 1) as f64
}

/// Compute the RMS energy of a signal.
pub fn rms_energy(signal: &[f32]) -> f64 {
    if signal.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = signal.iter().map(|&s| (s as f64).powi(2)).sum();
    (sum_sq / signal.len() as f64).sqrt()
}

/// Compute autocorrelation of a signal (full, unnormalized).
pub fn autocorrelation(signal: &[f32]) -> Vec<f64> {
    let n = signal.len();
    let mut result = vec![0.0; n];
    let m = mean(signal);

    for lag in 0..n {
        let mut sum = 0.0;
        for i in 0..n - lag {
            sum += (signal[i] as f64 - m) * (signal[i + lag] as f64 - m);
        }
        result[lag] = sum;
    }

    // Normalize by lag-0 value
    if result[0] > 1e-10 {
        let norm = result[0];
        for val in &mut result {
            *val /= norm;
        }
    }

    result
}

/// A detected peak with index and value.
#[derive(Debug, Clone)]
pub struct Peak {
    /// Position of the peak in the input data.
    pub index: usize,
    /// Amplitude of the peak.
    pub value: f64,
}

/// Find peaks in a signal that are above a height threshold and separated by minimum distance.
pub fn find_peaks(data: &[f64], min_height: f64, min_distance: usize) -> Vec<Peak> {
    let n = data.len();
    if n < 3 {
        return vec![];
    }

    let mut peaks = Vec::new();

    for i in 1..n - 1 {
        if data[i] > data[i - 1] && data[i] > data[i + 1] && data[i] >= min_height {
            peaks.push(Peak {
                index: i,
                value: data[i],
            });
        }
    }

    if min_distance <= 1 {
        return peaks;
    }

    // Filter by minimum distance: keep highest peaks when conflict
    let mut filtered = Vec::new();
    for peak in &peaks {
        if let Some(last) = filtered.last() {
            let last: &Peak = last;
            if peak.index - last.index < min_distance {
                // Keep the taller one
                if peak.value > last.value {
                    filtered.pop();
                    filtered.push(peak.clone());
                }
                continue;
            }
        }
        filtered.push(peak.clone());
    }

    filtered
}

/// Compute the Pearson correlation coefficient between two signals.
pub fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len().min(y.len()) as f64;
    if n < 2.0 {
        return 0.0;
    }

    let sum_x: f64 = x.iter().take(n as usize).sum();
    let sum_y: f64 = y.iter().take(n as usize).sum();
    let sum_xy: f64 = x.iter().zip(y).take(n as usize).map(|(a, b)| a * b).sum();
    let sum_x2: f64 = x.iter().take(n as usize).map(|a| a * a).sum();
    let sum_y2: f64 = y.iter().take(n as usize).map(|a| a * a).sum();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();

    if denominator > 1e-10 {
        numerator / denominator
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean() {
        assert!((mean(&[1.0, 2.0, 3.0, 4.0, 5.0]) - 3.0).abs() < 1e-10);
        assert_eq!(mean(&[]), 0.0);
    }

    #[test]
    fn test_std_dev() {
        let data: Vec<f32> = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let sd = std_dev(&data);
        assert!((sd - 2.138).abs() < 0.01);
        assert_eq!(std_dev(&[1.0]), 0.0);
    }

    #[test]
    fn test_skewness_symmetric() {
        let data: Vec<f32> = (-100..=100).map(|i| i as f32 / 100.0).collect();
        assert!(skewness(&data).abs() < 0.01);
    }

    #[test]
    fn test_kurtosis_uniform() {
        let data: Vec<f32> = (0..10000).map(|i| i as f32 / 10000.0).collect();
        let k = kurtosis(&data);
        assert!((k - (-1.2)).abs() < 0.1);
    }

    #[test]
    fn test_entropy() {
        let uniform = vec![0.25, 0.25, 0.25, 0.25];
        assert!((entropy(&uniform) - (4.0_f64).ln()).abs() < 1e-10);
        assert_eq!(entropy(&[1.0, 0.0, 0.0]), 0.0);
    }

    #[test]
    fn test_histogram() {
        let data: Vec<f32> = vec![0.0, 0.1, 0.2, 0.5, 0.9, 1.0];
        let hist = histogram(&data, 2);
        assert_eq!(hist.len(), 2);
        assert!((hist.iter().sum::<f64>() - 1.0).abs() < 1e-10);
        assert!(histogram(&[], 10).is_empty());
    }

    #[test]
    fn test_spectral_centroid() {
        assert!(spectral_centroid(&[1.0, 0.0, 0.0, 0.0]) < 0.01);
        assert!((spectral_centroid(&[0.0, 0.0, 0.0, 1.0]) - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_spectral_flatness() {
        let flat = vec![1.0_f32; 100];
        assert!((spectral_flatness(&flat) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_spectral_rolloff() {
        let mag = vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        assert!(spectral_rolloff(&mag, 0.85) <= 3);
    }

    #[test]
    fn test_zero_crossing_rate() {
        assert!((zero_crossing_rate(&[1.0, -1.0, 1.0, -1.0, 1.0]) - 1.0).abs() < 1e-10);
        assert_eq!(zero_crossing_rate(&[1.0, 1.0, 1.0]), 0.0);
    }

    #[test]
    fn test_rms_energy() {
        assert!((rms_energy(&[0.5_f32; 100]) - 0.5).abs() < 1e-6);
        assert_eq!(rms_energy(&[]), 0.0);
    }

    #[test]
    fn test_autocorrelation_lag0() {
        let signal: Vec<f32> = (0..100).map(|i| (i as f32 * 0.1).sin()).collect();
        let ac = autocorrelation(&signal);
        assert!((ac[0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_find_peaks() {
        let data = vec![0.0, 1.0, 0.0, 0.5, 0.0, 2.0, 0.0];
        assert_eq!(find_peaks(&data, 0.3, 1).len(), 3);
        assert!(find_peaks(&data, 0.3, 3).len() <= 2);
    }

    #[test]
    fn test_pearson_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        assert!((pearson_correlation(&x, &y) - 1.0).abs() < 1e-10);
        let y_neg = vec![10.0, 8.0, 6.0, 4.0, 2.0];
        assert!((pearson_correlation(&x, &y_neg) - -1.0).abs() < 1e-10);
    }
}
