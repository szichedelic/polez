//! Polez — audio forensics and sanitization engine.
//!
//! Provides detection and removal of watermarks, metadata, and statistical
//! fingerprints from audio files. Includes a CLI, a web-based GUI, and a
//! library API for programmatic use.

#![allow(dead_code)]
#![allow(clippy::new_without_default)]

/// Audio I/O and buffer management.
pub mod audio;
/// Command-line interface definitions.
pub mod cli;
/// YAML-based configuration and preset management.
pub mod config;
/// Watermark and fingerprint detection algorithms.
pub mod detection;
/// Crate-wide error types.
pub mod error;
/// Web-based forensics GUI (Axum + embedded React SPA).
#[cfg(feature = "gui")]
pub mod gui;
/// Spectrogram and bit-plane visualization tools.
pub mod inspect;
/// Audio cleaning and DSP pipeline.
pub mod sanitization;
/// Console output, banners, and progress bars.
pub mod ui;
/// Post-processing verification comparing before/after analysis.
pub mod verification;
