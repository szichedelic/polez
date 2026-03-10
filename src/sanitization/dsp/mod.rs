//! Low-level DSP primitives for audio processing.
//!
//! Provides STFT, biquad filters, Hilbert transform, resampling, statistical
//! functions, zero-phase filtering, and SIMD-optimized vector operations.

pub mod biquad;
pub mod filtfilt;
pub mod hilbert;
pub mod resample;
pub mod simd;
pub mod stats;
pub mod stft;
