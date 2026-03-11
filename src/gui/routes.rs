//! Axum route handlers for the GUI REST API.
//!
//! Provides endpoints for file upload, analysis, waveform/spectrogram data,
//! sanitization, verification, preset management, and batch processing.

use axum::extract::DefaultBodyLimit;
use axum::http::HeaderValue;
use axum::{
    extract::{Multipart, Query, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rustfft::num_complex::Complex64;
use rustfft::FftPlanner;
use serde::Deserialize;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

use crate::config::{
    defaults::{builtin_presets, default_config},
    AdvancedFlags,
};
use crate::detection::{MetadataScanner, PolezDetector, StatisticalAnalyzer, WatermarkDetector};
use crate::sanitization::pipeline::SanitizationMode;
use crate::sanitization::SanitizationPipeline;
use crate::verification;

use super::types::{
    AllAnalysisResult, BatchCleanResponse, BatchFileResult, BitPlaneData, CleanRequest,
    CleanResponse, FileInfo, PlaneSummary, PresetInfo, SpectrogramData, VerificationResult,
    WaveformData,
};
use super::SharedState;

#[derive(Clone)]
struct RateLimiter {
    tokens: Arc<AtomicU64>,
    max_tokens: u64,
}

impl RateLimiter {
    fn new(max_per_second: u64) -> Self {
        let limiter = Self {
            tokens: Arc::new(AtomicU64::new(max_per_second)),
            max_tokens: max_per_second,
        };
        let tokens = limiter.tokens.clone();
        let max = limiter.max_tokens;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                tokens.store(max, Ordering::Relaxed);
            }
        });
        limiter
    }
}

async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    // Atomically decrement using compare-exchange loop to prevent underflow.
    loop {
        let current = limiter.tokens.load(Ordering::Relaxed);
        if current == 0 {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, "1")],
                "Rate limit exceeded. Try again later.",
            )
                .into_response();
        }
        if limiter
            .tokens
            .compare_exchange_weak(current, current - 1, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            break;
        }
    }
    next.run(request).await
}

pub fn create_router(state: SharedState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            let origin = origin.to_str().unwrap_or("");
            origin.starts_with("http://localhost:")
                || origin.starts_with("http://127.0.0.1:")
                || origin == "http://localhost"
                || origin == "http://127.0.0.1"
        }))
        .allow_methods(Any)
        .allow_headers(Any);

    let limiter = RateLimiter::new(10);

    // Rate-limited processing endpoints (10 req/s → 429 when exceeded)
    let rate_limited_routes = Router::new()
        .route("/api/load", post(load_file))
        .route("/api/upload", post(upload_file))
        .route("/api/waveform", get(get_waveform))
        .route("/api/spectrogram", get(get_spectrogram))
        .route("/api/bitplane", get(get_bitplane))
        .route("/api/analyze/watermark", post(analyze_watermark))
        .route("/api/analyze/polez", post(analyze_polez))
        .route("/api/analyze/statistical", post(analyze_statistical))
        .route("/api/analyze/metadata", post(analyze_metadata))
        .route("/api/analyze/all", post(analyze_all))
        .route("/api/clean", post(clean_file))
        .route("/api/batch/clean", post(batch_clean))
        .route("/api/batch/download/{id}", get(batch_download))
        .route("/api/save", post(save_cleaned_file))
        .route_layer(middleware::from_fn_with_state(
            limiter,
            rate_limit_middleware,
        ));

    // Unrated routes (health, session, static assets, audio serving)
    let unrated_routes = Router::new()
        .route("/api/session", get(get_session))
        .route("/api/health", get(health))
        .route("/api/limits", get(get_limits))
        .route("/api/presets", get(list_presets))
        .route("/api/audio", get(serve_audio))
        .route("/api/audio/cleaned", get(serve_cleaned_audio))
        .route("/api/waveform/cleaned", get(get_cleaned_waveform))
        .route("/api/spectrogram/cleaned", get(get_cleaned_spectrogram));

    Router::new()
        .merge(rate_limited_routes)
        .merge(unrated_routes)
        .fallback(get(static_handler))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
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

async fn get_session(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let state = state.read().await;
    let file_info = state.buffer.as_ref().map(|buf| {
        serde_json::json!({
            "file_path": state.file_path,
            "format": state.format,
            "duration_secs": buf.duration_secs(),
            "sample_rate": buf.sample_rate,
            "channels": buf.num_channels(),
        })
    });
    let has_cleaned = state.cleaned_buffer.is_some();
    Json(serde_json::json!({
        "file_loaded": file_info.is_some(),
        "file_info": file_info,
        "has_cleaned": has_cleaned,
    }))
}

async fn health() -> &'static str {
    "ok"
}

const MAX_UPLOAD_BYTES: usize = 500 * 1024 * 1024; // 500MB

async fn get_limits() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "max_upload_bytes": MAX_UPLOAD_BYTES,
        "supported_formats": ["wav", "mp3", "flac", "aac", "m4a"]
    }))
}

async fn list_presets() -> Json<Vec<PresetInfo>> {
    let presets = builtin_presets()
        .into_iter()
        .map(|p| PresetInfo {
            name: p.name.to_string(),
            description: p.description.to_string(),
            builtin: true,
            paranoia_level: p.paranoia_level.to_string(),
            preserve_quality: p.preserve_quality.to_string(),
        })
        .collect();
    Json(presets)
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

    tracing::info!(
        action = "load_file",
        path = %req.path,
        format = %info.format,
        duration_secs = info.duration_secs,
        sample_rate = info.sample_rate,
        channels = info.channels,
    );

    let waveform_cache = super::WaveformCache::from_buffer(&buffer);

    let mut state = state.write().await;
    state.file_path = Some(req.path);
    state.format = Some(info.format.clone());
    state.waveform_cache = Some(waveform_cache);
    state.buffer = Some(buffer);

    Ok(Json(info))
}

async fn upload_file(
    State(state): State<SharedState>,
    mut multipart: Multipart,
) -> Result<Json<FileInfo>, (StatusCode, String)> {
    let field = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Multipart error: {e}")))?
        .ok_or((StatusCode::BAD_REQUEST, "No file field found".to_string()))?;

    let file_name = field.file_name().unwrap_or("upload.wav").to_string();

    let ext = Path::new(&file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("wav");

    let mut tmp = tempfile::Builder::new()
        .suffix(&format!(".{ext}"))
        .tempfile()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Temp file error: {e}"),
            )
        })?;

    let bytes = field
        .bytes()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Read error: {e}")))?;

    tmp.write_all(&bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Write error: {e}"),
        )
    })?;

    let tmp_path = tmp.into_temp_path();
    let persisted_path = tmp_path.to_path_buf();
    // Keep the file on disk by persisting the TempPath (consumes it without deleting)
    tmp_path.keep().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Temp path persist error: {e}"),
        )
    })?;
    let path_str = persisted_path.to_string_lossy().to_string();

    let (buffer, format) = crate::audio::load_audio(&persisted_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Load error: {e}"),
        )
    })?;

    let info = FileInfo {
        file_path: file_name,
        format: format.to_string(),
        duration_secs: buffer.duration_secs(),
        sample_rate: buffer.sample_rate,
        channels: buffer.num_channels(),
    };

    tracing::info!(
        action = "upload_file",
        filename = %info.file_path,
        format = %info.format,
        size_bytes = bytes.len(),
        duration_secs = info.duration_secs,
        sample_rate = info.sample_rate,
    );

    let waveform_cache = super::WaveformCache::from_buffer(&buffer);

    let mut app_state = state.write().await;
    app_state.temp_paths.push(persisted_path);
    app_state.file_path = Some(path_str);
    app_state.format = Some(info.format.clone());
    app_state.waveform_cache = Some(waveform_cache);
    app_state.buffer = Some(buffer);

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
    let value = serde_json::to_value(result).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Serialization error: {e}"),
        )
    })?;
    Ok(Json(value))
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
    let value = serde_json::to_value(result).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Serialization error: {e}"),
        )
    })?;
    Ok(Json(value))
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
    let value = serde_json::to_value(result).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Serialization error: {e}"),
        )
    })?;
    Ok(Json(value))
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
    let value = serde_json::to_value(result).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Serialization error: {e}"),
        )
    })?;
    Ok(Json(value))
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

    let width = query.width.unwrap_or(1024).max(1);
    let duration_secs = buffer.num_samples() as f64 / buffer.sample_rate as f64;
    let start_sec = query.start.unwrap_or(0.0).max(0.0);
    let end_sec = query.end.unwrap_or(duration_secs).min(duration_secs);

    // Use cached overview when resolution is sufficient (avoids to_mono_samples allocation)
    if let Some(cache) = &state.waveform_cache {
        let range_frac = (end_sec - start_sec) / cache.duration_secs;
        let cache_points_in_range = (range_frac * cache.min.len() as f64) as usize;

        // Cache is sufficient if it has at least as many points as requested
        if cache_points_in_range >= width {
            let (min_vals, max_vals) = cache.slice_for_range(start_sec, end_sec, width);
            return Ok(Json(WaveformData {
                min: min_vals,
                max: max_vals,
                sample_rate: cache.sample_rate,
                duration_secs: cache.duration_secs,
                channels: cache.channels,
            }));
        }
    }

    // High-zoom: compute from raw samples for the requested range only
    let samples = buffer.to_mono_samples();
    let sr = buffer.sample_rate as f64;
    let total_samples = samples.len();
    let start_idx = (start_sec * sr) as usize;
    let end_idx = ((end_sec * sr) as usize).min(total_samples);

    if start_idx >= end_idx {
        return Err((StatusCode::BAD_REQUEST, "Invalid time range".to_string()));
    }

    let slice = &samples[start_idx..end_idx];
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

fn compute_spectrogram(
    buffer: &crate::audio::AudioBuffer,
    query: &SpectrogramQuery,
) -> Result<SpectrogramData, (StatusCode, String)> {
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
                let db = 20.0 * mag.max(1e-12).log10();
                db.max(-120.0)
            })
            .collect();

        magnitudes.push(frame);
        pos += hop;
    }

    let num_time_frames = magnitudes.len();

    Ok(SpectrogramData {
        magnitudes,
        freq_min,
        freq_max,
        time_start: start_sec,
        time_end: end_sec,
        num_freq_bins,
        num_time_frames,
    })
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
    compute_spectrogram(buffer, &query).map(Json)
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

// --- Clean endpoint ---

async fn clean_file(
    State(state): State<SharedState>,
    Json(req): Json<CleanRequest>,
) -> Result<Json<CleanResponse>, (StatusCode, String)> {
    let mode = match req.mode.as_deref().unwrap_or("standard") {
        "fast" => SanitizationMode::Fast,
        "standard" => SanitizationMode::Standard,
        "preserving" => SanitizationMode::Preserving,
        "aggressive" => SanitizationMode::Aggressive,
        other => return Err((StatusCode::BAD_REQUEST, format!("Unknown mode: {other}"))),
    };

    // Read source info from state
    let (file_path, format_str) = {
        let s = state.read().await;
        let fp = s
            .file_path
            .clone()
            .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;
        let fmt = s
            .format
            .clone()
            .ok_or((StatusCode::BAD_REQUEST, "No file loaded".to_string()))?;
        (fp, fmt)
    };

    let ext = if format_str == "mp3" { "mp3" } else { "wav" };

    // Create temp output file
    let tmp = tempfile::Builder::new()
        .suffix(&format!("_cleaned.{ext}"))
        .tempfile()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Temp file error: {e}"),
            )
        })?;
    let tmp_path = tmp.into_temp_path();
    let output_path = tmp_path.to_path_buf();
    // Keep the file on disk; track for cleanup when state is dropped
    tmp_path.keep().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Temp path persist error: {e}"),
        )
    })?;

    let input_path = std::path::PathBuf::from(&file_path);
    let out = output_path.clone();

    // Build config from preset if specified, otherwise use defaults
    let preset_name = req.preset.clone();
    let req_adv_flags = req.advanced_flags;
    let req_fp_flags = req.fingerprint_flags;

    // Run sanitization in blocking thread (CPU-bound)
    let san_result = tokio::task::spawn_blocking(move || {
        let mut cfg = default_config();
        let mut flags = AdvancedFlags::default();

        if let Some(name) = &preset_name {
            if let Some(preset) = builtin_presets().into_iter().find(|p| p.name == name) {
                cfg.paranoia_level = preset.paranoia_level;
                cfg.preserve_quality = preset.preserve_quality;
                if let Some(preset_flags) = preset.advanced_flags {
                    flags = preset_flags;
                }
            }
        }

        // Override individual flags from request
        if let Some(af) = req_adv_flags {
            if let Some(v) = af.phase_dither {
                flags.phase_dither = v;
            }
            if let Some(v) = af.comb_mask {
                flags.comb_mask = v;
            }
            if let Some(v) = af.transient_shift {
                flags.transient_shift = v;
            }
            if let Some(v) = af.resample_nudge {
                flags.resample_nudge = v;
            }
            if let Some(v) = af.phase_noise {
                flags.phase_noise = v;
            }
            if let Some(v) = af.phase_swirl {
                flags.phase_swirl = v;
            }
            if let Some(v) = af.masked_hf_phase {
                flags.masked_hf_phase = v;
            }
            if let Some(v) = af.gated_resample_nudge {
                flags.gated_resample_nudge = v;
            }
            if let Some(v) = af.micro_eq_flutter {
                flags.micro_eq_flutter = v;
            }
            if let Some(v) = af.hf_decorrelate {
                flags.hf_decorrelate = v;
            }
            if let Some(v) = af.refined_transient {
                flags.refined_transient = v;
            }
            if let Some(v) = af.adaptive_transient {
                flags.adaptive_transient = v;
            }
            if let Some(v) = af.adaptive_notch {
                flags.adaptive_notch = v;
            }
        }

        let mut fp_config = cfg.fingerprint_removal;
        if let Some(ff) = req_fp_flags {
            if let Some(v) = ff.statistical_normalization {
                fp_config.statistical_normalization = v;
            }
            if let Some(v) = ff.temporal_randomization {
                fp_config.temporal_randomization = v;
            }
            if let Some(v) = ff.phase_randomization {
                fp_config.phase_randomization = v;
            }
            if let Some(v) = ff.micro_timing_perturbation {
                fp_config.micro_timing_perturbation = v;
            }
            if let Some(v) = ff.human_imperfections {
                fp_config.human_imperfections = v;
            }
        }

        let pipeline = SanitizationPipeline::new(
            mode,
            false,
            2,
            flags,
            fp_config,
            None,
            Vec::new(),
            None,
            None,
        );
        pipeline.run(&input_path, &out)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {e}"),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Sanitization error: {e}"),
        )
    })?;

    // Load cleaned audio
    let (cleaned_buffer, cleaned_fmt) = crate::audio::load_audio(&output_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Load cleaned error: {e}"),
        )
    })?;

    // Run detection on original (before)
    let before = {
        let s = state.read().await;
        let buf = s.buffer.as_ref().ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Original buffer no longer available".to_string(),
        ))?;
        let fp = s.file_path.as_ref().ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Original file path no longer available".to_string(),
        ))?;
        let watermark = WatermarkDetector::detect_all(buf);
        let polez = PolezDetector::detect(buf);
        let statistical = StatisticalAnalyzer::analyze(buf);
        let metadata = MetadataScanner::scan(Path::new(fp)).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Scan error: {e}"),
            )
        })?;
        AllAnalysisResult {
            watermark,
            polez,
            statistical,
            metadata,
        }
    };

    // Run detection on cleaned (after)
    let output_path_str = output_path.to_string_lossy().to_string();
    let watermark = WatermarkDetector::detect_all(&cleaned_buffer);
    let polez_result = PolezDetector::detect(&cleaned_buffer);
    let statistical = StatisticalAnalyzer::analyze(&cleaned_buffer);
    let metadata = MetadataScanner::scan(&output_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Scan error: {e}"),
        )
    })?;
    let after = AllAnalysisResult {
        watermark,
        polez: polez_result,
        statistical,
        metadata,
    };

    // Compute verification metrics
    let orig_path = std::path::PathBuf::from(&file_path);
    let ver_output = output_path.clone();
    let ver_result =
        tokio::task::spawn_blocking(move || verification::verify(&orig_path, &ver_output))
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Verification task error: {e}"),
                )
            })?
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Verification error: {e}"),
                )
            })?;

    let (verdict_text, verdict_color) =
        verification::verdict(ver_result.removal_effectiveness, ver_result.snr_db);

    let grade = match (ver_result.quality_score * 100.0) as u32 {
        90..=100 => "A",
        80..=89 => "B",
        70..=79 => "C",
        60..=69 => "D",
        _ => "F",
    };

    let verification = VerificationResult {
        original_threats: ver_result.original_threats,
        remaining_threats: ver_result.remaining_threats,
        removal_effectiveness: ver_result.removal_effectiveness,
        snr_db: ver_result.snr_db,
        spectral_similarity: ver_result.spectral_similarity,
        quality_score: ver_result.quality_score,
        grade: grade.to_string(),
        verdict: verdict_text.to_string(),
        verdict_color: verdict_color.to_string(),
    };

    // Store cleaned state and track temp file for cleanup
    {
        let cleaned_cache = super::WaveformCache::from_buffer(&cleaned_buffer);
        let mut s = state.write().await;
        s.temp_paths.push(output_path);
        s.cleaned_waveform_cache = Some(cleaned_cache);
        s.cleaned_buffer = Some(cleaned_buffer);
        s.cleaned_file_path = Some(output_path_str);
        s.cleaned_format = Some(cleaned_fmt.to_string());
    }

    tracing::info!(
        action = "clean_file",
        source = %file_path,
        mode = %req.mode.as_deref().unwrap_or("standard"),
        success = san_result.success,
        quality_loss = san_result.quality_loss,
        processing_time = san_result.processing_time,
        patterns_found = san_result.patterns_found,
        patterns_suppressed = san_result.patterns_suppressed,
    );

    Ok(Json(CleanResponse {
        success: san_result.success,
        metadata_removed: san_result.metadata_removed,
        patterns_found: san_result.patterns_found,
        patterns_suppressed: san_result.patterns_suppressed,
        quality_loss: san_result.quality_loss,
        processing_time: san_result.processing_time,
        before,
        after,
        verification,
    }))
}

// --- Serve cleaned audio ---

async fn serve_cleaned_audio(
    State(state): State<SharedState>,
) -> Result<Response, (StatusCode, String)> {
    let state = state.read().await;
    let file_path = state.cleaned_file_path.as_ref().ok_or((
        StatusCode::BAD_REQUEST,
        "No cleaned file available".to_string(),
    ))?;

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

// --- Cleaned waveform endpoint ---

async fn get_cleaned_waveform(
    State(state): State<SharedState>,
    Query(query): Query<WaveformQuery>,
) -> Result<Json<WaveformData>, (StatusCode, String)> {
    let state = state.read().await;
    let buffer = state.cleaned_buffer.as_ref().ok_or((
        StatusCode::BAD_REQUEST,
        "No cleaned file available".to_string(),
    ))?;

    let width = query.width.unwrap_or(1024).max(1);
    let duration_secs = buffer.num_samples() as f64 / buffer.sample_rate as f64;
    let start_sec = query.start.unwrap_or(0.0).max(0.0);
    let end_sec = query.end.unwrap_or(duration_secs).min(duration_secs);

    // Use cached overview when resolution is sufficient
    if let Some(cache) = &state.cleaned_waveform_cache {
        let range_frac = (end_sec - start_sec) / cache.duration_secs;
        let cache_points_in_range = (range_frac * cache.min.len() as f64) as usize;

        if cache_points_in_range >= width {
            let (min_vals, max_vals) = cache.slice_for_range(start_sec, end_sec, width);
            return Ok(Json(WaveformData {
                min: min_vals,
                max: max_vals,
                sample_rate: cache.sample_rate,
                duration_secs: cache.duration_secs,
                channels: cache.channels,
            }));
        }
    }

    // High-zoom: compute from raw samples
    let samples = buffer.to_mono_samples();
    let sr = buffer.sample_rate as f64;
    let total_samples = samples.len();
    let start_idx = (start_sec * sr) as usize;
    let end_idx = ((end_sec * sr) as usize).min(total_samples);

    if start_idx >= end_idx {
        return Err((StatusCode::BAD_REQUEST, "Invalid time range".to_string()));
    }

    let slice = &samples[start_idx..end_idx];
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

// --- Cleaned spectrogram endpoint ---

async fn get_cleaned_spectrogram(
    State(state): State<SharedState>,
    Query(query): Query<SpectrogramQuery>,
) -> Result<Json<SpectrogramData>, (StatusCode, String)> {
    let state = state.read().await;
    let buffer = state.cleaned_buffer.as_ref().ok_or((
        StatusCode::BAD_REQUEST,
        "No cleaned file available".to_string(),
    ))?;
    compute_spectrogram(buffer, &query).map(Json)
}

// --- Save/download cleaned file ---

async fn save_cleaned_file(
    State(state): State<SharedState>,
) -> Result<Response, (StatusCode, String)> {
    let state = state.read().await;
    let file_path = state.cleaned_file_path.as_ref().ok_or((
        StatusCode::BAD_REQUEST,
        "No cleaned file available".to_string(),
    ))?;

    let bytes = std::fs::read(file_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Read error: {e}"),
        )
    })?;

    let ext = if file_path.ends_with(".mp3") {
        "mp3"
    } else {
        "wav"
    };

    let filename = format!("cleaned_output.{ext}");
    let content_type = if ext == "mp3" {
        "audio/mpeg"
    } else {
        "audio/wav"
    };

    tracing::info!(
        action = "download_cleaned",
        filename = %filename,
        size_bytes = bytes.len(),
    );

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn batch_clean(
    State(state): State<SharedState>,
    Query(query): Query<super::types::BatchCleanQuery>,
    mut multipart: Multipart,
) -> Result<Json<BatchCleanResponse>, (StatusCode, String)> {
    let mode = match query.mode.as_deref().unwrap_or("standard") {
        "fast" => SanitizationMode::Fast,
        "standard" => SanitizationMode::Standard,
        "preserving" => SanitizationMode::Preserving,
        "aggressive" => SanitizationMode::Aggressive,
        other => return Err((StatusCode::BAD_REQUEST, format!("Unknown mode: {other}"))),
    };

    let mut results = Vec::new();
    let mut download_ids = std::collections::HashMap::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Multipart read error: {e}"),
        )
    })? {
        let filename = field.file_name().unwrap_or("unknown").to_string();
        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("wav")
            .to_string();

        let data = field.bytes().await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to read file data: {e}"),
            )
        })?;

        // Write to temp input file
        let input_tmp = tempfile::Builder::new()
            .suffix(&format!(".{ext}"))
            .tempfile()
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Temp file error: {e}"),
                )
            })?;
        let input_path = input_tmp.into_temp_path();
        std::fs::write(&input_path, &data).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Write error: {e}"),
            )
        })?;

        // Create output temp file
        let output_tmp = tempfile::Builder::new()
            .suffix(&format!("_cleaned.{ext}"))
            .tempfile()
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Temp file error: {e}"),
                )
            })?;
        let output_tmp_path = output_tmp.into_temp_path();
        let output_path = output_tmp_path.to_path_buf();
        output_tmp_path.keep().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Temp path error: {e}"),
            )
        })?;

        let input_buf = input_path.to_path_buf();
        let out_buf = output_path.clone();
        let file_mode = mode;

        let result = tokio::task::spawn_blocking(move || {
            let cfg = default_config();
            let flags = AdvancedFlags::default();
            let pipeline = SanitizationPipeline::new(
                file_mode,
                false,
                2,
                flags,
                cfg.fingerprint_removal,
                None,
                Vec::new(),
                None,
                None,
            );
            let start = std::time::Instant::now();
            let run_result = pipeline.run(&input_buf, &out_buf);
            (run_result, start.elapsed())
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Task error: {e}"),
            )
        })?;

        match result {
            (Ok(san_result), elapsed) => {
                let id = uuid_simple();
                {
                    let mut s = state.write().await;
                    s.temp_paths.push(output_path.clone());
                    s.temp_paths.push(input_path.to_path_buf());
                }
                download_ids.insert(filename.clone(), id.clone());
                // Store download path with timestamp for TTL-based cleanup
                if let Ok(mut map) = BATCH_DOWNLOADS.lock() {
                    purge_stale_downloads(&mut map);
                    map.insert(id, (output_path, std::time::Instant::now()));
                }
                results.push(BatchFileResult {
                    filename,
                    success: true,
                    error: None,
                    quality_loss: Some(san_result.quality_loss),
                    processing_time: Some(elapsed.as_secs_f64()),
                });
            }
            (Err(e), _) => {
                results.push(BatchFileResult {
                    filename,
                    success: false,
                    error: Some(e.to_string()),
                    quality_loss: None,
                    processing_time: None,
                });
            }
        }
    }

    let success_count = results.iter().filter(|r| r.success).count();
    let fail_count = results.len() - success_count;
    tracing::info!(
        action = "batch_clean",
        total = results.len(),
        success = success_count,
        failed = fail_count,
        mode = %query.mode.as_deref().unwrap_or("standard"),
    );

    Ok(Json(BatchCleanResponse {
        results,
        download_ids,
    }))
}

static BATCH_DOWNLOADS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, (std::path::PathBuf, std::time::Instant)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

const BATCH_DOWNLOAD_TTL: std::time::Duration = std::time::Duration::from_secs(3600);

fn purge_stale_downloads(
    map: &mut std::collections::HashMap<String, (std::path::PathBuf, std::time::Instant)>,
) {
    let now = std::time::Instant::now();
    map.retain(|_, (_, created)| now.duration_since(*created) < BATCH_DOWNLOAD_TTL);
}

fn uuid_simple() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:016x}{:016x}", rng.gen::<u64>(), rng.gen::<u64>())
}

async fn batch_download(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let path = BATCH_DOWNLOADS
        .lock()
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Download state unavailable".to_string(),
            )
        })?
        .remove(&id)
        .map(|(p, _)| p)
        .ok_or((StatusCode::NOT_FOUND, "Download not found".to_string()))?;

    let bytes = std::fs::read(&path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Read error: {e}"),
        )
    })?;

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("cleaned.wav");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("wav");
    let content_type = if ext == "mp3" {
        "audio/mpeg"
    } else {
        "audio/wav"
    };

    tracing::info!(
        action = "batch_download",
        filename = %filename,
        size_bytes = bytes.len(),
    );

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tokio::sync::RwLock;
    use tower::ServiceExt;

    fn test_state() -> SharedState {
        Arc::new(RwLock::new(super::super::AppState::new()))
    }

    fn test_app() -> Router {
        create_router(test_state())
    }

    async fn state_with_audio() -> SharedState {
        let state = Arc::new(RwLock::new(super::super::AppState::new()));
        let samples: Vec<f32> = (0..4410)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let buffer = crate::audio::AudioBuffer::from_mono(samples, 44100);
        let cache = super::super::WaveformCache::from_buffer(&buffer);
        let mut guard = state.write().await;
        guard.buffer = Some(buffer);
        guard.file_path = Some("/tmp/test.wav".to_string());
        guard.format = Some("wav".to_string());
        guard.waveform_cache = Some(cache);
        drop(guard);
        state
    }

    async fn app_with_audio() -> Router {
        create_router(state_with_audio().await)
    }

    async fn get_status(app: Router, uri: &str) -> StatusCode {
        let req = axum::http::Request::builder()
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        app.oneshot(req).await.unwrap().status()
    }

    async fn get_body(app: Router, uri: &str) -> (StatusCode, String) {
        let req = axum::http::Request::builder()
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&body).to_string())
    }

    async fn post_json(app: Router, uri: &str, body: &str) -> (StatusCode, String) {
        let req = axum::http::Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    #[tokio::test]
    async fn test_health() {
        let (status, body) = get_body(test_app(), "/api/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn test_limits() {
        let (status, body) = get_body(test_app(), "/api/limits").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["max_upload_bytes"], 500 * 1024 * 1024);
        assert!(json["supported_formats"].is_array());
    }

    #[tokio::test]
    async fn test_presets() {
        let (status, body) = get_body(test_app(), "/api/presets").await;
        assert_eq!(status, StatusCode::OK);
        let json: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(!json.is_empty());
        assert!(json[0]["name"].is_string());
    }

    #[tokio::test]
    async fn test_session_no_file() {
        let (status, body) = get_body(test_app(), "/api/session").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["file_loaded"], false);
        assert!(json["file_info"].is_null());
    }

    #[tokio::test]
    async fn test_session_with_file() {
        let (status, body) = get_body(app_with_audio().await, "/api/session").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["file_loaded"], true);
        assert_eq!(json["file_info"]["sample_rate"], 44100);
    }

    #[tokio::test]
    async fn test_waveform_no_file() {
        let status = get_status(test_app(), "/api/waveform?width=512").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_waveform_with_file() {
        let (status, body) = get_body(app_with_audio().await, "/api/waveform?width=256").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["min"].is_array());
        assert!(json["max"].is_array());
        assert_eq!(json["sample_rate"], 44100);
    }

    #[tokio::test]
    async fn test_spectrogram_no_file() {
        let status = get_status(test_app(), "/api/spectrogram").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_spectrogram_with_file() {
        let (status, body) =
            get_body(app_with_audio().await, "/api/spectrogram?fft_size=256").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["magnitudes"].is_array());
        assert!(json["num_freq_bins"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_bitplane_no_file() {
        let status = get_status(test_app(), "/api/bitplane").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_bitplane_with_file() {
        let (status, body) = get_body(app_with_audio().await, "/api/bitplane").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["planes"].as_array().unwrap().len(), 8);
    }

    #[tokio::test]
    async fn test_analyze_watermark_no_file() {
        let (status, _) = post_json(test_app(), "/api/analyze/watermark", "").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_analyze_watermark_with_file() {
        let (status, body) = post_json(app_with_audio().await, "/api/analyze/watermark", "").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["detected"].is_array());
    }

    #[tokio::test]
    async fn test_analyze_polez_no_file() {
        let (status, _) = post_json(test_app(), "/api/analyze/polez", "").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_analyze_polez_with_file() {
        let (status, body) = post_json(app_with_audio().await, "/api/analyze/polez", "").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["signals"].is_object());
    }

    #[tokio::test]
    async fn test_analyze_statistical_no_file() {
        let (status, _) = post_json(test_app(), "/api/analyze/statistical", "").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_analyze_statistical_with_file() {
        let (status, body) =
            post_json(app_with_audio().await, "/api/analyze/statistical", "").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json.is_object());
    }

    #[tokio::test]
    async fn test_analyze_metadata_no_file() {
        let (status, _) = post_json(test_app(), "/api/analyze/metadata", "").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_analyze_all_no_file() {
        let (status, _) = post_json(test_app(), "/api/analyze/all", "").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_serve_audio_no_file() {
        let status = get_status(test_app(), "/api/audio").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_serve_cleaned_audio_no_file() {
        let status = get_status(test_app(), "/api/audio/cleaned").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_cleaned_waveform_no_file() {
        let status = get_status(test_app(), "/api/waveform/cleaned").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_cleaned_spectrogram_no_file() {
        let status = get_status(test_app(), "/api/spectrogram/cleaned").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_load_nonexistent_file() {
        let (status, _) =
            post_json(test_app(), "/api/load", r#"{"path":"/nonexistent.wav"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_clean_no_file() {
        let (status, _) = post_json(test_app(), "/api/clean", r#"{"mode":"fast"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_save_no_cleaned_file() {
        let (status, _) = post_json(test_app(), "/api/save", r#"{"path":"/tmp/out.wav"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_batch_download_not_found() {
        let status = get_status(test_app(), "/api/batch/download/nonexistent").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_static_fallback_returns_response() {
        // Requesting unknown path should hit the static handler (404 or index)
        let status = get_status(test_app(), "/unknown-path").await;
        // Without built frontend assets, this will be NOT_FOUND
        assert!(status == StatusCode::OK || status == StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_rate_limiter_enforces_limit_under_concurrency() {
        let limiter = RateLimiter::new(5);
        let tokens = limiter.tokens.clone();

        // Spawn 20 concurrent tasks all trying to decrement
        let mut handles = Vec::new();
        for _ in 0..20 {
            let t = tokens.clone();
            handles.push(tokio::spawn(async move {
                loop {
                    let current = t.load(Ordering::Relaxed);
                    if current == 0 {
                        return false; // rejected
                    }
                    if t.compare_exchange_weak(
                        current,
                        current - 1,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                    {
                        return true; // accepted
                    }
                }
            }));
        }

        let mut accepted = 0;
        for h in handles {
            if h.await.unwrap() {
                accepted += 1;
            }
        }

        // Exactly 5 should be accepted (the initial token count)
        assert_eq!(accepted, 5, "Expected exactly 5 accepted, got {accepted}");

        // Token count must never wrap — should be 0, not u64::MAX
        let final_val = tokens.load(Ordering::Relaxed);
        assert_eq!(final_val, 0, "Tokens should be 0, got {final_val}");
    }
}
