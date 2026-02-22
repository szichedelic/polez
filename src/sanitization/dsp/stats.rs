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
    pub index: usize,
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
