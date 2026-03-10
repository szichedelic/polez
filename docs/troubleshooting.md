# Troubleshooting Guide & FAQ

## Common Issues

### 1. File won't load: "unsupported format"

**Symptoms:** Error message about unsupported or unrecognized audio format.

**Cause:** Polez supports WAV, MP3, FLAC, OGG, and AAC. Other formats (WMA, AIFF, ALAC, etc.) are not supported. The file may also be corrupted or truncated.

**Solution:**
- Convert the file to WAV or FLAC using ffmpeg: `ffmpeg -i input.wma output.wav`
- Verify the file plays correctly in another audio player
- Check that the file extension matches the actual format (rename won't fix a format mismatch)

### 2. File won't load: "codec not found"

**Symptoms:** File has a supported extension but fails to decode.

**Cause:** The file uses a codec variant not supported by the symphonia decoder (e.g., certain MP3 encoding modes, non-standard WAV chunks).

**Solution:**
- Re-encode with a standard codec: `ffmpeg -i input.mp3 -c:a libmp3lame output.mp3`
- For WAV files, ensure PCM encoding: `ffmpeg -i input.wav -c:a pcm_s16le output.wav`

### 3. Cleaning produces audible artifacts

**Symptoms:** Output audio has clicks, pops, tonal changes, or other audible distortions.

**Cause:** Sanitization mode is too aggressive for the content, or advanced DSP flags are causing cumulative degradation.

**Solution:**
- Use a gentler mode: `polez clean --mode preserving input.wav` instead of `aggressive`
- Reduce active advanced flags in config (disable `comb_mask`, `transient_shift`, `hf_decorrelate`)
- Increase quality preservation: set `preserve_quality: Maximum` in config
- Check the verification grade — grades below B indicate significant quality impact
- Use the `quality` preset: `polez config preset quality`

### 4. High quality_loss percentage

**Symptoms:** Verification reports quality loss above 5% or a low grade (C/D).

**Cause:** The combination of active operations is modifying the audio more than intended. This is common with `Maximum` paranoia level and many advanced flags enabled.

**Solution:**
- Lower paranoia level: `Medium` or `High` instead of `Maximum`
- Disable unnecessary fingerprint removal techniques (phase randomization and human imperfections cause the most quality impact)
- Use the `preserving` mode which applies all stealth operations but with gentler parameters
- Check `target_snr` in config — increasing it constrains the pipeline to maintain higher quality

### 5. GUI won't start: "address already in use"

**Symptoms:** `polez gui` fails with a port binding error.

**Cause:** Port 3000 is already in use by another process.

**Solution:**
- Find the process: `lsof -i :3000` (macOS/Linux) or `netstat -ano | findstr 3000` (Windows)
- Kill the process or wait for it to finish
- The GUI currently uses a hardcoded port (3000); close other services using that port

### 6. GUI loads but shows blank page

**Symptoms:** Browser opens to localhost:3000 but shows nothing or a white screen.

**Cause:** The frontend was not built before compiling the release binary, so no embedded assets exist.

**Solution:**
- Build the frontend first: `cd gui && npm run build`
- Then rebuild the binary: `cargo build --release`
- Or use the build script: `gui/build.sh`
- For development, run the Vite dev server separately: `cd gui && npm run dev` (port 5173) alongside `cargo run -- gui` (port 3000)

### 7. Detection false positives

**Symptoms:** Detection reports high confidence for watermarks or AI probability on files known to be clean.

**Cause:** Certain audio characteristics trigger false positives:
- Synthesizers and electronic music trigger spread spectrum and phase modulation detectors
- Heavily compressed audio (podcast, radio) triggers statistical anomaly detection
- Re-encoded files trigger codec artifact detection
- Music with strong high-frequency content triggers ultrasonic energy detection

**Solution:**
- Look at the overall confidence rather than individual method results — a single method flagging is often a false positive
- Use `--deep` mode for more thorough analysis: `polez detect --deep input.wav`
- Check multiple detection methods — real watermarks typically trigger several methods simultaneously
- Confidence below 30% is generally noise; 30-60% is inconclusive; above 60% is likely a true positive
- The Polez AI watermark detector requires 48 kHz+ sample rate — lower rates skip ultrasonic analysis

### 8. Large file performance / out of memory

**Symptoms:** Processing is very slow or the process runs out of memory on large files.

**Cause:** Files over ~120 seconds at 44.1 kHz (>5.3M samples) are processed in chunks. Very large files or high sample rates (96 kHz, 192 kHz) increase memory usage proportionally.

**Solution:**
- The chunked processing activates automatically for large files (~30 s chunks with 1 s overlap)
- Use `fast` mode for large batch jobs: `polez clean --mode fast input.wav`
- For batch processing, configure workers in config: `batch_processing.workers: 2` (reduce from default 4)
- Close other memory-intensive applications
- Consider downsampling to 44.1/48 kHz before processing if the source is 96+ kHz

### 9. Batch processing stops on first error

**Symptoms:** `polez sweep` stops after encountering one bad file.

**Cause:** By default, `continue_on_error` is false in the batch processing config.

**Solution:**
- Add `--continue-on-error` flag or set in config:
  ```yaml
  batch_processing:
    continue_on_error: true
  ```
- Failed files are logged but processing continues

### 10. Config changes have no effect

**Symptoms:** Editing config.yaml doesn't change behavior.

**Cause:** Command-line arguments override config file values. Environment variables also take precedence over the config file.

**Solution:**
- Check for `POLEZ_MODE`, `POLEZ_QUALITY`, `POLEZ_OUTPUT_FORMAT`, or `POLEZ_PARANOID` environment variables
- Check command-line flags — they always take precedence
- Verify config file location: `polez config show` displays the active configuration
- Validate the config file: syntax errors cause a silent fallback to defaults
- Run `polez config reset` and re-apply your changes to eliminate corrupted config

### 11. Output file is larger than input

**Symptoms:** The cleaned file is significantly larger than the original.

**Cause:** Format conversion occurred — typically MP3 input was saved as WAV, or a low-bitrate file was re-encoded at a higher bitrate.

**Solution:**
- Set `output_format: Preserve` in config to keep the original format
- When outputting MP3, set an appropriate bitrate: `formats.mp3.bitrate: "preserve"`
- WAV files are uncompressed and will always be larger than compressed formats

### 12. Watermarks still detected after cleaning

**Symptoms:** Running detection on the cleaned file still shows watermark confidence above zero.

**Cause:** Some detection methods measure statistical properties that may still show traces after cleaning. The `fast` mode only strips metadata and does not modify audio data.

**Solution:**
- Use `standard` or `preserving` mode instead of `fast`
- Check the verification panel — `removal_effectiveness` shows what percentage of threats were addressed
- Enable more advanced flags for stubborn watermarks: `masked_hf_phase`, `adaptive_notch`, `hf_decorrelate`
- Run with `aggressive` mode as a last resort (higher quality impact)
- Compare before/after confidence — a drop from 80% to 15% is a successful clean even if not zero

## FAQ

**Q: Does cleaning guarantee watermarks are fully removed?**
No. Cleaning reduces detection confidence and disrupts known watermark patterns, but sophisticated watermarks may leave residual traces. The verification grade indicates how effective the cleaning was.

**Q: Which mode should I use?**
Start with `standard`. If detection still shows high confidence, try `preserving`. Use `aggressive` only if quality is not a concern. Use `fast` when you only need metadata removal.

**Q: Can I undo cleaning?**
No. Set `backup_originals: true` in config before cleaning if you want to keep originals. Once processed, the original data cannot be recovered from the cleaned file.

**Q: Why does the AI probability score seem high on human-made music?**
The AI probability scorer uses statistical heuristics that can flag heavily processed, synthesized, or electronically produced music. It works best on acoustic or minimally processed recordings. Treat scores below 60% as inconclusive.

**Q: Does Polez modify audio quality?**
All modes except `fast` modify the audio signal. The `preserving` mode is designed to minimize perceptible changes. Check the SNR and spectral similarity metrics in the verification output to assess impact.

**Q: How do I process an entire directory?**
Use the `sweep` command: `polez sweep /path/to/directory --mode standard`. Configure workers and naming patterns in the batch processing config section.
