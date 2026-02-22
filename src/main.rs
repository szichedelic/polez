#![allow(dead_code)]

mod audio;
mod cli;
mod config;
mod detection;
mod error;
#[cfg(feature = "gui")]
mod gui;
mod inspect;
mod sanitization;
mod ui;
mod verification;

use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use clap::Parser;

use cli::{Cli, Commands, ConfigAction, FormatChoice};
use config::ConfigManager;
use detection::{MetadataScanner, StatisticalAnalyzer, WatermarkDetector};
use sanitization::SanitizationPipeline;
use ui::banners::BannerManager;
use ui::console::{
    AnalysisDisplay, BatchSummaryDisplay, ConsoleManager, SanitizationDisplay, VerificationDisplay,
};

fn main() {
    let cli = Cli::parse();
    let console = ConsoleManager::new();
    let banner = BannerManager::new();

    banner.show_main_banner();

    console.warning("LEGAL DISCLAIMER: This tool is for AUTHORIZED SECURITY RESEARCH ONLY");
    console.info("  Use only on files you own or have explicit permission to modify");
    console.info("  You are responsible for compliance with applicable laws");
    println!();

    if let Err(e) = run_command(cli.command, &console, &banner) {
        console.error(&format!("CRITICAL ERROR: {e}"));
        process::exit(1);
    }
}

fn run_command(
    cmd: Commands,
    console: &ConsoleManager,
    banner: &BannerManager,
) -> error::Result<()> {
    match cmd {
        Commands::Clean {
            input_file,
            output,
            paranoid,
            verify,
            backup,
            format,
            flags,
            fp_flags,
        } => cmd_clean(
            &input_file,
            output.as_deref(),
            paranoid,
            verify,
            backup,
            format,
            flags.into(),
            fp_flags.into(),
            console,
            banner,
        ),
        Commands::Sweep {
            directory,
            output_dir,
            extension,
            paranoid,
            workers,
            backup,
            recursive,
            dry_run,
            fp_flags,
        } => cmd_sweep(
            &directory,
            output_dir.as_deref(),
            &extension,
            paranoid,
            workers,
            backup,
            recursive,
            dry_run,
            fp_flags.into(),
            console,
            banner,
        ),
        Commands::Detect { input_file, deep } => cmd_detect(&input_file, deep, console),
        Commands::Benchmark {
            directory,
            output,
            recursive,
            extension,
        } => cmd_benchmark(&directory, &output, recursive, &extension, console),
        Commands::Inspect {
            input_file,
            start,
            duration,
            freq_min,
            freq_max,
        } => cmd_inspect(&input_file, start, duration, freq_min, freq_max, console),
        Commands::Bits {
            input_file,
            bit,
            offset,
            count,
            search,
        } => cmd_bits(&input_file, bit, offset, count, search, console),
        #[cfg(feature = "gui")]
        Commands::Gui { port, no_open } => cmd_gui(port, no_open, console),
        Commands::Config { action } => cmd_config(action, console),
        Commands::Version => {
            BannerManager::new().show_version_info();
            Ok(())
        }
    }
}

#[cfg(feature = "gui")]
fn cmd_gui(port: u16, no_open: bool, console: &ConsoleManager) -> error::Result<()> {
    console.info(&format!("Starting Polez GUI on http://localhost:{port}"));

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| error::PolezError::Other(anyhow::anyhow!("Runtime error: {e}")))?;

    rt.block_on(async { gui::start_server(port, no_open).await })
        .map_err(|e| error::PolezError::Other(anyhow::anyhow!("Server error: {e}")))
}

#[allow(clippy::too_many_arguments)]
fn cmd_clean(
    input_file: &Path,
    output: Option<&Path>,
    paranoid: bool,
    verify: bool,
    backup: bool,
    format: FormatChoice,
    flags: config::AdvancedFlags,
    fp_config: config::FingerprintRemovalConfig,
    console: &ConsoleManager,
    banner: &BannerManager,
) -> error::Result<()> {
    if !input_file.exists() {
        return Err(error::PolezError::FileNotFound(input_file.to_path_buf()));
    }

    console.success(&format!("Scanning: {}", input_file.display()));

    let config_mgr = ConfigManager::new()?;

    let output_path = match output {
        Some(p) => p.to_path_buf(),
        None => generate_output_path(
            input_file,
            &config_mgr.config.batch_processing.naming_pattern,
        ),
    };

    let out_format = match format {
        FormatChoice::Preserve => None,
        FormatChoice::Mp3 => Some(audio::AudioFormat::Mp3),
        FormatChoice::Wav => Some(audio::AudioFormat::Wav),
    };

    if backup {
        let backup_path = input_file.with_extension(format!(
            "{}.bak",
            input_file
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("bak")
        ));
        std::fs::copy(input_file, &backup_path)?;
        console.info(&format!("Backup created: {}", backup_path.display()));
    }

    console.info("Scanning for digital footprints...");
    let scan_result = MetadataScanner::scan(input_file)?;
    let (audio_buf, src_format) = audio::load_audio(input_file)?;

    let watermark_result = WatermarkDetector::detect_all(&audio_buf);
    let watermark_display: Vec<(String, bool, f64)> = watermark_result
        .method_results
        .iter()
        .map(|(name, mr)| (name.clone(), mr.detected, mr.confidence))
        .collect();

    let threats_found = scan_result.tags.len()
        + scan_result.suspicious_chunks.len()
        + watermark_result.watermark_count;

    let threat_level = if threats_found > 10 {
        "HIGH"
    } else if threats_found > 5 {
        "MEDIUM"
    } else {
        "LOW"
    };

    console.display_analysis(&AnalysisDisplay {
        file_path: input_file.display().to_string(),
        format: src_format.to_string(),
        duration_secs: audio_buf.duration_secs(),
        sample_rate: audio_buf.sample_rate,
        channels: audio_buf.num_channels(),
        metadata_tags: scan_result.tags.len(),
        suspicious_chunks: scan_result.suspicious_chunks.len(),
        threats_found,
        threat_level: threat_level.to_string(),
        watermark_results: Some(watermark_display),
        ai_probability: None,
    });

    if threats_found > 0 {
        console.error(&format!(
            "Found {threats_found} threats... time to eliminate them!"
        ));
    } else {
        console.warning("No obvious threats detected... but we'll clean it anyway!");
    }

    let mode = SanitizationPipeline::mode_from_config(&config_mgr.config);
    let pipeline = SanitizationPipeline::new(mode, paranoid, flags, fp_config, out_format);

    banner.show_processing_banner();
    let spinner = ui::progress::stage_spinner("Sanitizing audio...");

    let result = pipeline.run(input_file, &output_path)?;

    spinner.finish_and_clear();

    console.display_results(&SanitizationDisplay {
        success: result.success,
        metadata_removed: result.metadata_removed,
        patterns_found: result.patterns_found,
        patterns_suppressed: result.patterns_suppressed,
        quality_loss: result.quality_loss,
        processing_time: result.processing_time,
        output_file: Some(result.output_file.display().to_string()),
    });

    if verify && result.success {
        console.info("Verification phase: Double-checking our work...");
        let v = verification::verify(input_file, &output_path)?;
        let (verdict_text, verdict_color) =
            verification::verdict(v.removal_effectiveness, v.snr_db);

        console.display_verification(&VerificationDisplay {
            original_threats: v.original_threats,
            remaining_threats: v.remaining_threats,
            removal_effectiveness: v.removal_effectiveness,
            hash_different: v.hash_different,
            snr_db: Some(v.snr_db),
            spectral_similarity: Some(v.spectral_similarity),
            quality_score: Some(v.quality_score),
            verdict: Some((verdict_text.to_string(), verdict_color.to_string())),
        });
    }

    if result.success {
        banner.show_success_banner();
        console.success("File sanitized successfully.");
        console.hacker_quote();
    } else {
        console.error("Sanitization failed!");
        process::exit(1);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_sweep(
    directory: &Path,
    output_dir: Option<&Path>,
    extensions: &[String],
    paranoid: bool,
    workers: u32,
    backup: bool,
    recursive: bool,
    dry_run: bool,
    fp_config: config::FingerprintRemovalConfig,
    console: &ConsoleManager,
    banner: &BannerManager,
) -> error::Result<()> {
    let start = Instant::now();

    console.success(&format!(
        "Batch processing initiated: {}",
        directory.display()
    ));
    console.info(&format!("Extensions: {}", extensions.join(", ")));
    console.info(&format!(
        "Workers: {} | Paranoid: {} | Recursive: {}",
        workers,
        if paranoid { "ON" } else { "OFF" },
        if recursive { "ON" } else { "OFF" },
    ));

    let walker = walkdir::WalkDir::new(directory);
    let walker = if recursive {
        walker
    } else {
        walker.max_depth(1)
    };

    let files: Vec<PathBuf> = walker
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| extensions.iter().any(|x| x.eq_ignore_ascii_case(ext)))
                .unwrap_or(false)
        })
        .map(|e| e.into_path())
        .collect();

    if files.is_empty() {
        console.warning("No audio files found in directory");
        return Ok(());
    }

    console.success(&format!("Found {} files to process", files.len()));

    if dry_run {
        console.info("DRY RUN - No files will be processed:");
        for file in &files {
            console.info(&format!("  Would process: {}", file.display()));
        }
        console.success(&format!("{} files would be processed", files.len()));
        return Ok(());
    }

    let out_dir = match output_dir {
        Some(d) => {
            std::fs::create_dir_all(d)?;
            d.to_path_buf()
        }
        None => {
            let d = directory.join("polez_cleaned");
            std::fs::create_dir_all(&d)?;
            d
        }
    };

    let config_mgr = ConfigManager::new()?;
    let mode = SanitizationPipeline::mode_from_config(&config_mgr.config);
    let flags = config_mgr.config.advanced_flags.clone();

    let pb = ui::progress::batch_progress(files.len() as u64);

    use rayon::prelude::*;

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers as usize)
        .build()
        .map_err(|e| error::PolezError::Other(anyhow::anyhow!("Thread pool error: {e}")))?;

    let results: Vec<(PathBuf, std::result::Result<(), String>)> = pool.install(|| {
        files
            .par_iter()
            .map(|file| {
                let relative = file.strip_prefix(directory).unwrap_or(file.as_path());
                let output_file = out_dir.join(relative);

                if let Some(parent) = output_file.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                if backup {
                    let bak = file.with_extension(format!(
                        "{}.bak",
                        file.extension().and_then(|e| e.to_str()).unwrap_or("bak")
                    ));
                    let _ = std::fs::copy(file, &bak);
                }

                let pipeline = SanitizationPipeline::new(
                    mode,
                    paranoid,
                    flags.clone(),
                    fp_config.clone(),
                    None,
                );
                let result = pipeline
                    .run(file, &output_file)
                    .map(|_| ())
                    .map_err(|e| e.to_string());

                pb.inc(1);
                (file.clone(), result)
            })
            .collect()
    });

    pb.finish_and_clear();

    let mut success_count = 0;
    let mut fail_count = 0;
    let mut failed_files = Vec::new();

    for (file, result) in &results {
        match result {
            Ok(()) => {
                success_count += 1;
            }
            Err(e) => {
                fail_count += 1;
                failed_files.push((file.display().to_string(), e.clone()));
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f64();

    console.display_batch_summary(&BatchSummaryDisplay {
        total: files.len(),
        success: success_count,
        failed: fail_count,
        total_time: elapsed,
        output_dir: Some(out_dir.display().to_string()),
        failed_files,
    });

    banner.show_batch_complete_banner(success_count);

    Ok(())
}

fn cmd_detect(input_file: &Path, deep: bool, console: &ConsoleManager) -> error::Result<()> {
    if !input_file.exists() {
        return Err(error::PolezError::FileNotFound(input_file.to_path_buf()));
    }

    console.info(&format!("Forensic analysis: {}", input_file.display()));
    console.info("Scanning for digital footprints...");

    let scan_result = MetadataScanner::scan(input_file)?;
    let (audio_buf, src_format) = audio::load_audio(input_file)?;

    let watermark_result = WatermarkDetector::detect_all(&audio_buf);
    let watermark_display: Vec<(String, bool, f64)> = watermark_result
        .method_results
        .iter()
        .map(|(name, mr)| (name.clone(), mr.detected, mr.confidence))
        .collect();

    let polez_result = detection::PolezDetector::detect(&audio_buf);

    let (ai_probability, anomaly_count) = if deep {
        let stat_result = StatisticalAnalyzer::analyze(&audio_buf);
        (
            Some(stat_result.ai_probability),
            stat_result.anomalies.len(),
        )
    } else {
        (None, 0)
    };

    let threats_found = scan_result.tags.len()
        + scan_result.suspicious_chunks.len()
        + watermark_result.watermark_count
        + anomaly_count;

    let threat_level = if threats_found > 10 {
        "HIGH"
    } else if threats_found > 5 {
        "MEDIUM"
    } else {
        "LOW"
    };

    console.display_analysis(&AnalysisDisplay {
        file_path: input_file.display().to_string(),
        format: src_format.to_string(),
        duration_secs: audio_buf.duration_secs(),
        sample_rate: audio_buf.sample_rate,
        channels: audio_buf.num_channels(),
        metadata_tags: scan_result.tags.len(),
        suspicious_chunks: scan_result.suspicious_chunks.len(),
        threats_found,
        threat_level: threat_level.to_string(),
        watermark_results: Some(watermark_display),
        ai_probability,
    });

    display_polez_results(&polez_result, console);

    match threat_level {
        "HIGH" => console.error("HIGH THREAT LEVEL - This file is heavily watermarked!"),
        "MEDIUM" => console.warning("MEDIUM THREAT LEVEL - Some traces detected"),
        _ => console.success("LOW THREAT LEVEL - Relatively clean"),
    }

    Ok(())
}

fn cmd_benchmark(
    directory: &Path,
    output: &Path,
    recursive: bool,
    extensions: &[String],
    console: &ConsoleManager,
) -> error::Result<()> {
    use std::io::Write;

    console.info(&format!("Scanning directory: {}", directory.display()));

    let walker = walkdir::WalkDir::new(directory);
    let walker = if recursive {
        walker
    } else {
        walker.max_depth(1)
    };

    let files: Vec<PathBuf> = walker
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| extensions.iter().any(|x| x.eq_ignore_ascii_case(ext)))
                .unwrap_or(false)
        })
        .map(|e| e.into_path())
        .collect();

    if files.is_empty() {
        console.warning("No audio files found");
        return Ok(());
    }

    console.success(&format!("Found {} files to analyze", files.len()));

    let mut csv = std::fs::File::create(output)?;
    writeln!(
        csv,
        "file,duration_secs,sample_rate,detection_probability,confidence,ultrasonic_ratio,ultrasonic_score,bit_plane_bias,biased_planes,bit_plane_score,max_autocorr,autocorr_period,autocorr_score,verdict"
    )?;

    let pb = ui::progress::batch_progress(files.len() as u64);

    for file in &files {
        let result = analyze_file_for_benchmark(file);
        match result {
            Ok(row) => {
                writeln!(csv, "{row}")?;
            }
            Err(e) => {
                writeln!(
                    csv,
                    "{},ERROR,,,,,,,,,,,,,{}",
                    file.display(),
                    e.to_string().replace(',', ";")
                )?;
            }
        }
        pb.inc(1);
    }

    pb.finish_and_clear();

    console.success(&format!("Results written to: {}", output.display()));
    console.info("Open in Excel/Google Sheets to analyze the dataset");

    Ok(())
}

fn analyze_file_for_benchmark(file: &Path) -> error::Result<String> {
    let (audio_buf, _) = audio::load_audio(file)?;
    let polez_result = detection::PolezDetector::detect(&audio_buf);

    let s = &polez_result.signals;
    Ok(format!(
        "{},{:.2},{},{:.4},{:.4},{:.6},{:.4},{:.6},{},{:.4},{:.6},{},{:.4},\"{}\"",
        file.display(),
        audio_buf.duration_secs(),
        audio_buf.sample_rate,
        polez_result.detection_probability,
        polez_result.confidence,
        s.ultrasonic_ratio,
        s.ultrasonic_score,
        s.bit_plane_bias,
        s.biased_planes,
        s.bit_plane_score,
        s.max_autocorr,
        s.autocorr_period,
        s.autocorr_score,
        polez_result.verdict
    ))
}

fn display_polez_results(result: &detection::PolezDetectionResult, _console: &ConsoleManager) {
    use colored::Colorize;

    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════════╗"
            .magenta()
    );
    println!(
        "{}",
        "║  POLEZ AI WATERMARK DETECTION                                                    ║"
            .magenta()
    );
    println!(
        "{}",
        "╠══════════════════════════════════════════════════════════════════════════════════╣"
            .magenta()
    );

    let prob_pct = result.detection_probability * 100.0;
    let bar_len = (prob_pct / 100.0 * 50.0) as usize;
    let bar = "█".repeat(bar_len);
    let empty = "░".repeat(50 - bar_len);

    let prob_colored = if prob_pct > 70.0 {
        format!("{prob_pct:5.1}%").red().bold()
    } else if prob_pct > 40.0 {
        format!("{prob_pct:5.1}%").yellow()
    } else {
        format!("{prob_pct:5.1}%").green()
    };

    println!(
        "{}",
        format!("║  Detection Probability: {bar}{empty} {prob_colored}             ║").magenta()
    );
    println!(
        "{}",
        format!(
            "║  Confidence: {:.0}%                                                               ║",
            result.confidence * 100.0
        )
        .magenta()
    );

    println!(
        "{}",
        "╠══════════════════════════════════════════════════════════════════════════════════╣"
            .magenta()
    );
    println!(
        "{}",
        "║  Detection Signals:                                                              ║"
            .magenta()
    );

    let s = &result.signals;

    let ultra_indicator = if s.ultrasonic_score > 0.5 {
        "▲ HIGH".red()
    } else {
        "▽ low".green()
    };
    println!(
        "{}",
        format!(
            "║    Ultrasonic (23-24kHz): ratio={:.4}  score={:.2}  {}                    ║",
            s.ultrasonic_ratio, s.ultrasonic_score, ultra_indicator
        )
        .magenta()
    );

    let bits_indicator = if s.biased_planes >= 6 {
        "▲ HIGH".red()
    } else if s.biased_planes >= 3 {
        "- MED".yellow()
    } else {
        "▽ low".green()
    };
    println!(
        "{}",
        format!(
            "║    Bit Plane Bias: {}/8 planes biased, avg={:.4}  score={:.2}  {}         ║",
            s.biased_planes, s.bit_plane_bias, s.bit_plane_score, bits_indicator
        )
        .magenta()
    );

    let auto_indicator = if s.autocorr_score > 0.5 {
        "▲ HIGH".red()
    } else {
        "▽ low".green()
    };
    println!(
        "{}",
        format!(
            "║    Autocorrelation: period={}, strength={:.4}  score={:.2}  {}            ║",
            s.autocorr_period, s.max_autocorr, s.autocorr_score, auto_indicator
        )
        .magenta()
    );

    println!(
        "{}",
        "╠══════════════════════════════════════════════════════════════════════════════════╣"
            .magenta()
    );

    // Pad to fixed width before coloring to avoid ANSI escape sequences expanding the box
    let verdict_padded = format!("{:<60}", result.verdict);
    let verdict_display = if result.detection_probability > 0.7 {
        verdict_padded.red().bold()
    } else if result.detection_probability > 0.4 {
        verdict_padded.yellow()
    } else {
        verdict_padded.green()
    };
    println!(
        "{}",
        format!("║  Verdict: {verdict_display}         ║").magenta()
    );

    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════════╝"
            .magenta()
    );
}

fn cmd_inspect(
    input_file: &Path,
    start: f64,
    duration: f64,
    freq_min: u32,
    freq_max: u32,
    console: &ConsoleManager,
) -> error::Result<()> {
    if !input_file.exists() {
        return Err(error::PolezError::FileNotFound(input_file.to_path_buf()));
    }

    console.info(&format!("Loading: {}", input_file.display()));
    let (audio_buf, _) = audio::load_audio(input_file)?;

    console.info(&format!(
        "Generating spectrogram view ({}-{} kHz, {:.1}s-{:.1}s)...",
        freq_min / 1000,
        freq_max / 1000,
        start,
        start + duration
    ));

    let view = inspect::SpectrogramView::new(freq_min, freq_max, start, duration);
    view.render(&audio_buf)?;

    Ok(())
}

fn cmd_bits(
    input_file: &Path,
    bit: u8,
    offset: usize,
    count: usize,
    search: bool,
    console: &ConsoleManager,
) -> error::Result<()> {
    if !input_file.exists() {
        return Err(error::PolezError::FileNotFound(input_file.to_path_buf()));
    }

    console.info(&format!("Loading: {}", input_file.display()));
    let (audio_buf, _) = audio::load_audio(input_file)?;

    console.info(&format!(
        "Analyzing bit plane {} (samples {}-{})...",
        bit,
        offset,
        offset + count
    ));

    let view = inspect::BitsView::new(bit, offset, count, search);
    view.render(&audio_buf)?;

    Ok(())
}

fn cmd_config(action: Option<ConfigAction>, console: &ConsoleManager) -> error::Result<()> {
    match action {
        None | Some(ConfigAction::Show) => {
            let config_mgr = ConfigManager::new()?;
            let yaml = serde_yaml::to_string(&config_mgr.config)
                .map_err(|e| error::PolezError::Config(format!("Serialize error: {e}")))?;

            console.info("Current Configuration:");
            println!("{yaml}");
            Ok(())
        }
        Some(ConfigAction::Preset { name }) => {
            let mut config_mgr = ConfigManager::new()?;
            config_mgr.apply_preset(&name)?;
            console.success(&format!("Applied preset: {name}"));
            Ok(())
        }
        Some(ConfigAction::List) => {
            let config_mgr = ConfigManager::new()?;

            console.info("Available Presets:");
            println!();
            console.info("Built-in Presets:");
            for preset in config::defaults::builtin_presets() {
                println!("  - {}: {}", preset.name, preset.description);
            }

            let custom = config_mgr.list_custom_presets();
            if custom.is_empty() {
                println!();
                console.info("No custom presets. Create one with: polez config create <name>");
            } else {
                println!();
                console.info("Custom Presets:");
                for name in custom {
                    println!("  - {name}");
                }
            }
            Ok(())
        }
        Some(ConfigAction::Create {
            name,
            paranoid,
            quality,
            format,
            backup,
            verify,
        }) => {
            let config_mgr = ConfigManager::new()?;
            let mut preset_config = config::defaults::default_config();
            preset_config.paranoia_level = match paranoid {
                cli::ParanoiaChoice::Low => config::ParanoiaLevel::Low,
                cli::ParanoiaChoice::Medium => config::ParanoiaLevel::Medium,
                cli::ParanoiaChoice::High => config::ParanoiaLevel::High,
                cli::ParanoiaChoice::Maximum => config::ParanoiaLevel::Maximum,
            };
            preset_config.preserve_quality = match quality {
                cli::QualityChoice::Low => config::QualityLevel::Low,
                cli::QualityChoice::Medium => config::QualityLevel::Medium,
                cli::QualityChoice::High => config::QualityLevel::High,
                cli::QualityChoice::Maximum => config::QualityLevel::Maximum,
            };
            preset_config.output_format = match format {
                FormatChoice::Preserve => config::OutputFormat::Preserve,
                FormatChoice::Mp3 => config::OutputFormat::Mp3,
                FormatChoice::Wav => config::OutputFormat::Wav,
            };
            preset_config.backup_originals = backup;
            preset_config.verification.auto_verify = verify;

            config_mgr.create_preset(&name, &preset_config)?;
            console.success(&format!("Created preset: {name}"));
            console.info(&format!("  Apply with: polez config preset {name}"));
            Ok(())
        }
        Some(ConfigAction::Delete { name }) => {
            let config_mgr = ConfigManager::new()?;
            config_mgr.delete_preset(&name)?;
            console.success(&format!("Deleted preset: {name}"));
            Ok(())
        }
        Some(ConfigAction::Reset) => {
            let mut config_mgr = ConfigManager::new()?;
            config_mgr.reset_to_defaults()?;
            console.success("Configuration reset to defaults");
            Ok(())
        }
    }
}

fn generate_output_path(input: &Path, pattern: &str) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ext = input.extension().and_then(|s| s.to_str()).unwrap_or("wav");

    let name = pattern
        .replace("{name}", stem)
        .replace("{ext}", &format!(".{ext}"));

    input.with_file_name(name)
}
