# Algorithm Reference

This document describes the detection and sanitization algorithms used by Polez, their parameters, and operational characteristics.

## Detection Methods

Polez uses 10 watermark detection methods, statistical AI probability scoring, AI-specific watermark detection, and metadata scanning.

### Watermark Detection

The `WatermarkDetector` runs up to 10 independent detection methods. Each returns a confidence score (0.0-1.0) and a detected/not-detected flag.

#### Spread Spectrum

Analyzes high-frequency energy consistency above 15 kHz using STFT. Computes the ratio of standard deviation to mean power across high-frequency bins. A consistency score above 0.7 suggests embedded spread-spectrum watermarks. Also checks for suspicious energy at carrier frequencies (18, 19, 20, 21 kHz) where power exceeds mean + 3 standard deviations.

**False positives:** Audio with naturally strong high-frequency content (cymbals, synthesizers).

#### Echo Signatures

Performs autocorrelation on the first 50 ms of signal to find echo patterns. Tests delays in the 1-50 ms range with a minimum strength ratio of 0.1. Requires two or more consistent echo delays for detection. Measures interval consistency; triggers if consistency exceeds 0.8.

**False positives:** Audio recorded in small reflective rooms or with intentional delay effects.

#### Statistical Anomalies

Checks four statistical properties:
- **Entropy** < 6.0 (confidence 0.7) - low entropy suggests structured data
- **Kurtosis** deviation from 3.0 > 2.0 (confidence 0.6) - non-Gaussian distribution
- **Skewness** > 0.5 (confidence 0.5) - asymmetric distribution
- **Spectral entropy** < 8.0 (confidence 0.5) - unusually structured spectrum

**False negatives:** Well-designed watermarks that preserve statistical properties.

#### Phase Modulation

Uses STFT (2048-sample window, 75% overlap) to track phase evolution across frames. For each frequency bin, measures the standard deviation of frame-to-frame phase differences. High consistency (score > 0.7) in phase evolution suggests embedded phase modulation patterns.

**False positives:** Tonal audio with stable pitch.

#### Amplitude Modulation

Computes the signal envelope via Hilbert transform, then analyzes the modulation spectrum (1-100 Hz). Detection triggers when more than 5 peaks appear in the modulation spectrum, suggesting embedded AM patterns.

**False positives:** Tremolo effects, amplitude-modulated synthesis.

#### Frequency Domain

Tests multiple STFT window sizes (512, 1024, 2048, 4096). Per window, computes spectral flatness per frame. Flags if average flatness exceeds 0.3 or peak consistency across frames exceeds 0.8.

**False positives:** White noise or broadband signals.

#### LSB Steganography

Four tests on least-significant bits:

1. **Bias test**: Checks if LSB ones-ratio deviates from 0.5 by more than 0.02
2. **Chi-squared test**: Analyzes LSB pair distribution against uniform expectation (threshold p=0.05, df=3)
3. **Periodicity test**: Tests LSB autocorrelation at common embedding lags (128, 256, 441, 512, 576, 1024, 1152, 2048, 2304, 4096, 4608)
4. **Runs test**: Z-score of run length distribution; flags if z > 2.58

**False positives:** Dithered audio or audio with low bit depth.

#### Codec Artifacts

Three tests for re-encoding signatures:

1. **MP3 frame boundaries**: Tests 1152-sample periodicity; flags if energy discontinuity at boundaries exceeds 1.3x the interior average
2. **Frequency cutoff**: Fits log-linear model to 15-20 kHz rolloff; matches against known codec profiles (MP3 128/320 kbps, AAC at various bitrates)
3. **Spectral band replication (SBR)**: Correlates lower vs upper frequency bands; flags if correlation exceeds 0.5

#### Phase Coherence

For stereo audio, analyzes inter-channel phase difference stability across four frequency bands (100 Hz-1 kHz, 1-4 kHz, 4-8 kHz, 8-16 kHz). Flags if more than 30% of bins have phase difference standard deviation below 0.1 (unnaturally phase-locked).

For mono audio, analyzes temporal phase consistency across 8-sample segments. Flags if average coherence exceeds 0.7 with standard deviation below 0.1.

#### Spatial Encoding

M/S (mid-side) analysis on stereo audio. Computes mid/side ratios across four bands (low, mid, high, ultrasonic). Flags if ratio standard deviation is below 0.02 with average ratio between 0.01 and 0.5 (unnaturally stable stereo image).

Also checks inter-channel correlation above 12 kHz; localized high correlation suggests embedded watermarks.

### AI Probability Scoring

The `StatisticalAnalyzer` combines AI-specific indicators (60% weight) with classic statistical features (40% weight) to estimate the probability that audio was AI-generated.

#### AI-Specific Indicators (60%)

| Indicator | Weight | What It Measures |
|-----------|--------|------------------|
| Spectral continuity | 20% | Frame-to-frame spectral difference consistency; AI audio has unnaturally smooth transitions |
| Micro-silence patterns | 15% | Periodic energy dips at 5-50 ms intervals; AI models often produce regular micro-silences |
| Harmonic regularity | 15% | Consistency of harmonic ratios across frames; AI has unnaturally stable overtones |
| Onset precision | 10% | Rise time consistency across detected onsets; AI produces very regular attack times |

#### Classic Features (40%)

| Feature | Weight | Natural Range |
|---------|--------|--------------|
| Entropy | 10% | 6.0-10.0 |
| Kurtosis | 10% | 1.5-6.0 |
| Skewness | 10% | -0.5 to 0.5 |
| Spectral flatness | 10% | Varies by content |

### Polez AI Watermark Detection

The `PolezDetector` targets AI-service-specific watermarks using three weighted signals:

| Signal | Weight | Method |
|--------|--------|--------|
| Ultrasonic energy | 45% | DFT energy ratio in 23-24 kHz vs 15-20 kHz reference band. Requires 48 kHz+ sample rate. AI watermark ratio > 0.1; human audio < 0.02. |
| Bit plane bias | 35% | Analyzes 8 bit planes of 16-bit PCM. AI watermarks bias 6-8 planes; human audio biases 0-2. Threshold: deviation from 50% > 1%. |
| Autocorrelation | 20% | Tests LSB autocorrelation at periods 2-1024. AI watermarks show correlation > 0.05 at period 2. |

Confidence is based on signal agreement (lower variance between scores = higher confidence). Final probability uses sigmoid calibration to push values away from 0.5.

### Metadata Scanning

The `MetadataScanner` checks two areas:

**Tag keys flagged:** unique, fingerprint, identifier, tracking, license, isrc, barcode, upc, ean, catalog, txxx

**Tag values flagged:** suno, udio, audiocraft, musicgen, stable audio, ai-generated, generated by

**Binary patterns:** Searches raw bytes for AI service markers (SUNO, UDIO, AudioCraft, MusicGen, Stable Audio) and tag headers (APETAGEX, ID3).

## Sanitization Modes

| Mode | Operations | Use Case |
|------|-----------|----------|
| Fast | Metadata stripping only | Quick metadata removal without audio modification |
| Standard | Metadata + spectral cleaning + fingerprint removal | Balanced cleaning for most files |
| Preserving | Standard + all stealth DSP operations | Maximum cleaning with quality preservation focus |
| Aggressive | All operations with stronger parameters | Maximum disruption when quality is secondary |

For files exceeding ~5.3M samples, processing is chunked (~30 s chunks with 1 s overlap) and parallelized with rayon.

### Spectral Cleaning

The `SpectralCleaner` operates in the STFT domain targeting known watermark frequency bands:

- 18-18.5 kHz
- 19-19.5 kHz
- 20-20.5 kHz
- 21-21.5 kHz

Four operations run in a single STFT pass:

1. **Periodic disruption**: Phase randomization above 15 kHz (normal: +/-0.02 rad, aggressive: +/-0.05 rad)
2. **Spectral smoothing**: 5-bin moving average on magnitude above 15 kHz
3. **Spread-spectrum attenuation**: 0.8x magnitude scaling in high-frequency bins
4. **Adaptive noise shaping**: High-pass filtered noise addition (8 kHz cutoff, normal: 9e-9, aggressive: 1.8e-8)

All operations respect psychoacoustic masking thresholds to avoid audible artifacts.

**Adaptive notch pass** (post-processing): Scans 15-20 kHz for ultrasonic peaks exceeding 1.3x the predicted rolloff. Applies cascaded lowpass and notch filters (Q=3-5) to surgically remove detected peaks.

### Fingerprint Removal

The `FingerprintRemover` applies five configurable techniques:

1. **Statistical normalization**: Adjusts kurtosis toward the 1.5-4.0 human range using cubic soft expansion or compression (strength 0.01)
2. **Temporal randomization**: Interpolated sample jitter (normal: sigma=0.1, aggressive: sigma=0.15 samples)
3. **Phase randomization**: FFT-domain phase perturbation (normal: sigma=0.01 rad, aggressive: sigma=0.015 rad)
4. **Micro-timing perturbation**: Circular shift by 0-1 ms to break timing synchronization
5. **Human imperfections**: Velocity variation (sigma=0.002), drift with decay (sigma=0.0001), and soft even-harmonic distortion (0.0001x * |x|, mimicking analog saturation)

### Stealth DSP Operations

Twenty flag-gated operations applied in Preserving and Aggressive modes. Each has normal and paranoid (aggressive) parameter levels.

| Operation | Description | Normal | Paranoid |
|-----------|-------------|--------|----------|
| Phase dither | Sub-block (512-sample) FFT phase perturbation | +/-0.02 rad | +/-0.04 rad |
| Comb mask | Dynamic notch filters above 15 kHz at random harmonics | 3 notches, Q=10 | 5 notches, Q=10 |
| Transient shift | Sub-sample onset shifts detected via energy rise | +/-0.08 ms | +/-0.1 ms |
| Resample nudge | Subtle sample rate warping | +/-0.035% | +/-0.06% |
| Phase noise | FFT-domain full-spectrum phase perturbation | sigma=0.05 rad | sigma=0.08 rad |
| Phase swirl | Allpass cascade at 2 kHz and 5 kHz | alpha=[0.012, -0.01] | alpha=[0.016, -0.014] |
| Masked HF phase | Phase noise restricted to > 15.5 kHz | +/-0.10 rad | +/-0.15 rad |
| Gated resample nudge | RMS-gated sample rate warping (> 0.01 RMS) | 0.025% | 0.04% |
| Micro EQ flutter | Time-varying peaking EQ at 3 kHz | +/-0.01 dB, 0.3 Hz | +/-0.015 dB |
| HF decorrelate | Phase randomization in 13-17 kHz band | 13-17 kHz | 12-16 kHz |
| Refined transient | Gaussian-distributed onset shifts with crossfade | +/-0.08 ms | +/-0.12 ms |
| Adaptive transient | Onset-strength-gated shifting | 0.10 ms max | 0.15 ms max |
| Adaptive notch | Scan-based ultrasonic peak removal with cascaded filters | 2 cascades | 3 cascades |
| Spectral phase noise | Band-limited phase noise below 10 kHz | +/-0.08 rad | +/-0.12 rad |
| HF noise and dither | High-pass (10 kHz) noise + TPDF dither | 9e-8 + 2e-6 | 1.8e-7 + 4e-6 |
| Humanization | Wow/flutter simulation (tape machine imperfections) | 0.15 Hz, 5 samples | 0.21 Hz, 8 samples |
| Micro resample warp | Resample up then down for timing distortion | +/-0.15% | +/-0.22% |
| Analog warmth | tanh() soft saturation | 1.04x drive | 1.07x drive |
| Band-limiting | Butterworth lowpass with zero-phase filtering | 20 kHz | 19 kHz |
| Room tone | Gaussian noise simulating ambient room noise | sigma=1.5e-7 | sigma=3e-7 |

### Quality Preservation

All sanitization modes apply:

- **RMS normalization**: Restores original RMS level after processing
- **Soft clipping**: Clamps samples to +/-0.99 to prevent digital overs
- **Psychoacoustic masking**: Spectral modifications only apply below the masking threshold to avoid audible artifacts
