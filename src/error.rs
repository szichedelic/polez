use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum PolezError {
    #[error("Audio I/O error: {0}")]
    AudioIo(String),

    #[error("Unsupported audio format: {0}")]
    UnsupportedFormat(String),

    #[error("Metadata error: {0}")]
    Metadata(String),

    #[error("DSP processing error: {0}")]
    Dsp(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Verification failed: {0}")]
    Verification(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, PolezError>;
