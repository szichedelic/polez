use colored::Colorize;
use rand::seq::SliceRandom;
use std::collections::HashMap;

pub struct ConsoleManager;

impl ConsoleManager {
    pub fn new() -> Self {
        Self
    }

    pub fn success(&self, msg: &str) {
        println!("{}", msg.green().bold());
    }

    pub fn error(&self, msg: &str) {
        println!("{}", msg.red().bold());
    }

    pub fn warning(&self, msg: &str) {
        println!("{}", msg.yellow().bold());
    }

    pub fn info(&self, msg: &str) {
        println!("{}", msg.cyan().bold());
    }

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
            println!("\n{}", format!("  \"{quote}\"").magenta().italic());
        }
    }

    pub fn display_analysis(&self, analysis: &AnalysisDisplay) {
        println!();
        println!("{}", "=== Audio Analysis Results ===".blue().bold());
        println!("  File:        {}", analysis.file_path);
        println!("  Format:      {}", analysis.format);
        println!("  Duration:    {:.2}s", analysis.duration_secs);
        println!("  Sample Rate: {} Hz", analysis.sample_rate);
        println!("  Channels:    {}", analysis.channels);
        println!();

        if analysis.metadata_tags > 0 {
            println!(
                "  Metadata Tags:      {}",
                format!("{}", analysis.metadata_tags).yellow()
            );
        }
        if analysis.suspicious_chunks > 0 {
            println!(
                "  Suspicious Chunks:  {}",
                format!("{}", analysis.suspicious_chunks).red()
            );
        }

        // Watermark detection results
        if let Some(ref watermarks) = analysis.watermark_results {
            println!();
            println!("  {}", "Watermark Detection:".cyan().bold());
            for (method, detected, confidence) in watermarks {
                let status = if *detected {
                    format!("DETECTED (confidence: {confidence:.2})")
                        .red()
                        .bold()
                        .to_string()
                } else {
                    "not detected".dimmed().to_string()
                };
                println!("    {:<22} {}", format!("{}:", method), status);
            }
        }

        // AI probability
        if let Some(ai_prob) = analysis.ai_probability {
            println!();
            let prob_str = format!("{:.0}%", ai_prob * 100.0);
            let colored_prob = if ai_prob > 0.7 {
                prob_str.red().bold().to_string()
            } else if ai_prob > 0.4 {
                prob_str.yellow().bold().to_string()
            } else {
                prob_str.green().bold().to_string()
            };
            println!("  AI Probability:     {colored_prob} ({ai_prob:.2})");
        }

        let threat_str = format!("{}", analysis.threats_found);
        let level_str = &analysis.threat_level;
        let colored_level = match level_str.as_str() {
            "HIGH" => level_str.red().bold().to_string(),
            "MEDIUM" => level_str.yellow().bold().to_string(),
            _ => level_str.green().bold().to_string(),
        };
        println!();
        println!("  Threats Found:      {threat_str}");
        println!("  Threat Level:       {colored_level}");
        println!();
    }

    pub fn display_results(&self, results: &SanitizationDisplay) {
        println!();
        println!("{}", "=== Sanitization Results ===".green().bold());
        let status = if results.success {
            "SUCCESS".green().bold().to_string()
        } else {
            "FAILED".red().bold().to_string()
        };
        println!("  Status:              {status}");
        println!("  Metadata Removed:    {}", results.metadata_removed);
        println!("  Patterns Found:      {}", results.patterns_found);
        println!("  Patterns Suppressed: {}", results.patterns_suppressed);
        println!("  Quality Loss:        {:.2}%", results.quality_loss);
        println!("  Processing Time:     {:.2}s", results.processing_time);
        if let Some(ref path) = results.output_file {
            println!("  Output File:         {path}");
        }
        println!();
    }

    pub fn display_verification(&self, v: &VerificationDisplay) {
        println!();
        println!("{}", "=== Verification Results ===".blue().bold());
        println!("  Original Threats:      {}", v.original_threats);
        println!("  Remaining Threats:     {}", v.remaining_threats);

        let eff = v.removal_effectiveness;
        let eff_str = format!("{eff:.1}%");
        let colored_eff = if eff >= 95.0 {
            eff_str.green().bold().to_string()
        } else if eff >= 80.0 {
            eff_str.yellow().bold().to_string()
        } else {
            eff_str.red().bold().to_string()
        };
        println!("  Removal Effectiveness: {colored_eff}");
        println!(
            "  Hash Changed:          {}",
            if v.hash_different {
                "Yes".green().to_string()
            } else {
                "No".red().to_string()
            }
        );

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
            println!("  SNR:                   {colored_snr}");
        }

        if let Some(sim) = v.spectral_similarity {
            println!("  Spectral Similarity:   {sim:.4}");
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
            println!("  Quality Preservation:  {colored_q}");
        }

        // Verdict
        if let Some((ref verdict_text, ref verdict_color)) = v.verdict {
            let colored_verdict = match verdict_color.as_str() {
                "green" => verdict_text.green().bold().to_string(),
                "yellow" => verdict_text.yellow().bold().to_string(),
                "red" => verdict_text.red().bold().to_string(),
                _ => verdict_text.to_string(),
            };
            println!();
            println!("  Verdict: {colored_verdict}");
        }

        println!();
    }

    pub fn display_config(&self, items: &[(String, String)]) {
        println!();
        println!("{}", "=== Configuration ===".blue().bold());
        for (key, value) in items {
            println!("  {key:<30} {value}");
        }
        println!();
    }

    pub fn display_config_map(&self, config: &HashMap<String, String>) {
        println!();
        println!("{}", "=== Configuration ===".blue().bold());
        let mut keys: Vec<_> = config.keys().collect();
        keys.sort();
        for key in keys {
            println!("  {:<30} {}", key, config[key]);
        }
        println!();
    }

    pub fn display_batch_summary(&self, summary: &BatchSummaryDisplay) {
        println!();
        println!("{}", "=== Batch Processing Summary ===".green().bold());
        println!("  Total Files:     {}", summary.total);
        println!(
            "  Successful:      {}",
            format!("{}", summary.success).green()
        );
        println!(
            "  Failed:          {}",
            if summary.failed > 0 {
                format!("{}", summary.failed).red().to_string()
            } else {
                "0".to_string()
            }
        );
        println!("  Total Time:      {:.1}s", summary.total_time);
        if summary.total > 0 {
            println!(
                "  Avg per File:    {:.2}s",
                summary.total_time / summary.total as f64
            );
        }
        if let Some(ref dir) = summary.output_dir {
            println!("  Output Dir:      {dir}");
        }

        if !summary.failed_files.is_empty() {
            println!();
            println!("  {}", "Failed Files:".red().bold());
            for (file, error) in &summary.failed_files {
                println!("    - {file}: {error}");
            }
        }
        println!();
    }
}

// Display data structs

pub struct AnalysisDisplay {
    pub file_path: String,
    pub format: String,
    pub duration_secs: f64,
    pub sample_rate: u32,
    pub channels: usize,
    pub metadata_tags: usize,
    pub suspicious_chunks: usize,
    pub threats_found: usize,
    pub threat_level: String,
    pub watermark_results: Option<Vec<(String, bool, f64)>>,
    pub ai_probability: Option<f64>,
}

pub struct SanitizationDisplay {
    pub success: bool,
    pub metadata_removed: usize,
    pub patterns_found: usize,
    pub patterns_suppressed: usize,
    pub quality_loss: f64,
    pub processing_time: f64,
    pub output_file: Option<String>,
}

pub struct VerificationDisplay {
    pub original_threats: usize,
    pub remaining_threats: usize,
    pub removal_effectiveness: f64,
    pub hash_different: bool,
    pub snr_db: Option<f64>,
    pub spectral_similarity: Option<f64>,
    pub quality_score: Option<f64>,
    pub verdict: Option<(String, String)>,
}

pub struct BatchSummaryDisplay {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub total_time: f64,
    pub output_dir: Option<String>,
    pub failed_files: Vec<(String, String)>,
}
