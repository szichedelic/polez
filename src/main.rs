#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use clap::Parser;
use serde::Serialize;
use sha2::{Digest, Sha256};

use polez::audio;
use polez::cli;
use polez::cli::{Cli, Commands, ConfigAction, FormatChoice};
use polez::config;
use polez::config::ConfigManager;
use polez::detection;
use polez::detection::{MetadataScanner, StatisticalAnalyzer, WatermarkDetector};
use polez::error;
#[cfg(feature = "gui")]
use polez::gui;
use polez::inspect;
use polez::sanitization::pipeline::SanitizationMode;
use polez::sanitization::SanitizationPipeline;
use polez::ui;
use polez::ui::banners::BannerManager;
use polez::ui::console::{
    AnalysisDisplay, BatchSummaryDisplay, ConsoleManager, SanitizationDisplay, VerificationDisplay,
};
use polez::verification;

#[derive(Serialize)]
struct JsonReport {
    file_path: String,
    format: String,
    duration_secs: f64,
    sample_rate: u32,
    channels: usize,
    watermark: detection::WatermarkResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<detection::MetadataScanResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    statistical: Option<detection::StatisticalResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    polez: Option<detection::PolezDetectionResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sanitization: Option<SanitizationReport>,
}

#[derive(Serialize)]
struct SanitizationReport {
    success: bool,
    metadata_removed: usize,
    patterns_found: usize,
    patterns_suppressed: usize,
    quality_loss: f64,
    processing_time: f64,
    output_file: String,
}

fn print_json<T: Serialize>(value: &T) -> error::Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| error::PolezError::Other(anyhow::anyhow!("JSON serialization error: {e}")))?;
    println!("{json}");
    Ok(())
}

fn write_json_report(report: &JsonReport, path: &Path) -> error::Result<()> {
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| error::PolezError::Other(anyhow::anyhow!("JSON serialization error: {e}")))?;
    std::fs::write(path, json)?;
    Ok(())
}

#[derive(Serialize)]
struct AuditEntry {
    timestamp: String,
    input_hash: String,
    output_hash: String,
    mode: String,
    paranoid: bool,
    flags: config::AdvancedFlags,
    metadata_removed: usize,
    patterns_found: usize,
    patterns_suppressed: usize,
    quality_loss: f64,
    processing_time: f64,
    success: bool,
}

fn file_sha256(path: &Path) -> error::Result<String> {
    let data = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(format!("{:x}", hasher.finalize()))
}

fn write_audit_entry(entry: &AuditEntry, path: &Path) -> error::Result<()> {
    use std::io::Write;
    let json = serde_json::to_string(entry)
        .map_err(|e| error::PolezError::Other(anyhow::anyhow!("Audit serialization error: {e}")))?;
    let is_new = !path.exists();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    #[cfg(unix)]
    if is_new {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    writeln!(file, "{json}")?;
    Ok(())
}

fn unix_timestamp_iso8601() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Simple epoch-to-date conversion
    let mut y = 1970i64;
    let mut remaining_days = days as i64;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0;
    for &md in &month_days {
        if remaining_days < md {
            break;
        }
        remaining_days -= md;
        m += 1;
    }
    format!(
        "{y:04}-{:02}-{:02}T{hours:02}:{minutes:02}:{seconds:02}Z",
        m + 1,
        remaining_days + 1
    )
}

fn main() {
    let cli = Cli::parse();
    let json_mode = cli.json;
    let quiet_mode = cli.quiet || json_mode;

    let log_level = if cli.verbose >= 2 {
        tracing::Level::TRACE
    } else if cli.verbose == 1 {
        tracing::Level::DEBUG
    } else if quiet_mode {
        tracing::Level::ERROR
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(log_level.into()),
        )
        .init();

    let console = if json_mode {
        ConsoleManager::stderr()
    } else {
        ConsoleManager::new()
    };
    let banner = BannerManager::new();

    if !quiet_mode {
        banner.show_main_banner();
        console.warning("LEGAL DISCLAIMER: This tool is for AUTHORIZED SECURITY RESEARCH ONLY");
        console.info("  Use only on files you own or have explicit permission to modify");
        console.info("  You are responsible for compliance with applicable laws");
        println!();
    }

    if let Err(e) = run_command(cli.command, json_mode, &console, &banner) {
        if json_mode {
            let err = serde_json::json!({"error": e.to_string()});
            println!("{}", serde_json::to_string_pretty(&err).unwrap_or_default());
        } else {
            console.error(&format!("CRITICAL ERROR: {e}"));
        }
        process::exit(1);
    }
}

fn run_command(
    cmd: Commands,
    json_mode: bool,
    console: &ConsoleManager,
    banner: &BannerManager,
) -> error::Result<()> {
    match cmd {
        Commands::Clean {
            input_file,
            output,
            paranoid,
            paranoid_passes,
            quality,
            verify,
            backup,
            dry_run,
            format,
            report,
            audit_log,
            sample_rate,
            bit_depth,
            naming,
            freq_range,
            flags,
            fp_flags,
        } => cmd_clean(
            &input_file,
            output.as_deref(),
            paranoid,
            paranoid_passes,
            quality,
            verify,
            backup,
            dry_run,
            report.as_deref(),
            audit_log.as_deref(),
            format,
            sample_rate,
            bit_depth,
            naming.as_deref(),
            freq_range,
            flags.into(),
            fp_flags.into(),
            json_mode,
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
            format,
            naming,
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
            format,
            naming.as_deref(),
            fp_flags.into(),
            json_mode,
            console,
            banner,
        ),
        Commands::Detect {
            input_file,
            deep,
            report,
            filter,
        } => cmd_detect(
            &input_file,
            deep,
            report.as_deref(),
            filter,
            json_mode,
            console,
        ),
        Commands::Benchmark {
            directory,
            output,
            recursive,
            extension,
        } => cmd_benchmark(&directory, &output, recursive, &extension, console),
        Commands::Inspect {
            input_file,
            output,
            start,
            duration,
            freq_min,
            freq_max,
        } => cmd_inspect(
            &input_file,
            output.as_deref(),
            start,
            duration,
            freq_min,
            freq_max,
            json_mode,
            console,
        ),
        Commands::Bits {
            input_file,
            bit,
            offset,
            count,
            search,
        } => cmd_bits(&input_file, bit, offset, count, search, json_mode, console),
        #[cfg(feature = "gui")]
        Commands::Gui { port, no_open } => cmd_gui(port, no_open, console),
        Commands::Fingerprint { files } => cmd_fingerprint(&files, json_mode, console),
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
    paranoid_passes: u32,
    quality: Option<u32>,
    verify: bool,
    backup: bool,
    dry_run: bool,
    report: Option<&Path>,
    audit_log: Option<&Path>,
    format: FormatChoice,
    target_sample_rate: Option<u32>,
    bit_depth: Option<u16>,
    naming: Option<&str>,
    freq_ranges: Vec<(f64, f64)>,
    flags: config::AdvancedFlags,
    fp_config: config::FingerprintRemovalConfig,
    json_mode: bool,
    console: &ConsoleManager,
    banner: &BannerManager,
) -> error::Result<()> {
    if !input_file.exists() {
        return Err(error::PolezError::FileNotFound(input_file.to_path_buf()));
    }

    if let Some(sr) = target_sample_rate {
        const VALID_RATES: &[u32] = &[
            8000, 11025, 16000, 22050, 44100, 48000, 88200, 96000, 176400, 192000,
        ];
        if !VALID_RATES.contains(&sr) {
            return Err(error::PolezError::AudioIo(format!(
                "Invalid sample rate {sr} Hz. Supported: {}",
                VALID_RATES
                    .iter()
                    .map(|r| r.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }

    if let Some(bd) = bit_depth {
        if !matches!(bd, 16 | 24 | 32) {
            return Err(error::PolezError::AudioIo(format!(
                "Invalid bit depth {bd}. Supported: 16, 24, 32"
            )));
        }
    }

    if !json_mode {
        console.success(&format!("Scanning: {}", input_file.display()));
    }

    let config_mgr = ConfigManager::new()?;

    let naming_pattern = naming
        .map(|s| s.to_string())
        .unwrap_or_else(|| config_mgr.config.batch_processing.naming_pattern.clone());
    validate_naming_template(&naming_pattern)?;

    let mode_for_naming = match quality {
        Some(q) => quality_to_mode(q),
        None => SanitizationPipeline::mode_from_config(&config_mgr.config),
    };
    let mode_str = format!("{mode_for_naming:?}").to_lowercase();
    let quality_str = config_mgr.config.preserve_quality.to_string();

    let output_path = match output {
        Some(p) => p.to_path_buf(),
        None => generate_output_path(input_file, &naming_pattern, &mode_str, &quality_str),
    };

    let out_format = resolve_output_format(format)?;

    if backup {
        let backup_path = input_file.with_extension(format!(
            "{}.bak",
            input_file
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("bak")
        ));
        std::fs::copy(input_file, &backup_path)?;
        if !json_mode {
            console.info(&format!("Backup created: {}", backup_path.display()));
        }
    }

    if !json_mode {
        console.info("Scanning for digital footprints...");
    }
    let scan_result = MetadataScanner::scan(input_file)?;
    let (audio_buf, src_format) = audio::load_audio(input_file)?;

    let watermark_result = WatermarkDetector::detect_all(&audio_buf);

    if !json_mode {
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
    }

    if dry_run {
        let json_report = JsonReport {
            file_path: input_file.display().to_string(),
            format: src_format.to_string(),
            duration_secs: audio_buf.duration_secs(),
            sample_rate: audio_buf.sample_rate,
            channels: audio_buf.num_channels(),
            watermark: watermark_result,
            metadata: Some(scan_result),
            statistical: None,
            polez: None,
            sanitization: None,
        };
        if json_mode {
            print_json(&json_report)?;
        } else {
            if let Some(report_path) = report {
                write_json_report(&json_report, report_path)?;
                console.success(&format!("Report saved to: {}", report_path.display()));
            }
            console.info("Dry run complete — no output file written.");
        }
        return Ok(());
    }

    let mode = match quality {
        Some(q) => quality_to_mode(q),
        None => SanitizationPipeline::mode_from_config(&config_mgr.config),
    };
    let audit_flags = flags.clone();
    if !paranoid && paranoid_passes != 2 {
        console.warning("--paranoid-passes has no effect without --paranoid");
    }
    let pipeline = SanitizationPipeline::new(
        mode,
        paranoid,
        paranoid_passes,
        flags,
        fp_config,
        out_format,
        freq_ranges,
        target_sample_rate,
        bit_depth,
    );

    if !json_mode {
        banner.show_processing_banner();
    }
    let spinner = if json_mode {
        None
    } else {
        Some(ui::progress::stage_spinner("Sanitizing audio..."))
    };

    let result = pipeline.run(input_file, &output_path)?;

    if let Some(s) = spinner {
        s.finish_and_clear();
    }

    if !json_mode {
        console.display_results(&SanitizationDisplay {
            success: result.success,
            metadata_removed: result.metadata_removed,
            patterns_found: result.patterns_found,
            patterns_suppressed: result.patterns_suppressed,
            quality_loss: result.quality_loss,
            processing_time: result.processing_time,
            output_file: Some(result.output_file.display().to_string()),
        });
    }

    if verify && result.success && !json_mode {
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

    if json_mode {
        let json_report = JsonReport {
            file_path: input_file.display().to_string(),
            format: src_format.to_string(),
            duration_secs: audio_buf.duration_secs(),
            sample_rate: audio_buf.sample_rate,
            channels: audio_buf.num_channels(),
            watermark: watermark_result,
            metadata: Some(scan_result),
            statistical: None,
            polez: None,
            sanitization: Some(SanitizationReport {
                success: result.success,
                metadata_removed: result.metadata_removed,
                patterns_found: result.patterns_found,
                patterns_suppressed: result.patterns_suppressed,
                quality_loss: result.quality_loss,
                processing_time: result.processing_time,
                output_file: result.output_file.display().to_string(),
            }),
        };
        print_json(&json_report)?;
    } else if let Some(report_path) = report {
        let json_report = JsonReport {
            file_path: input_file.display().to_string(),
            format: src_format.to_string(),
            duration_secs: audio_buf.duration_secs(),
            sample_rate: audio_buf.sample_rate,
            channels: audio_buf.num_channels(),
            watermark: watermark_result,
            metadata: Some(scan_result),
            statistical: None,
            polez: None,
            sanitization: Some(SanitizationReport {
                success: result.success,
                metadata_removed: result.metadata_removed,
                patterns_found: result.patterns_found,
                patterns_suppressed: result.patterns_suppressed,
                quality_loss: result.quality_loss,
                processing_time: result.processing_time,
                output_file: result.output_file.display().to_string(),
            }),
        };
        write_json_report(&json_report, report_path)?;
        console.success(&format!("Report saved to: {}", report_path.display()));
    }

    if let Some(audit_path) = audit_log {
        let input_hash = file_sha256(input_file).unwrap_or_else(|_| "hash_error".to_string());
        let output_hash = file_sha256(&output_path).unwrap_or_else(|_| "hash_error".to_string());
        let entry = AuditEntry {
            timestamp: unix_timestamp_iso8601(),
            input_hash,
            output_hash,
            mode: format!("{mode:?}"),
            paranoid,
            flags: audit_flags.clone(),
            metadata_removed: result.metadata_removed,
            patterns_found: result.patterns_found,
            patterns_suppressed: result.patterns_suppressed,
            quality_loss: result.quality_loss,
            processing_time: result.processing_time,
            success: result.success,
        };
        write_audit_entry(&entry, audit_path)?;
        if !json_mode {
            console.info(&format!(
                "Audit entry appended to: {}",
                audit_path.display()
            ));
        }
    }

    if !json_mode {
        if result.success {
            banner.show_success_banner();
            console.success("File sanitized successfully.");
            console.hacker_quote();
        } else {
            console.error("Sanitization failed!");
            process::exit(1);
        }
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
    format: FormatChoice,
    naming: Option<&str>,
    fp_config: config::FingerprintRemovalConfig,
    json_mode: bool,
    console: &ConsoleManager,
    banner: &BannerManager,
) -> error::Result<()> {
    let out_format = resolve_output_format(format)?;
    let start = Instant::now();

    if !json_mode {
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
    }

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
        if json_mode {
            return print_json(&serde_json::json!({
                "total": 0, "success": 0, "failed": 0, "files": []
            }));
        }
        console.warning("No audio files found in directory");
        return Ok(());
    }

    if !json_mode {
        console.success(&format!("Found {} files to process", files.len()));
    }

    if dry_run {
        if json_mode {
            let file_list: Vec<String> = files.iter().map(|f| f.display().to_string()).collect();
            return print_json(&serde_json::json!({
                "dry_run": true, "total": files.len(), "files": file_list
            }));
        }
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
    let sweep_naming = naming
        .map(|s| s.to_string())
        .unwrap_or_else(|| config_mgr.config.batch_processing.naming_pattern.clone());
    validate_naming_template(&sweep_naming)?;
    let mode = SanitizationPipeline::mode_from_config(&config_mgr.config);
    let mode_str = format!("{mode:?}").to_lowercase();
    let quality_str = config_mgr.config.preserve_quality.to_string();
    let flags = config_mgr.config.advanced_flags.clone();

    let pb = if json_mode {
        indicatif::ProgressBar::hidden()
    } else {
        ui::progress::batch_progress(files.len() as u64)
    };

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
                let named = generate_output_path(file, &sweep_naming, &mode_str, &quality_str);
                let named_filename = named.file_name().unwrap_or_default();
                let output_file = if let Some(parent) = relative.parent() {
                    out_dir.join(parent).join(named_filename)
                } else {
                    out_dir.join(named_filename)
                };

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
                    2,
                    flags.clone(),
                    fp_config.clone(),
                    out_format,
                    Vec::new(),
                    None,
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

    if json_mode {
        let failed_json: Vec<serde_json::Value> = failed_files
            .iter()
            .map(|(f, e)| serde_json::json!({"file": f, "error": e}))
            .collect();
        return print_json(&serde_json::json!({
            "total": files.len(),
            "success": success_count,
            "failed": fail_count,
            "processing_time": elapsed,
            "output_dir": out_dir.display().to_string(),
            "failures": failed_json,
        }));
    }

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

fn cmd_detect(
    input_file: &Path,
    deep: bool,
    report: Option<&Path>,
    filter: Option<Vec<String>>,
    json_mode: bool,
    console: &ConsoleManager,
) -> error::Result<()> {
    if !input_file.exists() {
        return Err(error::PolezError::FileNotFound(input_file.to_path_buf()));
    }

    // Validate filter method names
    if let Some(ref methods) = filter {
        for m in methods {
            if !WatermarkDetector::METHOD_NAMES.contains(&m.as_str()) {
                return Err(error::PolezError::Other(anyhow::anyhow!(
                    "Unknown detection method: '{}'. Valid methods: {}",
                    m,
                    WatermarkDetector::METHOD_NAMES.join(", ")
                )));
            }
        }
    }

    let is_filtered = filter.is_some();

    if !json_mode {
        console.info(&format!("Forensic analysis: {}", input_file.display()));
        console.info("Scanning for digital footprints...");
    }

    // When --filter is specified, skip metadata and polez detection for speed
    let scan_result = if is_filtered {
        None
    } else {
        Some(MetadataScanner::scan(input_file)?)
    };

    let (audio_buf, src_format) = audio::load_audio(input_file)?;

    let watermark_result = WatermarkDetector::detect_filtered(&audio_buf, filter.as_deref());

    let polez_result = if is_filtered {
        None
    } else {
        Some(detection::PolezDetector::detect(&audio_buf))
    };

    let stat_result = if deep && !is_filtered {
        Some(StatisticalAnalyzer::analyze(&audio_buf))
    } else {
        None
    };

    if json_mode {
        let json_report = JsonReport {
            file_path: input_file.display().to_string(),
            format: src_format.to_string(),
            duration_secs: audio_buf.duration_secs(),
            sample_rate: audio_buf.sample_rate,
            channels: audio_buf.num_channels(),
            watermark: watermark_result,
            metadata: scan_result,
            statistical: stat_result,
            polez: polez_result,
            sanitization: None,
        };
        return print_json(&json_report);
    }

    let watermark_display: Vec<(String, bool, f64)> = watermark_result
        .method_results
        .iter()
        .map(|(name, mr)| (name.clone(), mr.detected, mr.confidence))
        .collect();

    let ai_probability = stat_result.as_ref().map(|s| s.ai_probability);
    let anomaly_count = stat_result.as_ref().map(|s| s.anomalies.len()).unwrap_or(0);

    let metadata_tags = scan_result.as_ref().map_or(0, |s| s.tags.len());
    let suspicious_chunks = scan_result
        .as_ref()
        .map_or(0, |s| s.suspicious_chunks.len());

    let threats_found =
        metadata_tags + suspicious_chunks + watermark_result.watermark_count + anomaly_count;

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
        metadata_tags,
        suspicious_chunks,
        threats_found,
        threat_level: threat_level.to_string(),
        watermark_results: Some(watermark_display),
        ai_probability,
    });

    if let Some(ref polez) = polez_result {
        display_polez_results(polez, console);
    }

    match threat_level {
        "HIGH" => console.error("HIGH THREAT LEVEL - This file is heavily watermarked!"),
        "MEDIUM" => console.warning("MEDIUM THREAT LEVEL - Some traces detected"),
        _ => console.success("LOW THREAT LEVEL - Relatively clean"),
    }

    if let Some(report_path) = report {
        let json_report = JsonReport {
            file_path: input_file.display().to_string(),
            format: src_format.to_string(),
            duration_secs: audio_buf.duration_secs(),
            sample_rate: audio_buf.sample_rate,
            channels: audio_buf.num_channels(),
            watermark: watermark_result,
            metadata: scan_result,
            statistical: stat_result,
            polez: polez_result,
            sanitization: None,
        };
        write_json_report(&json_report, report_path)?;
        console.success(&format!("Report saved to: {}", report_path.display()));
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

#[allow(clippy::too_many_arguments)]
fn cmd_inspect(
    input_file: &Path,
    output: Option<&Path>,
    start: f64,
    duration: f64,
    freq_min: u32,
    freq_max: u32,
    json_mode: bool,
    console: &ConsoleManager,
) -> error::Result<()> {
    if !input_file.exists() {
        return Err(error::PolezError::FileNotFound(input_file.to_path_buf()));
    }

    if !json_mode {
        console.info(&format!("Loading: {}", input_file.display()));
    }
    let (audio_buf, _) = audio::load_audio(input_file)?;

    if json_mode {
        let result = serde_json::json!({
            "command": "inspect",
            "file": input_file.display().to_string(),
            "sample_rate": audio_buf.sample_rate,
            "duration_secs": audio_buf.duration_secs(),
            "channels": audio_buf.num_channels(),
            "freq_min": freq_min,
            "freq_max": freq_max,
            "start": start,
            "duration": duration,
        });
        return print_json(&result);
    }

    let view = inspect::SpectrogramView::new(freq_min, freq_max, start, duration);

    if let Some(svg_path) = output {
        console.info(&format!("Exporting spectrogram to: {}", svg_path.display()));
        view.export_svg(&audio_buf, svg_path)?;
        console.success(&format!("SVG written to {}", svg_path.display()));
    } else {
        console.info(&format!(
            "Generating spectrogram view ({}-{} kHz, {:.1}s-{:.1}s)...",
            freq_min / 1000,
            freq_max / 1000,
            start,
            start + duration
        ));
        view.render(&audio_buf)?;
    }

    Ok(())
}

fn cmd_bits(
    input_file: &Path,
    bit: u8,
    offset: usize,
    count: usize,
    search: bool,
    json_mode: bool,
    console: &ConsoleManager,
) -> error::Result<()> {
    if !input_file.exists() {
        return Err(error::PolezError::FileNotFound(input_file.to_path_buf()));
    }

    if !json_mode {
        console.info(&format!("Loading: {}", input_file.display()));
    }
    let (audio_buf, _) = audio::load_audio(input_file)?;

    if json_mode {
        let samples = audio_buf.to_mono_samples();
        let end = (offset + count).min(samples.len());
        let slice = &samples[offset..end];
        let bits: Vec<u8> = slice
            .iter()
            .map(|&s| ((s * 32767.0) as i16 >> bit) as u8 & 1)
            .collect();
        let ones = bits.iter().filter(|&&b| b == 1).count();
        let result = serde_json::json!({
            "command": "bits",
            "file": input_file.display().to_string(),
            "bit_plane": bit,
            "offset": offset,
            "count": bits.len(),
            "ones": ones,
            "zeros": bits.len() - ones,
            "ones_ratio": ones as f64 / bits.len() as f64,
            "bias": ((ones as f64 / bits.len() as f64) - 0.5).abs(),
        });
        return print_json(&result);
    }

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

fn cmd_fingerprint(
    files: &[PathBuf],
    json_mode: bool,
    console: &ConsoleManager,
) -> error::Result<()> {
    use detection::perceptual_hash;

    // Validate all files exist
    for f in files {
        if !f.exists() {
            return Err(error::PolezError::FileNotFound(f.clone()));
        }
    }

    // Compute hashes for all files
    let mut hashes: Vec<(String, perceptual_hash::PerceptualHash)> = Vec::new();
    for f in files {
        if !json_mode {
            console.info(&format!("Hashing: {}", f.display()));
        }
        let (buffer, _fmt) = audio::io::load_audio(f)?;
        let hash = perceptual_hash::compute_hash(&buffer);
        hashes.push((f.display().to_string(), hash));
    }

    // Compare all pairs
    let mut comparisons: Vec<perceptual_hash::HashComparison> = Vec::new();
    for i in 0..hashes.len() {
        for j in (i + 1)..hashes.len() {
            let similarity = perceptual_hash::compare_hashes(&hashes[i].1, &hashes[j].1);
            comparisons.push(perceptual_hash::HashComparison {
                file_a: hashes[i].0.clone(),
                file_b: hashes[j].0.clone(),
                similarity,
                hash_length_a: hashes[i].1.hash.len(),
                hash_length_b: hashes[j].1.hash.len(),
            });
        }
    }

    if json_mode {
        #[derive(Serialize)]
        struct FingerprintReport {
            hashes: Vec<FileHash>,
            comparisons: Vec<perceptual_hash::HashComparison>,
        }
        #[derive(Serialize)]
        struct FileHash {
            file: String,
            hash: String,
            hash_words: usize,
            duration_secs: f64,
        }
        let report = FingerprintReport {
            hashes: hashes
                .iter()
                .map(|(name, h)| FileHash {
                    file: name.clone(),
                    hash: perceptual_hash::hash_to_hex(&h.hash),
                    hash_words: h.hash.len(),
                    duration_secs: h.duration_secs,
                })
                .collect(),
            comparisons,
        };
        print_json(&report)?;
    } else {
        console.info("Perceptual Hashes");
        for (name, hash) in &hashes {
            let hex = perceptual_hash::hash_to_hex(&hash.hash);
            let display_hex = if hex.len() > 32 {
                format!("{}...", &hex[..32])
            } else {
                hex
            };
            console.info(&format!(
                "  {} — {} ({:.1}s)",
                name, display_hex, hash.duration_secs
            ));
        }

        println!();
        console.info("Comparisons");
        for comp in &comparisons {
            let label = if comp.similarity > 0.9 {
                "MATCH"
            } else if comp.similarity > 0.7 {
                "SIMILAR"
            } else if comp.similarity > 0.5 {
                "WEAK"
            } else {
                "DIFFERENT"
            };
            console.info(&format!(
                "  {} vs {} — {:.1}% similarity [{}]",
                comp.file_a,
                comp.file_b,
                comp.similarity * 100.0,
                label
            ));
        }
    }

    Ok(())
}

fn cmd_config(action: Option<ConfigAction>, console: &ConsoleManager) -> error::Result<()> {
    match action {
        None | Some(ConfigAction::Show) => {
            let config_mgr = ConfigManager::new()?;
            let yaml = serde_yaml::to_string(&config_mgr.config)
                .map_err(|e| error::PolezError::Config(format!("Serialize error: {e}")))?;

            if !config_mgr.env_overrides.is_empty() {
                console.warning("Active environment variable overrides:");
                for ov in &config_mgr.env_overrides {
                    console.info(&format!("  {ov}"));
                }
                println!();
            }

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
                FormatChoice::Flac | FormatChoice::Ogg | FormatChoice::Aac => {
                    config::OutputFormat::Preserve
                }
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
        Some(ConfigAction::Validate) => {
            let config_mgr = ConfigManager::new()?;

            let unknown = config_mgr.check_unknown_fields();
            let issues = config_mgr.validate();

            let mut has_problems = false;

            for warning in &unknown {
                console.warning(warning);
                has_problems = true;
            }

            for issue in &issues {
                if issue.is_error {
                    console.error(&format!("{}: {}", issue.field, issue.message));
                } else {
                    console.warning(&format!("{}: {}", issue.field, issue.message));
                }
                has_problems = true;
            }

            if !has_problems {
                console.success("Configuration is valid");
            }

            Ok(())
        }
    }
}

fn quality_to_mode(q: u32) -> SanitizationMode {
    match q {
        0..=24 => SanitizationMode::Fast,
        25..=49 => SanitizationMode::Standard,
        50..=74 => SanitizationMode::Preserving,
        _ => SanitizationMode::Aggressive,
    }
}

fn resolve_output_format(format: FormatChoice) -> error::Result<Option<audio::AudioFormat>> {
    match format {
        FormatChoice::Preserve => Ok(None),
        FormatChoice::Mp3 => Ok(Some(audio::AudioFormat::Mp3)),
        FormatChoice::Wav => Ok(Some(audio::AudioFormat::Wav)),
        FormatChoice::Flac => Ok(Some(audio::AudioFormat::Flac)),
        FormatChoice::Ogg => Ok(Some(audio::AudioFormat::Ogg)),
        FormatChoice::Aac => Err(error::PolezError::UnsupportedFormat(
            "AAC encoding is not supported; use wav, mp3, flac, or ogg".into(),
        )),
    }
}

fn generate_output_path(input: &Path, pattern: &str, mode: &str, quality: &str) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ext = input.extension().and_then(|s| s.to_str()).unwrap_or("wav");
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    let name = pattern
        .replace("{name}", stem)
        .replace("{ext}", &format!(".{ext}"))
        .replace("{date}", &date)
        .replace("{mode}", mode)
        .replace("{quality}", quality)
        .replace("{counter}", "001");

    input.with_file_name(name)
}

/// Validate that a naming template only contains known variables.
fn validate_naming_template(pattern: &str) -> error::Result<()> {
    let known = [
        "{name}",
        "{ext}",
        "{date}",
        "{mode}",
        "{quality}",
        "{counter}",
    ];
    let mut rest = pattern;
    while let Some(start) = rest.find('{') {
        if let Some(end) = rest[start..].find('}') {
            let var = &rest[start..start + end + 1];
            if !known.contains(&var) {
                return Err(error::PolezError::Config(format!(
                    "Unknown naming template variable '{var}'. Valid: {}",
                    known.join(", ")
                )));
            }
            rest = &rest[start + end + 1..];
        } else {
            return Err(error::PolezError::Config(
                "Unclosed '{' in naming template".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_output_path_default() {
        let input = Path::new("/tmp/track.wav");
        let result = generate_output_path(input, "{name}_clean{ext}", "standard", "high");
        assert_eq!(result.file_name().unwrap(), "track_clean.wav");
    }

    #[test]
    fn test_generate_output_path_with_mode() {
        let input = Path::new("/tmp/song.mp3");
        let result = generate_output_path(input, "{name}_{mode}{ext}", "fast", "low");
        assert_eq!(result.file_name().unwrap(), "song_fast.mp3");
    }

    #[test]
    fn test_generate_output_path_with_date() {
        let input = Path::new("/tmp/file.wav");
        let result = generate_output_path(input, "{name}_{date}{ext}", "standard", "high");
        let name = result.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("file_"));
        assert!(name.ends_with(".wav"));
        assert!(name.contains('-'));
    }

    #[test]
    fn test_generate_output_path_with_quality() {
        let input = Path::new("/tmp/audio.flac");
        let result = generate_output_path(input, "{name}_{quality}{ext}", "standard", "maximum");
        assert_eq!(result.file_name().unwrap(), "audio_maximum.flac");
    }

    #[test]
    fn test_validate_naming_template_valid() {
        assert!(validate_naming_template("{name}_clean{ext}").is_ok());
        assert!(validate_naming_template("{name}_{date}_{mode}{ext}").is_ok());
        assert!(validate_naming_template("{name}_{counter}{ext}").is_ok());
        assert!(validate_naming_template("prefix_{name}{ext}").is_ok());
    }

    #[test]
    fn test_validate_naming_template_unknown_var() {
        let result = validate_naming_template("{name}_{unknown}{ext}");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown"));
    }

    #[test]
    fn test_validate_naming_template_unclosed_brace() {
        let result = validate_naming_template("{name}_{broken");
        assert!(result.is_err());
    }
}
