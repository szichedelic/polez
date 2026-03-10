//! Audio I/O and buffer management.
//!
//! Provides the core `AudioBuffer` type for multi-channel sample data,
//! along with functions for loading and saving audio in various formats.

pub mod buffer;
pub mod io;

pub use buffer::AudioBuffer;
pub use io::{load_audio, save_audio, AudioFormat};
