# Polez

Audio forensics and sanitization engine. Analyzes, detects, and removes watermarks, metadata, and statistical fingerprints from audio files.

> *"The silent hand that scrubs the score"*

## Install

Requires Rust 1.70+.

```bash
cargo build --release
```

Binary outputs to `target/release/polez` (or `polez.exe` on Windows).

## Commands

### obliterate

Sanitize a single audio file.

```bash
polez obliterate input.mp3 -o clean.mp3
polez obliterate input.wav --paranoid --verify
polez obliterate input.mp3 --backup --format wav
```

| Flag | Description |
|------|-------------|
| `-o, --output` | Output file path (auto-generated if omitted) |
| `--paranoid` | Multi-pass aggressive cleaning |
| `--verify` | Re-analyze output and report removal effectiveness |
| `--backup` | Create backup of original |
| `-f, --format` | Output format: `preserve`, `mp3`, `wav` |

### massacre

Batch process a directory of audio files.

```bash
polez massacre ./music/
polez massacre ./music/ -d ./clean/ --paranoid --workers 8
polez massacre ./music/ -r --extension mp3 wav flac
polez massacre ./music/ --dry-run
```

| Flag | Description |
|------|-------------|
| `-d, --output-dir` | Output directory |
| `-e, --extension` | File extensions to process (default: mp3, wav) |
| `--paranoid` | Multi-pass aggressive cleaning |
| `-w, --workers` | Parallel worker count (default: 4) |
| `-r, --recursive` | Process subdirectories |
| `--dry-run` | List files without processing |
| `--backup` | Create backups |

### analyze

Detect watermarks and fingerprints without modifying the file.

```bash
polez analyze input.mp3
polez analyze input.wav --deep
```

| Flag | Description |
|------|-------------|
| `--deep` | Enable statistical analysis with AI probability scoring |

### config

Manage presets and configuration.

```bash
polez config list
polez config show
polez config preset stealth-plus
polez config create my-preset --paranoid high --quality maximum
polez config delete my-preset
polez config reset
```

Built-in presets: `stealth`, `stealth-plus`, `fast`, `quality`, `research`.

## Advanced Stealth Flags

Fine-grained control over individual sanitization operations (used with `obliterate`):

| Flag | Default | Description |
|------|---------|-------------|
| `--phase-dither` | on | Sub-block phase dither |
| `--comb-mask` | on | Dynamic comb masking |
| `--transient-shift` | on | Transient micro-shift |
| `--resample-nudge` | on | Resample nudge |
| `--phase-noise` | on | FFT phase noise |
| `--phase-swirl` | on | Phase swirl (allpass cascade) |
| `--masked-hf-phase` | off | Masked high-frequency phase noise |
| `--gated-resample-nudge` | off | RMS-gated resample nudge |
| `--micro-eq-flutter` | off | Micro-EQ flutter modulation |
| `--hf-decorrelate` | off | HF band decorrelation |
| `--refined-transient` | off | Refined transient micro-shift |
| `--adaptive-transient` | off | Adaptive onset-strength transient shift |

## Detection Methods

Polez uses 6 watermark detection algorithms:

- **Spread spectrum** — STFT high-frequency consistency analysis
- **Echo signatures** — Autocorrelation peak detection (1–50ms delays)
- **Statistical anomalies** — Entropy, kurtosis, and spectral entropy checks
- **Phase modulation** — STFT phase unwrapping consistency
- **Amplitude modulation** — Hilbert envelope FFT peak analysis
- **Frequency domain** — Spectral flatness and peak consistency

## Sanitization Pipeline

Four modes with increasing aggressiveness:

| Mode | What it does |
|------|-------------|
| **Fast** | Metadata stripping only |
| **Standard** | Spectral cleaning + fingerprint removal |
| **Preserving** | Standard + 20 stealth DSP operations |
| **Aggressive** | All operations with paranoid parameters |

The `--paranoid` flag forces Aggressive mode with 2 additional passes.

## Supported Formats

| Format | Read | Write |
|--------|------|-------|
| WAV | Yes | Yes |
| MP3 | Yes | Yes |

## License

Proprietary. All rights reserved.
