//! Web-based forensics GUI backend.
//!
//! Serves an embedded React SPA via `rust-embed` and exposes REST API
//! endpoints through an Axum server for audio analysis and sanitization.

mod routes;
mod types;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::audio::AudioBuffer;

/// Embedded frontend assets from `gui/dist/`.
#[derive(rust_embed::RustEmbed)]
#[folder = "gui/dist/"]
pub(crate) struct Assets;

/// Pre-computed min/max waveform data at a fixed overview resolution.
/// Avoids recomputing downsampled waveform on every request.
#[derive(Clone)]
pub struct WaveformCache {
    /// Per-chunk minimum sample values.
    pub min: Vec<f32>,
    /// Per-chunk maximum sample values.
    pub max: Vec<f32>,
    /// Total audio duration in seconds.
    pub duration_secs: f64,
    /// Audio sample rate in Hz.
    pub sample_rate: u32,
    /// Number of audio channels.
    pub channels: usize,
}

/// Overview resolution — number of min/max points in the cached overview.
const OVERVIEW_WIDTH: usize = 2048;

impl WaveformCache {
    /// Build a downsampled overview from an AudioBuffer.
    pub fn from_buffer(buffer: &AudioBuffer) -> Self {
        let samples = buffer.to_mono_samples();
        let total = samples.len();
        let duration_secs = total as f64 / buffer.sample_rate as f64;
        let chunk_size = (total / OVERVIEW_WIDTH).max(1);

        let mut min_vals = Vec::with_capacity(OVERVIEW_WIDTH);
        let mut max_vals = Vec::with_capacity(OVERVIEW_WIDTH);

        for chunk in samples.chunks(chunk_size) {
            let mut lo = f32::MAX;
            let mut hi = f32::MIN;
            for &s in chunk {
                if s < lo {
                    lo = s;
                }
                if s > hi {
                    hi = s;
                }
            }
            min_vals.push(lo);
            max_vals.push(hi);
        }

        Self {
            min: min_vals,
            max: max_vals,
            duration_secs,
            sample_rate: buffer.sample_rate,
            channels: buffer.num_channels(),
        }
    }

    /// Extract a sub-range from the cached overview, re-downsampled to the requested width.
    pub fn slice_for_range(
        &self,
        start_sec: f64,
        end_sec: f64,
        width: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let total_points = self.min.len();
        let start_frac = (start_sec / self.duration_secs).clamp(0.0, 1.0);
        let end_frac = (end_sec / self.duration_secs).clamp(0.0, 1.0);
        let start_idx = (start_frac * total_points as f64) as usize;
        let end_idx = ((end_frac * total_points as f64) as usize).min(total_points);

        if start_idx >= end_idx {
            return (vec![], vec![]);
        }

        let slice_min = &self.min[start_idx..end_idx];
        let slice_max = &self.max[start_idx..end_idx];

        if slice_min.len() <= width {
            return (slice_min.to_vec(), slice_max.to_vec());
        }

        // Further downsample from cache to requested width
        let chunk_size = (slice_min.len() / width).max(1);
        let mut out_min = Vec::with_capacity(width);
        let mut out_max = Vec::with_capacity(width);

        for i in (0..slice_min.len()).step_by(chunk_size) {
            let end = (i + chunk_size).min(slice_min.len());
            let mut lo = f32::MAX;
            let mut hi = f32::MIN;
            for j in i..end {
                if slice_min[j] < lo {
                    lo = slice_min[j];
                }
                if slice_max[j] > hi {
                    hi = slice_max[j];
                }
            }
            out_min.push(lo);
            out_max.push(hi);
        }

        (out_min, out_max)
    }
}

/// Server-side state holding loaded and cleaned audio buffers.
pub struct AppState {
    /// Currently loaded original audio buffer.
    pub buffer: Option<AudioBuffer>,
    /// Path of the loaded original file.
    pub file_path: Option<String>,
    /// Detected audio format of the original file.
    pub format: Option<String>,
    /// Audio buffer after sanitization.
    pub cleaned_buffer: Option<AudioBuffer>,
    /// Path of the cleaned output file.
    pub cleaned_file_path: Option<String>,
    /// Audio format of the cleaned file.
    pub cleaned_format: Option<String>,
    /// Pre-computed waveform overview for original audio
    pub waveform_cache: Option<WaveformCache>,
    /// Pre-computed waveform overview for cleaned audio
    pub cleaned_waveform_cache: Option<WaveformCache>,
    /// Temp file paths to clean up when state is dropped
    pub temp_paths: Vec<std::path::PathBuf>,
}

impl AppState {
    /// Create empty application state with no loaded audio.
    pub fn new() -> Self {
        Self {
            buffer: None,
            file_path: None,
            format: None,
            cleaned_buffer: None,
            cleaned_file_path: None,
            cleaned_format: None,
            waveform_cache: None,
            cleaned_waveform_cache: None,
            temp_paths: Vec::new(),
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        for path in &self.temp_paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Thread-safe shared application state.
pub type SharedState = Arc<RwLock<AppState>>;

/// Start the Axum HTTP server on the given port and optionally open a browser.
pub async fn start_server(port: u16, no_open: bool) -> Result<()> {
    let state: SharedState = Arc::new(RwLock::new(AppState::new()));
    let app = routes::create_router(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Polez GUI running at http://localhost:{port}");

    if !no_open {
        let _ = open::that(format!("http://localhost:{port}"));
    }

    axum::serve(listener, app).await?;
    Ok(())
}
