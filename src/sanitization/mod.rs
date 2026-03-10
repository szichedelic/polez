//! Audio sanitization pipeline and cleaning modules.
//!
//! Orchestrates metadata stripping, spectral cleaning, fingerprint removal,
//! and stealth DSP operations to remove watermarks and fingerprints from audio.

pub mod dsp;
pub mod fingerprint;
pub mod metadata;
pub mod pipeline;
pub mod psychoacoustic;
pub mod spectral;
pub mod stealth;

pub use pipeline::SanitizationPipeline;
