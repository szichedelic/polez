# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build release binary (outputs to target/release/polez.exe on Windows)
cargo build --release

# Run checks
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# GUI development
cd gui && npm run dev          # Start Vite dev server (port 5173)
cargo run -- gui               # Start backend (port 3000)
cd gui && npm run build        # Build frontend for embedding
cargo build --release          # Build with embedded frontend
./target/release/polez gui     # Run GUI (opens browser)
gui/build.sh                   # Build frontend + release binary
```

## Architecture

Polez is an audio forensics and sanitization CLI tool written in Rust. It detects and removes watermarks, metadata, and statistical fingerprints from audio files.

### Module Structure

- **cli/** - Command-line interface using clap with derive macros
  - `flags.rs` - Advanced stealth flag definitions (12+ DSP operation toggles)

- **audio/** - Audio I/O and buffer management
  - `buffer.rs` - `AudioBuffer` type for multi-channel f32 sample data
  - `io.rs` - Load/save using symphonia (decode) and hound/mp3lame (encode)

- **detection/** - Watermark and fingerprint detection (6 algorithms)
  - `watermark.rs` - `WatermarkDetector` with spread spectrum, echo, phase modulation, etc.
  - `statistical.rs` - `StatisticalAnalyzer` for entropy/kurtosis analysis and AI probability scoring
  - `metadata_scan.rs` - `MetadataScanner` using lofty for tag/chunk detection

- **sanitization/** - Audio cleaning pipeline
  - `pipeline.rs` - `SanitizationPipeline` orchestrates 4 modes: Fast, Standard, Preserving, Aggressive
  - `spectral.rs` - `SpectralCleaner` for frequency-domain cleaning
  - `fingerprint.rs` - `FingerprintRemover` for pattern suppression
  - `stealth.rs` - `StealthOps` applies 12 DSP operations (phase dither, comb mask, transient shift, etc.)
  - `metadata.rs` - `MetadataCleaner` strips tags via lofty
  - **dsp/** - Low-level DSP primitives (STFT, biquad filters, Hilbert transform, resampling, filtfilt)

- **config/** - YAML-based configuration and presets
  - `types.rs` - Config structs (`AppConfig`, `AdvancedFlags`, `ParanoiaLevel`, etc.)
  - `defaults.rs` - Built-in presets (stealth, stealth-plus, fast, quality, research)
  - `manager.rs` - `ConfigManager` handles config file I/O in user config directory

- **verification/** - Post-processing verification comparing before/after analysis

- **gui/** - Web GUI backend (behind `gui` feature flag)
  - `mod.rs` - Axum server setup, `AppState`, rust-embed static serving
  - `routes.rs` - REST API endpoints (load, analyze, waveform, spectrogram, bitplane)
  - `types.rs` - API request/response types

- **ui/** - Console output, banners, and progress bars using indicatif/console/colored

### Key Types

- `AudioBuffer` - Core audio data container with f32 samples, sample rate, RMS/normalization helpers
- `SanitizationPipeline` - Unified pipeline replacing 5 separate Python sanitizers from prior version
- `SanitizationMode` - Enum controlling processing intensity (Fast/Standard/Preserving/Aggressive)
- `AdvancedFlags` - 12 boolean toggles for individual stealth DSP operations

### CLI Commands

- `clean` - Single file sanitization
- `sweep` - Batch directory processing with parallel workers (rayon)
- `detect` - Detection-only mode with optional `--deep` statistical analysis
- `config` - Preset management (list/show/preset/create/delete/reset)
- `gui` - Web-based forensics GUI (axum + embedded React SPA)

### Processing Pipeline Flow

1. Metadata stripping (lofty)
2. Load audio (symphonia decode)
3. DSP processing based on mode (spectral cleaning, fingerprint removal, stealth ops)
4. Optional paranoid multi-pass (2 additional passes)
5. RMS restoration and soft clipping
6. Save output (hound/mp3lame encode)
