# Configuration Guide

Polez uses YAML configuration files with environment variable overrides and built-in presets.

## Config File Location

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/polez/config.yaml` |
| Linux | `~/.config/polez/config.yaml` |
| Windows | `%LOCALAPPDATA%\polez\config.yaml` |

Custom presets are stored as `preset_{name}.yaml` in the same directory.

## Complete Example Config

```yaml
version: "2.0.0"
paranoia_level: Medium      # Low, Medium, High, Maximum
preserve_quality: High       # Low, Medium, High, Maximum
output_format: Preserve      # Preserve, Mp3, Wav
backup_originals: false

audio_processing:
  sample_rate: ~             # null = preserve original
  bit_depth: ~               # null = preserve original
  channels: "preserve"       # "preserve", "mono", "stereo"
  normalize: true
  dithering: true

watermark_detection:
  - SpreadSpectrum
  - EchoBased
  - Statistical
  - PhaseModulation
  - AmplitudeModulation
  - FrequencyDomain

spectral_cleaning:
  high_freq_cutoff: 15000    # Hz - frequency above which cleaning is applied
  notch_filter_q: 30         # Q factor for notch filters
  smoothing_window: 5        # frames for spectral smoothing
  adaptive_noise: true       # add shaped noise to mask artifacts

metadata_cleaning:
  aggressive_mode: false     # strip all metadata vs selective
  preserve_date: false       # keep date-related tags
  preserve_technical: false  # keep technical tags (sample rate, etc.)
  strip_binary_chunks: true  # remove unknown binary chunks
  remove_id3v1: true
  remove_id3v2: true
  remove_ape_tags: true

fingerprint_removal:
  statistical_normalization: true   # adjust kurtosis to human range
  temporal_randomization: true      # add sub-sample timing jitter
  phase_randomization: false        # FFT-domain phase perturbation
  micro_timing_perturbation: true   # circular shift to break sync
  human_imperfections: false        # add analog-like imperfections

quality_preservation:
  target_snr: 40             # dB - minimum signal-to-noise ratio
  max_quality_loss: 5        # percent - maximum acceptable quality loss
  preserve_dynamics: true
  preserve_frequency_response: true

batch_processing:
  workers: 4                 # parallel processing threads
  progress_updates: true
  continue_on_error: false
  output_directory: ~        # null = same as input directory
  naming_pattern: "{name}_clean{ext}"

verification:
  auto_verify: true          # run verification after cleaning
  deep_analysis: false       # include statistical analysis
  compare_spectra: true
  check_watermarks: true
  calculate_metrics: true

ui:
  color_output: true
  unicode_symbols: true
  progress_bars: true
  detailed_output: false
  show_quotes: true
  ascii_art: true

formats:
  mp3:
    bitrate: "preserve"      # "preserve", "128", "192", "256", "320"
    quality: 2               # LAME quality 0 (best) to 9 (fastest)
    joint_stereo: true
  wav:
    bit_depth: "preserve"    # "preserve", "16", "24", "32"
    sample_format: "pcm"     # "pcm" or "float"

advanced_flags:
  phase_dither: true
  comb_mask: true
  transient_shift: true
  resample_nudge: true
  phase_noise: true
  phase_swirl: true
  masked_hf_phase: false
  gated_resample_nudge: false
  micro_eq_flutter: false
  hf_decorrelate: false
  refined_transient: false
  adaptive_transient: false
  adaptive_notch: false
```

## Config Sections Explained

### Top-Level Settings

- **paranoia_level**: Controls processing intensity. `Low` does minimal processing; `Maximum` enables multi-pass processing with stronger parameters.
- **preserve_quality**: Controls the quality/effectiveness trade-off. `Maximum` uses gentler parameters to minimize audio artifacts.
- **output_format**: `Preserve` keeps the original format. `Mp3` or `Wav` forces conversion.
- **backup_originals**: When true, saves a copy of the original file before cleaning.

### audio_processing

Controls how audio is loaded and saved. Set `sample_rate` or `bit_depth` to force conversion. `normalize` restores RMS level after processing. `dithering` adds TPDF dither when reducing bit depth.

### watermark_detection

List of detection algorithms to enable. Remove entries to skip specific detection methods. All six are enabled by default.

### spectral_cleaning

Controls frequency-domain watermark removal. `high_freq_cutoff` sets the frequency above which cleaning operations target. `notch_filter_q` controls how narrow the notch filters are (higher = narrower). `adaptive_noise` adds psychoacoustically shaped noise to mask processing artifacts.

### metadata_cleaning

Controls tag and chunk stripping. In non-aggressive mode, only suspicious tags are removed. `aggressive_mode` strips everything. Individual tag format removal can be toggled independently.

### fingerprint_removal

Five techniques for removing statistical fingerprints. `statistical_normalization` and `temporal_randomization` are the safest (enabled by default). `phase_randomization` and `human_imperfections` are more aggressive and disabled by default.

### quality_preservation

Sets quality bounds for the processing pipeline. `target_snr` is the minimum signal-to-noise ratio to maintain. `max_quality_loss` caps the acceptable quality degradation percentage.

### batch_processing

Controls the `sweep` command. `workers` sets thread count for parallel file processing. `naming_pattern` supports `{name}` (original filename without extension) and `{ext}` (original extension) placeholders.

### advanced_flags

Individual toggles for stealth DSP operations. The first six are enabled by default. The remaining seven are more aggressive and disabled by default. See [algorithms.md](algorithms.md) for details on each operation.

## Environment Variables

Environment variables override config file values. Values are case-insensitive.

| Variable | Values | Maps To |
|----------|--------|---------|
| `POLEZ_MODE` | `fast`, `standard`, `preserving`, `aggressive` | `paranoia_level` (Low/Medium/High/Maximum) |
| `POLEZ_QUALITY` | `low`, `medium`, `high`, `maximum` | `preserve_quality` |
| `POLEZ_OUTPUT_FORMAT` | `preserve`, `mp3`, `wav` | `output_format` |
| `POLEZ_PARANOID` | `1`, `true`, `yes` | Sets `paranoia_level` to Maximum |

Example:

```bash
POLEZ_MODE=aggressive POLEZ_QUALITY=high polez clean input.wav
```

## Built-in Presets

Manage presets with `polez config preset <name>` to apply, or `polez config list` to see available presets.

### stealth

Maximum paranoia with maximum quality preservation. Best for high-value audio where both thoroughness and quality matter.

```bash
polez config preset stealth
```

Settings: `paranoia_level: Maximum`, `preserve_quality: Maximum`, default advanced flags.

### stealth-plus

Optimized for detector evasion. Disables most stealth DSP operations except `gated_resample_nudge` and `phase_noise` for a minimal but effective processing footprint.

```bash
polez config preset stealth-plus
```

Settings: `paranoia_level: Maximum`, `preserve_quality: Maximum`, custom advanced flags (only `gated_resample_nudge` and `phase_noise` enabled).

### fast

Quick processing with basic cleaning. Useful for large batch jobs where speed matters more than thoroughness.

```bash
polez config preset fast
```

Settings: `paranoia_level: Low`, `preserve_quality: Medium`.

### quality

Prioritizes audio quality above all else. Uses medium paranoia to still clean effectively while maximizing quality preservation.

```bash
polez config preset quality
```

Settings: `paranoia_level: Medium`, `preserve_quality: Maximum`.

### research

Deep analysis mode with detailed logging. Useful for investigating audio files rather than cleaning them.

```bash
polez config preset research
```

Settings: `paranoia_level: High`, `preserve_quality: High`.

## Preset Recipes

### Music Production

For cleaning audio destined for release where quality is paramount:

```yaml
paranoia_level: Medium
preserve_quality: Maximum
output_format: Wav
quality_preservation:
  target_snr: 50
  max_quality_loss: 2
  preserve_dynamics: true
  preserve_frequency_response: true
fingerprint_removal:
  statistical_normalization: true
  temporal_randomization: true
  phase_randomization: false
  micro_timing_perturbation: false
  human_imperfections: false
advanced_flags:
  phase_dither: true
  comb_mask: false
  transient_shift: false
  resample_nudge: true
  phase_noise: true
  phase_swirl: false
  masked_hf_phase: false
  gated_resample_nudge: false
  micro_eq_flutter: false
  hf_decorrelate: false
  refined_transient: false
  adaptive_transient: false
  adaptive_notch: false
```

### Podcast Processing

For spoken-word content where subtle artifacts are less noticeable:

```yaml
paranoia_level: High
preserve_quality: High
output_format: Mp3
formats:
  mp3:
    bitrate: "192"
    quality: 2
    joint_stereo: true
metadata_cleaning:
  aggressive_mode: true
  strip_binary_chunks: true
  remove_id3v1: true
  remove_id3v2: true
  remove_ape_tags: true
fingerprint_removal:
  statistical_normalization: true
  temporal_randomization: true
  phase_randomization: true
  micro_timing_perturbation: true
  human_imperfections: true
```

### Forensic Analysis

For investigating files without modifying them:

```yaml
paranoia_level: High
preserve_quality: Maximum
verification:
  auto_verify: true
  deep_analysis: true
  compare_spectra: true
  check_watermarks: true
  calculate_metrics: true
ui:
  detailed_output: true
```

Use with the `detect` command:

```bash
polez detect --deep input.wav
```

## Config Management Commands

```bash
polez config list              # Show current config and available presets
polez config show              # Display full current configuration
polez config preset <name>     # Apply a built-in or custom preset
polez config create <name>     # Save current config as a custom preset
polez config delete <name>     # Delete a custom preset
polez config reset             # Reset to factory defaults
```
