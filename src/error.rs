//! Crate-wide error types and result alias.

use std::path::PathBuf;

/// Unified error type for all polez operations.
#[derive(thiserror::Error, Debug)]
pub enum PolezError {
    /// Error during audio file reading or writing.
    #[error("Audio I/O error: {0}")]
    AudioIo(String),

    /// The audio codec or container is not supported.
    #[error("Unsupported audio format: {0}")]
    UnsupportedFormat(String),

    /// Error reading or writing audio metadata tags.
    #[error("Metadata error: {0}")]
    Metadata(String),

    /// Error in DSP processing (FFT, filtering, resampling, etc.).
    #[error("DSP processing error: {0}")]
    Dsp(String),

    /// Invalid or missing configuration value.
    #[error("Configuration error: {0}")]
    Config(String),

    /// The specified file does not exist.
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// Post-sanitization verification detected a problem.
    #[error("Verification failed: {0}")]
    Verification(String),

    /// Transparent wrapper for standard I/O errors.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Catch-all for other errors via `anyhow`.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Convenience result type using [`PolezError`].
pub type Result<T> = std::result::Result<T, PolezError>;
