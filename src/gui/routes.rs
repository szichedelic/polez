use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rustfft::num_complex::Complex64;
use rustfft::FftPlanner;
use serde::Deserialize;
use std::path::Path;
use tower_http::cors::{Any, CorsLayer};

use crate::detection::{MetadataScanner, PolezDetector, StatisticalAnalyzer, WatermarkDetector};

use super::types::{
    AllAnalysisResult, BitPlaneData, FileInfo, PlaneSummary, SpectrogramData, WaveformData,
};
use super::SharedState;

pub fn create_router(state: SharedState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(health))
        .route("/api/load", post(load_file))
        .route("/api/waveform", get(get_waveform))
        .route("/api/spectrogram", get(get_spectrogram))
        .route("/api/bitplane", get(get_bitplane))
        .route("/api/audio", get(serve_audio))
        .route("/api/analyze/watermark", post(analyze_watermark))
        .route("/api/analyze/polez", post(analyze_polez))
        .route("/api/analyze/statistical", post(analyze_statistical))
        .route("/api/analyze/metadata", post(analyze_metadata))
        .route("/api/analyze/all", post(analyze_all))
        .fallback(get(static_handler))
        .layer(cors)
        .with_state(state)
}

async fn static_handler(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match super::Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.to_vec(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: serve index.html for all non-API routes
            match super::Assets::get("index.html") {
                Some(content) => {
                    ([(header::CONTENT_TYPE, "text/html")], content.data.to_vec()).into_response()
                }
                None => (StatusCode::NOT_FOUND, "Not found").into_response(),
            }
        }
    }
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
struct LoadRequest {
    path: String,
}

async fn load_file(
    State(state): State<SharedState>,
    Json(req): Json<LoadRequest>,
) -> Result<Json<FileInfo>, (StatusCode, String)> {
    let path = Path::new(&req.path);

    if !path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("File not found: {}", req.path),
        ));
    }

    let (buffer, format) = crate::audio::load_audio(path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Load error: {e}"),
        )
    })?;

    let info = FileInfo {
        file_path: req.path.clone(),
        format: format.to_string(),
        duration_secs: buffer.duration_secs(),
        sample_rate: buffer.sample_rate,
        channels: buffer.num_channels(),
    };

    let mut state = state.write().await;
    state.file_path = Some(req.path);
    state.format = Some(info.format.clone());
    state.buffer = Some(buffer);

    Ok(Json(info))
}

// --- Analysis endpoints ---

async fn analyze_watermark(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let state = state.read().await;
    let buffer = state
        .buffer
        .as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;
    let result = WatermarkDetector::detect_all(buffer);
    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn analyze_polez(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let state = state.read().await;
    let buffer = state
        .buffer
        .as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;
    let result = PolezDetector::detect(buffer);
    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn analyze_statistical(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let state = state.read().await;
    let buffer = state
        .buffer
        .as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;
    let result = StatisticalAnalyzer::analyze(buffer);
    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn analyze_metadata(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let state = state.read().await;
    let file_path = state
        .file_path
        .as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;
    let result = MetadataScanner::scan(Path::new(file_path)).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Scan error: {e}"),
        )
    })?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn analyze_all(
    State(state): State<SharedState>,
) -> Result<Json<AllAnalysisResult>, (StatusCode, String)> {
    let state = state.read().await;
    let buffer = state
        .buffer
        .as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;
    let file_path = state
        .file_path
        .as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;

    let watermark = WatermarkDetector::detect_all(buffer);
    let polez = PolezDetector::detect(buffer);
    let statistical = StatisticalAnalyzer::analyze(buffer);
    let metadata = MetadataScanner::scan(Path::new(file_path)).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Scan error: {e}"),
        )
    })?;

    Ok(Json(AllAnalysisResult {
        watermark,
        polez,
        statistical,
        metadata,
    }))
}

// --- Waveform endpoint ---

#[derive(Deserialize)]
struct WaveformQuery {
    width: Option<usize>,
    start: Option<f64>,
    end: Option<f64>,
}

async fn get_waveform(
    State(state): State<SharedState>,
    Query(query): Query<WaveformQuery>,
) -> Result<Json<WaveformData>, (StatusCode, String)> {
    let state = state.read().await;
    let buffer = state
        .buffer
        .as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;

    let samples = buffer.to_mono_samples();
    let sr = buffer.sample_rate as f64;
    let total_samples = samples.len();
    let duration_secs = total_samples as f64 / sr;

    let start_sec = query.start.unwrap_or(0.0).max(0.0);
    let end_sec = query.end.unwrap_or(duration_secs).min(duration_secs);
    let start_idx = (start_sec * sr) as usize;
    let end_idx = ((end_sec * sr) as usize).min(total_samples);

    if start_idx >= end_idx {
        return Err((StatusCode::BAD_REQUEST, "Invalid time range".to_string()));
    }

    let slice = &samples[start_idx..end_idx];
    let width = query.width.unwrap_or(1024).max(1);
    let chunk_size = (slice.len() / width).max(1);

    let mut min_vals = Vec::with_capacity(width);
    let mut max_vals = Vec::with_capacity(width);

    for chunk in slice.chunks(chunk_size) {
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

    Ok(Json(WaveformData {
        min: min_vals,
        max: max_vals,
        sample_rate: buffer.sample_rate,
        duration_secs,
        channels: buffer.num_channels(),
    }))
}

// --- Spectrogram endpoint ---

#[derive(Deserialize)]
struct SpectrogramQuery {
    fft_size: Option<usize>,
    freq_min: Option<f64>,
    freq_max: Option<f64>,
    start: Option<f64>,
    duration: Option<f64>,
}

async fn get_spectrogram(
    State(state): State<SharedState>,
    Query(query): Query<SpectrogramQuery>,
) -> Result<Json<SpectrogramData>, (StatusCode, String)> {
    let state = state.read().await;
    let buffer = state
        .buffer
        .as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;

    let samples = buffer.to_mono_samples();
    let sr = buffer.sample_rate as f64;
    let total_duration = samples.len() as f64 / sr;

    let fft_size = query.fft_size.unwrap_or(2048);
    let hop = fft_size / 4;
    let freq_min = query.freq_min.unwrap_or(0.0);
    let freq_max = query.freq_max.unwrap_or(sr / 2.0);
    let start_sec = query.start.unwrap_or(0.0).max(0.0);
    let dur = query.duration.unwrap_or(total_duration - start_sec);
    let end_sec = (start_sec + dur).min(total_duration);

    let start_idx = (start_sec * sr) as usize;
    let end_idx = ((end_sec * sr) as usize).min(samples.len());

    if start_idx >= end_idx || end_idx - start_idx < fft_size {
        return Err((
            StatusCode::BAD_REQUEST,
            "Not enough samples for FFT".to_string(),
        ));
    }

    let slice = &samples[start_idx..end_idx];

    // Build Hann window
    let window: Vec<f64> = (0..fft_size)
        .map(|i| {
            0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (fft_size as f64 - 1.0)).cos())
        })
        .collect();

    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(fft_size);

    // Frequency bin indices for the requested range
    let bin_min = ((freq_min * fft_size as f64 / sr).floor() as usize).min(fft_size / 2);
    let bin_max = ((freq_max * fft_size as f64 / sr).ceil() as usize).min(fft_size / 2);
    let num_freq_bins = bin_max - bin_min;

    if num_freq_bins == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid frequency range".to_string(),
        ));
    }

    let mut magnitudes: Vec<Vec<f64>> = Vec::new();

    let mut pos = 0;
    while pos + fft_size <= slice.len() {
        let mut fft_buf: Vec<Complex64> = slice[pos..pos + fft_size]
            .iter()
            .enumerate()
            .map(|(i, &s)| Complex64::new(s as f64 * window[i], 0.0))
            .collect();

        fft.process(&mut fft_buf);

        let frame: Vec<f64> = (bin_min..bin_max)
            .map(|bin| {
                let mag = fft_buf[bin].norm();
                // Convert to dB, floor at -120 dB
                let db = 20.0 * mag.max(1e-12).log10();
                db.max(-120.0)
            })
            .collect();

        magnitudes.push(frame);
        pos += hop;
    }

    let num_time_frames = magnitudes.len();

    Ok(Json(SpectrogramData {
        magnitudes,
        freq_min,
        freq_max,
        time_start: start_sec,
        time_end: end_sec,
        num_freq_bins,
        num_time_frames,
    }))
}

// --- Bitplane endpoint ---

async fn get_bitplane(
    State(state): State<SharedState>,
) -> Result<Json<BitPlaneData>, (StatusCode, String)> {
    let state = state.read().await;
    let buffer = state
        .buffer
        .as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;

    let samples = buffer.to_mono_samples();
    let total = samples.len() as f64;

    let planes: Vec<PlaneSummary> = (0..8u8)
        .map(|bit| {
            let mut ones = 0u64;
            for &s in &samples {
                let val = (s * 32767.0) as i16;
                if (val >> bit) & 1 == 1 {
                    ones += 1;
                }
            }
            let ones_ratio = ones as f64 / total;
            let bias = (ones_ratio - 0.5).abs();
            PlaneSummary {
                bit,
                ones_ratio,
                bias,
            }
        })
        .collect();

    Ok(Json(BitPlaneData { planes }))
}

// --- Audio serving endpoint ---

async fn serve_audio(State(state): State<SharedState>) -> Result<Response, (StatusCode, String)> {
    let state = state.read().await;
    let file_path = state
        .file_path
        .as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;

    let bytes = std::fs::read(file_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Read error: {e}"),
        )
    })?;

    let content_type = if file_path.ends_with(".mp3") {
        "audio/mpeg"
    } else {
        "audio/wav"
    };

    Ok(([(header::CONTENT_TYPE, content_type)], bytes).into_response())
}
