# Ralph Fix Plan

## High Priority (Bugs)

- [x] #2 - Replace unsafe `unwrap()` calls with proper error handling in `src/gui/routes.rs`
  - Replaced 4x `serde_json::to_value().unwrap()` with `map_err` in analyze endpoints
  - Replaced 2x `.as_ref().unwrap()` with `.ok_or(...)` in `clean_file`
- [x] #7 - Fix temp file memory leak from `Box::leak()` in `src/gui/routes.rs`
  - Added `temp_paths: Vec<PathBuf>` to `AppState` with `Drop` impl for cleanup
  - Replaced `Box::leak`/`mem::forget` with `tmp_path.keep()` + tracking in state

## High Priority (Code Quality)

- [x] #4 - Replace `println!/eprintln!` with structured logging (tracing crate)
  - Added tracing + tracing-subscriber deps, initialized in main()
  - Replaced gui/mod.rs println with tracing::info!, config/manager.rs eprintln with tracing::warn!
  - Note: src/ui/ and src/inspect/ println calls are intentional CLI user output, not logging

## Medium Priority (Features - Backend)

- [x] #6 - Expose FLAC and AAC format support in CLI
  - Added `flac` feature to symphonia, added Flac/Aac variants to AudioFormat
  - Generalized load_mp3 into load_symphonia for all symphonia-decoded formats
  - No FLAC/AAC encoders available; pipeline auto-falls back to WAV output
  - Updated sweep/benchmark default extensions to include flac/aac/m4a
- [x] #12 - Add `--dry-run` flag to clean command
  - Added `--dry-run` flag to Clean command in CLI
  - Runs full detection/analysis then exits before sanitization pipeline
- [x] #13 - Add export report feature (JSON and PDF)
  - Added `--report <path>` flag to both `clean` and `detect` commands
  - Exports JSON with file info, watermark, metadata, statistical, polez, and sanitization results
  - Made serde_json a non-optional dep; re-exported detection result types
  - PDF deferred (no easy pure-Rust PDF crate)
- [x] #19 - Add streaming support for large audio files
  - Added split_chunks/join_chunks to AudioBuffer with overlap-add crossfading
  - Pipeline auto-switches to chunked mode for files >5M samples (~60s at 44.1kHz)
  - Chunks: ~30s with 1s overlap, crossfaded back together after processing
  - Original buffer dropped before chunk processing to reduce peak memory

## Medium Priority (Features - GUI)

- [x] #1 - Add spectrogram visualization for cleaned audio
  - Extracted `compute_spectrogram` helper from `get_spectrogram` to avoid duplication
  - Added `/api/spectrogram/cleaned` endpoint using cleaned_buffer from state
  - Frontend rendering deferred to a separate frontend-focused issue
- [x] #5 - Add upload progress indicator and file size validation
  - Backend: added `/api/limits` endpoint returning max_upload_bytes and supported_formats
  - Frontend: replaced fetch with XHR for progress tracking, added progress bar UI
  - Client-side file size validation before upload with clear error message
  - Shows max file size hint in upload area
- [x] #10 - Display verification results and quality scores in GUI
  - Added VerificationResult type with SNR, spectral similarity, effectiveness, grade (A-F), verdict
  - Clean endpoint now runs verification::verify() and returns metrics inline
  - Frontend VerificationPanel shows grade badge, verdict, threat reduction, SNR, spectral similarity, effectiveness
- [ ] #11 - Add configuration and preset selection UI
- [ ] #14 - Add audio playback controls in GUI
- [ ] #9 - Add before/after audio comparison timeline
- [ ] #20 - Add detailed metadata tag viewer in GUI

## Low Priority (Infrastructure)

- [ ] #3 - Implement comprehensive test suite
  - Unit tests for detection modules, sanitization pipeline, DSP primitives
  - Integration tests for CLI commands
- [ ] #16 - Add React error boundaries for component crashes
- [ ] #8 - Add session persistence across page refreshes
- [ ] #17 - Add code coverage reporting (CI)
- [ ] #15 - Add release artifact publishing on git tags (CI)
- [ ] #18 - Optimize spectral operations to reuse STFT computations

## Completed
- [x] Project enabled for Ralph

## Working Notes
- All issues sourced from GitHub issues list (as of 2026-03-10)
- Issues labeled `ralph-task`: #2, #3, #4, #6, #7, #12, #15, #16, #17, #18
- After completing an issue, run `gh issue close <number>` to close it
- Always run `cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo fmt --all --check` before committing
- Commit format: conventional commits, single line, no co-authorship trailers
