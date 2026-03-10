pub mod flags;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

pub use flags::{AdvancedFlagsCli, FingerprintFlagsCli};

#[derive(Parser)]
#[command(
    name = "polez",
    version = "2.0.0",
    about = "Polez - Audio forensics and sanitization engine",
    long_about = "Audio forensics tool for analyzing and sanitizing watermarks and metadata\nfrom audio files."
)]
pub struct Cli {
    /// Output results as JSON to stdout (suppresses banners and progress bars)
    #[arg(long, global = true)]
    pub json: bool,

    /// Increase verbosity (-v = debug, -vv = trace)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress all output except errors and final result
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Sanitize all traces from a single audio file
    #[command(after_help = "\
Examples:
  polez clean track.wav                         Clean with default settings
  polez clean track.mp3 -o clean.mp3 --verify   Clean and verify effectiveness
  polez clean track.wav --paranoid --backup      Maximum destruction with backup
  polez clean track.wav --paranoid --paranoid-passes 5  Custom pass count
  polez clean track.wav --dry-run --report r.json  Analyze without cleaning
  polez clean track.mp3 -f wav                    Convert output to WAV format
  polez clean track.wav --quality 75              Fine-grained quality control (0-100)
  polez --json clean track.wav                   Machine-readable JSON output")]
    Clean {
        /// Input audio file path
        input_file: PathBuf,

        /// Output file path (auto-generates if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Maximum destruction mode - multiple passes with aggressive cleaning
        #[arg(long)]
        paranoid: bool,

        /// Number of additional paranoid passes (1-10, default: 2)
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u32).range(1..=10))]
        paranoid_passes: u32,

        /// Quality slider 0-100 (alternative to config-based mode selection)
        /// 0-24=fast, 25-49=standard, 50-74=preserving, 75-100=aggressive
        #[arg(long, value_parser = clap::value_parser!(u32).range(0..=100))]
        quality: Option<u32>,

        /// Verify watermark removal effectiveness
        #[arg(long)]
        verify: bool,

        /// Create backup of original file
        #[arg(long)]
        backup: bool,

        /// Run detection and analysis only, without writing output
        #[arg(long)]
        dry_run: bool,

        /// Output audio format
        #[arg(short, long, value_enum, default_value = "preserve")]
        format: FormatChoice,

        /// Export JSON analysis report to file
        #[arg(long)]
        report: Option<PathBuf>,

        /// Append audit log entry (JSON lines) — logs hashes, mode, flags (no file paths)
        #[arg(long)]
        audit_log: Option<PathBuf>,

        #[command(flatten)]
        flags: AdvancedFlagsCli,

        #[command(flatten)]
        fp_flags: FingerprintFlagsCli,
    },

    /// Sweep an entire directory of audio files
    #[command(after_help = "\
Examples:
  polez sweep ./music                           Clean all audio files in directory
  polez sweep ./music -d ./clean -w 8           Custom output dir, 8 workers
  polez sweep ./music -r --paranoid --backup    Recursive paranoid mode with backups
  polez sweep ./music -e mp3 -e wav --dry-run   Preview which files would be processed
  polez sweep ./music -f wav                    Convert all output to WAV format")]
    Sweep {
        /// Directory containing audio files
        directory: PathBuf,

        /// Output directory (creates subdirectory if not specified)
        #[arg(short = 'd', long)]
        output_dir: Option<PathBuf>,

        /// File extensions to process
        #[arg(short, long, default_values = ["mp3", "wav", "flac", "aac", "m4a"])]
        extension: Vec<String>,

        /// Maximum destruction mode
        #[arg(long)]
        paranoid: bool,

        /// Number of parallel workers
        #[arg(short, long, default_value = "4")]
        workers: u32,

        /// Create backups of original files
        #[arg(long)]
        backup: bool,

        /// Recursively process subdirectories
        #[arg(short, long)]
        recursive: bool,

        /// List files without processing
        #[arg(long)]
        dry_run: bool,

        /// Output audio format (overrides input format)
        #[arg(short, long, value_enum, default_value = "preserve")]
        format: FormatChoice,

        #[command(flatten)]
        fp_flags: FingerprintFlagsCli,
    },

    /// Detect watermarks and metadata in an audio file
    #[command(after_help = "\
Examples:
  polez detect track.wav                        Quick watermark scan
  polez detect track.mp3 --deep                 Deep analysis with statistical tests
  polez detect track.wav --report analysis.json  Export detailed report to JSON
  polez detect track.wav --filter spread_spectrum,echo_signatures
  polez --json detect track.wav                  Pipe results to jq or scripts")]
    Detect {
        /// Input audio file path
        input_file: PathBuf,

        /// Enable deep analysis (slower but more thorough)
        #[arg(long)]
        deep: bool,

        /// Export JSON analysis report to file
        #[arg(long)]
        report: Option<PathBuf>,

        /// Run only specific detection methods (comma-separated)
        /// Valid: spread_spectrum, echo_signatures, statistical_anomalies,
        /// phase_modulation, amplitude_modulation, frequency_domain
        #[arg(long, value_delimiter = ',')]
        filter: Option<Vec<String>>,
    },

    /// Batch scan directory and output CSV results for dataset analysis
    #[command(after_help = "\
Examples:
  polez benchmark ./dataset                     Scan directory, output CSV
  polez benchmark ./dataset -o results.csv -r   Recursive scan with custom output
  polez benchmark ./dataset -e wav -e flac      Scan only WAV and FLAC files")]
    Benchmark {
        /// Directory containing audio files
        directory: PathBuf,

        /// Output CSV file path
        #[arg(short, long, default_value = "polez_benchmark.csv")]
        output: PathBuf,

        /// Recursively scan subdirectories
        #[arg(short, long)]
        recursive: bool,

        /// File extensions to process
        #[arg(short, long, default_values = ["mp3", "wav", "flac", "aac", "m4a"])]
        extension: Vec<String>,
    },

    /// Visualize high-frequency spectrogram to reveal watermarks
    #[command(after_help = "\
Examples:
  polez inspect track.wav                       Show 15-24 kHz spectrogram
  polez inspect track.wav --start 10 --duration 3  Inspect specific time range
  polez inspect track.wav --freq-min 18000      Focus on ultrasonic range
  polez inspect track.wav -o spectrogram.svg    Export spectrogram as SVG file")]
    Inspect {
        /// Input audio file path
        input_file: PathBuf,

        /// Export spectrogram to SVG file instead of console
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Start time in seconds
        #[arg(long, default_value = "0")]
        start: f64,

        /// Duration in seconds (0 = auto)
        #[arg(long, default_value = "5")]
        duration: f64,

        /// Minimum frequency in Hz
        #[arg(long, default_value = "15000")]
        freq_min: u32,

        /// Maximum frequency in Hz
        #[arg(long, default_value = "24000")]
        freq_max: u32,
    },

    /// View raw bit patterns to find embedded watermark data
    #[command(after_help = "\
Examples:
  polez bits track.wav                          Analyze LSB plane (default)
  polez bits track.wav -b 1 --search            Scan bit plane 1 for ASCII strings
  polez bits track.wav --offset 5000 -c 20000   Analyze specific sample range")]
    Bits {
        /// Input audio file path
        input_file: PathBuf,

        /// Which bit plane to analyze (0=LSB, 7=MSB)
        #[arg(short, long, default_value = "0")]
        bit: u8,

        /// Start sample offset
        #[arg(long, default_value = "0")]
        offset: usize,

        /// Number of samples to analyze
        #[arg(short, long, default_value = "10000")]
        count: usize,

        /// Search for ASCII strings in bit stream
        #[arg(long)]
        search: bool,
    },

    /// Launch web-based forensics GUI
    #[cfg(feature = "gui")]
    Gui {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Don't auto-open browser
        #[arg(long)]
        no_open: bool,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },

    /// Show version and build information
    Version,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,

    /// Apply a configuration preset
    Preset {
        /// Preset name (stealth, stealth-plus, fast, quality, research)
        name: String,
    },

    /// List available presets
    List,

    /// Create a custom configuration preset
    Create {
        /// Preset name
        name: String,

        /// Paranoia level
        #[arg(long, value_enum, default_value = "medium")]
        paranoid: ParanoiaChoice,

        /// Quality preservation level
        #[arg(long, value_enum, default_value = "high")]
        quality: QualityChoice,

        /// Output format
        #[arg(long, value_enum, default_value = "preserve")]
        format: FormatChoice,

        /// Backup originals by default
        #[arg(long)]
        backup: bool,

        /// Auto-verify after processing
        #[arg(long, default_value = "true")]
        verify: bool,
    },

    /// Delete a custom preset
    Delete {
        /// Preset name to delete
        name: String,
    },

    /// Reset configuration to defaults
    Reset,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum FormatChoice {
    Preserve,
    Mp3,
    Wav,
    Flac,
    Aac,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ParanoiaChoice {
    Low,
    Medium,
    High,
    Maximum,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum QualityChoice {
    Low,
    Medium,
    High,
    Maximum,
}
