pub mod metadata_scan;
pub mod polez;
pub mod statistical;
pub mod watermark;

pub use metadata_scan::{MetadataScanResult, MetadataScanner};
pub use polez::{PolezDetectionResult, PolezDetector};
pub use statistical::{StatisticalAnalyzer, StatisticalResult};
pub use watermark::{WatermarkDetector, WatermarkResult};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioBuffer;
    use std::f32::consts::PI;

    /// Generate a clean sine wave (should have no watermarks).
    fn clean_sine(freq: f32, sr: u32, duration_secs: f32) -> AudioBuffer {
        let len = (sr as f32 * duration_secs) as usize;
        let data: Vec<f32> = (0..len)
            .map(|i| 0.5 * (2.0 * PI * freq * i as f32 / sr as f32).sin())
            .collect();
        AudioBuffer::from_mono(data, sr)
    }

    /// Generate a multi-frequency signal (more realistic audio).
    fn multi_freq_signal(sr: u32, duration_secs: f32) -> AudioBuffer {
        let len = (sr as f32 * duration_secs) as usize;
        let data: Vec<f32> = (0..len)
            .map(|i| {
                let t = i as f32 / sr as f32;
                0.2 * (2.0 * PI * 440.0 * t).sin()
                    + 0.15 * (2.0 * PI * 880.0 * t).sin()
                    + 0.1 * (2.0 * PI * 1760.0 * t).sin()
                    + 0.05 * (2.0 * PI * 3520.0 * t).sin()
            })
            .collect();
        AudioBuffer::from_mono(data, sr)
    }

    #[test]
    fn test_watermark_detector_clean_signal() {
        let buf = multi_freq_signal(44100, 1.0);
        let result = WatermarkDetector::detect_all(&buf);
        assert!(result.overall_confidence < 0.8);
    }

    #[test]
    fn test_watermark_detector_short_signal() {
        let buf = AudioBuffer::from_mono(vec![0.0; 100], 44100);
        let result = WatermarkDetector::detect_all(&buf);
        assert_eq!(result.watermark_count, 0);
        assert_eq!(result.overall_confidence, 0.0);
    }

    #[test]
    fn test_watermark_result_has_methods() {
        let buf = multi_freq_signal(44100, 1.0);
        let result = WatermarkDetector::detect_all(&buf);
        // Should have results for all 6 detection methods
        assert!(result.method_results.len() >= 5);
    }

    #[test]
    fn test_statistical_analyzer_clean() {
        let buf = multi_freq_signal(44100, 1.0);
        let result = StatisticalAnalyzer::analyze(&buf);
        // Clean synthetic audio should not score extremely high AI probability
        assert!(result.ai_probability < 0.95);
        assert!(!result.features.is_empty());
    }

    #[test]
    fn test_statistical_features_computed() {
        let buf = multi_freq_signal(44100, 1.0);
        let result = StatisticalAnalyzer::analyze(&buf);
        assert!(result.features.contains_key("mean"));
        assert!(result.features.contains_key("std"));
        assert!(result.features.contains_key("rms_energy"));
    }

    #[test]
    fn test_statistical_temporal_analysis() {
        let buf = multi_freq_signal(44100, 1.0);
        let result = StatisticalAnalyzer::analyze(&buf);
        assert!(result.temporal.temporal_entropy >= 0.0);
    }

    #[test]
    fn test_polez_detector_clean() {
        let buf = multi_freq_signal(48000, 1.0);
        let result = PolezDetector::detect(&buf);
        assert!(result.detection_probability >= 0.0);
        assert!(result.detection_probability <= 1.0);
        assert!(result.confidence >= 0.0);
        assert!(!result.verdict.is_empty());
    }

    #[test]
    fn test_polez_signals_valid_ranges() {
        let buf = multi_freq_signal(48000, 1.0);
        let result = PolezDetector::detect(&buf);
        assert!(result.signals.ultrasonic_score >= 0.0);
        assert!(result.signals.bit_plane_score >= 0.0);
        assert!(result.signals.autocorr_score >= 0.0);
    }

    #[test]
    fn test_detection_stereo() {
        let mono = multi_freq_signal(44100, 1.0);
        let interleaved: Vec<f32> = mono
            .to_mono_samples()
            .iter()
            .flat_map(|&s| [s, s * 0.8])
            .collect();
        let stereo = AudioBuffer::from_interleaved(&interleaved, 2, 44100);
        // Should not panic on stereo input
        let _ = WatermarkDetector::detect_all(&stereo);
        let _ = StatisticalAnalyzer::analyze(&stereo);
        let _ = PolezDetector::detect(&stereo);
    }
}
