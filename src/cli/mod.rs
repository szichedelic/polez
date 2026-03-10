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
    Clean {
        /// Input audio file path
        input_file: PathBuf,

        /// Output file path (auto-generates if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Maximum destruction mode - multiple passes with aggressive cleaning
        #[arg(long)]
        paranoid: bool,

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

        #[command(flatten)]
        fp_flags: FingerprintFlagsCli,
    },

    /// Detect watermarks and metadata in an audio file
    Detect {
        /// Input audio file path
        input_file: PathBuf,

        /// Enable deep analysis (slower but more thorough)
        #[arg(long)]
        deep: bool,

        /// Export JSON analysis report to file
        #[arg(long)]
        report: Option<PathBuf>,
    },

    /// Batch scan directory and output CSV results for dataset analysis
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
    Inspect {
        /// Input audio file path
        input_file: PathBuf,

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
