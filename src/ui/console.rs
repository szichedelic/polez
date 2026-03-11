//! Formatted console output for analysis results, sanitization reports,
//! and configuration display.

use colored::Colorize;
use rand::seq::SliceRandom;
use std::collections::HashMap;

/// Manages styled console output for all CLI display needs.
///
/// When `stderr_mode` is enabled (e.g. for `--json`), all output goes to
/// stderr so that only machine-readable JSON appears on stdout.
pub struct ConsoleManager {
    stderr_mode: bool,
}

impl ConsoleManager {
    /// Create a new console manager that prints to stdout.
    pub fn new() -> Self {
        Self { stderr_mode: false }
    }

    /// Create a console manager that prints to stderr (for JSON mode).
    pub fn stderr() -> Self {
        Self { stderr_mode: true }
    }

    /// Print a line to the correct output stream based on stderr_mode.
    fn out(&self, args: std::fmt::Arguments<'_>) {
        if self.stderr_mode {
            eprintln!("{args}");
        } else {
            println!("{args}");
        }
    }

    /// Print a green success message.
    pub fn success(&self, msg: &str) {
        self.out(format_args!("{}", msg.green().bold()));
    }

    /// Print a red error message.
    pub fn error(&self, msg: &str) {
        self.out(format_args!("{}", msg.red().bold()));
    }

    /// Print a yellow warning message.
    pub fn warning(&self, msg: &str) {
        self.out(format_args!("{}", msg.yellow().bold()));
    }

    /// Print a cyan informational message.
    pub fn info(&self, msg: &str) {
        self.out(format_args!("{}", msg.cyan().bold()));
    }

    /// Print a random themed quote in magenta italic.
    pub fn hacker_quote(&self) {
        let quotes = [
            "In the symphony of digital rights, we are the conductors of chaos.",
            "The best place to hide a dead body is page two of the search results.",
            "404 sanity not found. But the audio is clean!",
            "Debugging is like being a detective in a crime movie where you are also the murderer.",
            "The code works because I made it work. Don't question the magic.",
            "Life is short, sanitize everything.",
            "There's no place like 127.0.0.1... especially after sanitization.",
            "Why do programmers prefer dark mode? Because light attracts bugs!",
            "I would tell you a UDP joke, but you might not get it.",
            "The quieter you become, the more you can hear.",
            "Every byte tells a story. We just rewrote the ending.",
            "In a world of ones and zeros, we choose silence.",
            "The signal is clean. The noise was never here.",
            "What the algorithm giveth, the algorithm taketh away.",
            "Digital footprints washed away like sand at high tide.",
            "They can't trace what doesn't exist.",
            "Audio forensics hate this one weird trick.",
            "The best encryption is the one nobody knows about.",
            "We don't delete history. We rewrite it.",
            "Clean as a whistle. Literally.",
        ];
        let mut rng = rand::thread_rng();
        if let Some(quote) = quotes.choose(&mut rng) {
            self.out(format_args!(""));
            self.out(format_args!(
                "{}",
                format!("  \"{quote}\"").magenta().italic()
            ));
        }
    }

    /// Display formatted audio analysis results including watermark detection and AI probability.
    pub fn display_analysis(&self, analysis: &AnalysisDisplay) {
        self.out(format_args!(""));
        self.out(format_args!(
            "{}",
            "=== Audio Analysis Results ===".blue().bold()
        ));
        self.out(format_args!("  File:        {}", analysis.file_path));
        self.out(format_args!("  Format:      {}", analysis.format));
        self.out(format_args!(
            "  Duration:    {:.2}s",
            analysis.duration_secs
        ));
        self.out(format_args!("  Sample Rate: {} Hz", analysis.sample_rate));
        self.out(format_args!("  Channels:    {}", analysis.channels));
        self.out(format_args!(""));

        if analysis.metadata_tags > 0 {
            self.out(format_args!(
                "  Metadata Tags:      {}",
                format!("{}", analysis.metadata_tags).yellow()
            ));
        }
        if analysis.suspicious_chunks > 0 {
            self.out(format_args!(
                "  Suspicious Chunks:  {}",
                format!("{}", analysis.suspicious_chunks).red()
            ));
        }

        // Watermark detection results
        if let Some(ref watermarks) = analysis.watermark_results {
            self.out(format_args!(""));
            self.out(format_args!("  {}", "Watermark Detection:".cyan().bold()));
            for (method, detected, confidence) in watermarks {
                let status = if *detected {
                    format!("DETECTED (confidence: {confidence:.2})")
                        .red()
                        .bold()
                        .to_string()
                } else {
                    "not detected".dimmed().to_string()
                };
                self.out(format_args!(
                    "    {:<22} {}",
                    format!("{}:", method),
                    status
                ));
            }
        }

        // AI probability
        if let Some(ai_prob) = analysis.ai_probability {
            self.out(format_args!(""));
            let prob_str = format!("{:.0}%", ai_prob * 100.0);
            let colored_prob = if ai_prob > 0.7 {
                prob_str.red().bold().to_string()
            } else if ai_prob > 0.4 {
                prob_str.yellow().bold().to_string()
            } else {
                prob_str.green().bold().to_string()
            };
            self.out(format_args!(
                "  AI Probability:     {colored_prob} ({ai_prob:.2})"
            ));
        }

        let threat_str = format!("{}", analysis.threats_found);
        let level_str = &analysis.threat_level;
        let colored_level = match level_str.as_str() {
            "HIGH" => level_str.red().bold().to_string(),
            "MEDIUM" => level_str.yellow().bold().to_string(),
            _ => level_str.green().bold().to_string(),
        };
        self.out(format_args!(""));
        self.out(format_args!("  Threats Found:      {threat_str}"));
        self.out(format_args!("  Threat Level:       {colored_level}"));
        self.out(format_args!(""));
    }

    /// Display sanitization pipeline results (metadata removed, patterns suppressed, etc.).
    pub fn display_results(&self, results: &SanitizationDisplay) {
        self.out(format_args!(""));
        self.out(format_args!(
            "{}",
            "=== Sanitization Results ===".green().bold()
        ));
        let status = if results.success {
            "SUCCESS".green().bold().to_string()
        } else {
            "FAILED".red().bold().to_string()
        };
        self.out(format_args!("  Status:              {status}"));
        self.out(format_args!(
            "  Metadata Removed:    {}",
            results.metadata_removed
        ));
        self.out(format_args!(
            "  Patterns Found:      {}",
            results.patterns_found
        ));
        self.out(format_args!(
            "  Patterns Suppressed: {}",
            results.patterns_suppressed
        ));
        self.out(format_args!(
            "  Quality Loss:        {:.2}%",
            results.quality_loss
        ));
        self.out(format_args!(
            "  Processing Time:     {:.2}s",
            results.processing_time
        ));
        if let Some(ref path) = results.output_file {
            self.out(format_args!("  Output File:         {path}"));
        }
        self.out(format_args!(""));
    }

    /// Display before/after verification results with SNR, spectral similarity, and verdict.
    pub fn display_verification(&self, v: &VerificationDisplay) {
        self.out(format_args!(""));
        self.out(format_args!(
            "{}",
            "=== Verification Results ===".blue().bold()
        ));
        self.out(format_args!(
            "  Original Threats:      {}",
            v.original_threats
        ));
        self.out(format_args!(
            "  Remaining Threats:     {}",
            v.remaining_threats
        ));

        let eff = v.removal_effectiveness;
        let eff_str = format!("{eff:.1}%");
        let colored_eff = if eff >= 95.0 {
            eff_str.green().bold().to_string()
        } else if eff >= 80.0 {
            eff_str.yellow().bold().to_string()
        } else {
            eff_str.red().bold().to_string()
        };
        self.out(format_args!("  Removal Effectiveness: {colored_eff}"));
        self.out(format_args!(
            "  Hash Changed:          {}",
            if v.hash_different {
                "Yes".green().to_string()
            } else {
                "No".red().to_string()
            }
        ));

        // SNR and quality metrics
        if let Some(snr) = v.snr_db {
            let snr_str = format!("{snr:.1} dB");
            let colored_snr = if snr > 40.0 {
                snr_str.green().to_string()
            } else if snr > 20.0 {
                snr_str.yellow().to_string()
            } else {
                snr_str.red().to_string()
            };
            self.out(format_args!("  SNR:                   {colored_snr}"));
        }

        if let Some(sim) = v.spectral_similarity {
            self.out(format_args!("  Spectral Similarity:   {sim:.4}"));
        }

        if let Some(quality) = v.quality_score {
            let q_str = format!("{quality:.3}");
            let colored_q = if quality > 0.9 {
                q_str.green().to_string()
            } else if quality > 0.7 {
                q_str.yellow().to_string()
            } else {
                q_str.red().to_string()
            };
            self.out(format_args!("  Quality Preservation:  {colored_q}"));
        }

        // Verdict
        if let Some((ref verdict_text, ref verdict_color)) = v.verdict {
            let colored_verdict = match verdict_color.as_str() {
                "green" => verdict_text.green().bold().to_string(),
                "yellow" => verdict_text.yellow().bold().to_string(),
                "red" => verdict_text.red().bold().to_string(),
                _ => verdict_text.to_string(),
            };
            self.out(format_args!(""));
            self.out(format_args!("  Verdict: {colored_verdict}"));
        }

        self.out(format_args!(""));
    }

    /// Display configuration as a list of key-value pairs.
    pub fn display_config(&self, items: &[(String, String)]) {
        self.out(format_args!(""));
        self.out(format_args!("{}", "=== Configuration ===".blue().bold()));
        for (key, value) in items {
            self.out(format_args!("  {key:<30} {value}"));
        }
        self.out(format_args!(""));
    }

    /// Display configuration from a HashMap, sorted by key.
    pub fn display_config_map(&self, config: &HashMap<String, String>) {
        self.out(format_args!(""));
        self.out(format_args!("{}", "=== Configuration ===".blue().bold()));
        let mut keys: Vec<_> = config.keys().collect();
        keys.sort();
        for key in keys {
            self.out(format_args!("  {:<30} {}", key, config[key]));
        }
        self.out(format_args!(""));
    }

    /// Display batch processing summary with success/failure counts and timing.
    pub fn display_batch_summary(&self, summary: &BatchSummaryDisplay) {
        self.out(format_args!(""));
        self.out(format_args!(
            "{}",
            "=== Batch Processing Summary ===".green().bold()
        ));
        self.out(format_args!("  Total Files:     {}", summary.total));
        self.out(format_args!(
            "  Successful:      {}",
            format!("{}", summary.success).green()
        ));
        self.out(format_args!(
            "  Failed:          {}",
            if summary.failed > 0 {
                format!("{}", summary.failed).red().to_string()
            } else {
                "0".to_string()
            }
        ));
        self.out(format_args!(
            "  Total Time:      {:.1}s",
            summary.total_time
        ));
        if summary.total > 0 {
            self.out(format_args!(
                "  Avg per File:    {:.2}s",
                summary.total_time / summary.total as f64
            ));
        }
        if let Some(ref dir) = summary.output_dir {
            self.out(format_args!("  Output Dir:      {dir}"));
        }

        if !summary.failed_files.is_empty() {
            self.out(format_args!(""));
            self.out(format_args!("  {}", "Failed Files:".red().bold()));
            for (file, error) in &summary.failed_files {
                self.out(format_args!("    - {file}: {error}"));
            }
        }
        self.out(format_args!(""));
    }
}

// Display data structs

/// Data for rendering audio analysis results in the console.
pub struct AnalysisDisplay {
    /// Path to the analyzed file.
    pub file_path: String,
    /// Audio format (e.g. "WAV", "MP3").
    pub format: String,
    /// Total duration in seconds.
    pub duration_secs: f64,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of audio channels.
    pub channels: usize,
    /// Number of metadata tags found.
    pub metadata_tags: usize,
    /// Number of suspicious binary chunks.
    pub suspicious_chunks: usize,
    /// Total threats detected.
    pub threats_found: usize,
    /// Threat severity label (e.g. "HIGH", "MEDIUM", "LOW").
    pub threat_level: String,
    /// Per-method watermark results: (method name, detected, confidence).
    pub watermark_results: Option<Vec<(String, bool, f64)>>,
    /// Estimated probability the audio is AI-generated.
    pub ai_probability: Option<f64>,
}

/// Data for rendering sanitization results in the console.
pub struct SanitizationDisplay {
    /// Whether sanitization completed without error.
    pub success: bool,
    /// Number of metadata tags removed.
    pub metadata_removed: usize,
    /// Number of watermark patterns detected.
    pub patterns_found: usize,
    /// Number of patterns successfully suppressed.
    pub patterns_suppressed: usize,
    /// Estimated quality loss as a percentage.
    pub quality_loss: f64,
    /// Wall-clock processing time in seconds.
    pub processing_time: f64,
    /// Path to the sanitized output file.
    pub output_file: Option<String>,
}

/// Data for rendering before/after verification results.
pub struct VerificationDisplay {
    /// Threats detected in the original file.
    pub original_threats: usize,
    /// Threats remaining in the cleaned file.
    pub remaining_threats: usize,
    /// Percentage of threats removed.
    pub removal_effectiveness: f64,
    /// Whether the file hash changed after cleaning.
    pub hash_different: bool,
    /// Signal-to-noise ratio in dB (original vs. cleaned).
    pub snr_db: Option<f64>,
    /// Pearson correlation of FFT magnitudes.
    pub spectral_similarity: Option<f64>,
    /// Combined quality preservation score (0.0 - 1.0).
    pub quality_score: Option<f64>,
    /// Verdict text and color name (e.g. ("EXCELLENT", "green")).
    pub verdict: Option<(String, String)>,
}

/// Data for rendering batch processing summary.
pub struct BatchSummaryDisplay {
    /// Total files processed.
    pub total: usize,
    /// Number of successfully processed files.
    pub success: usize,
    /// Number of files that failed.
    pub failed: usize,
    /// Total wall-clock time in seconds.
    pub total_time: f64,
    /// Output directory path.
    pub output_dir: Option<String>,
    /// List of (filename, error message) for failed files.
    pub failed_files: Vec<(String, String)>,
}
