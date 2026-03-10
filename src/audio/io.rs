use std::path::Path;

use crate::audio::AudioBuffer;
use crate::error::{PolezError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Wav,
    Mp3,
    Flac,
    Aac,
}

impl AudioFormat {
    pub fn extension(&self) -> &str {
        match self {
            AudioFormat::Wav => "wav",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Flac => "flac",
            AudioFormat::Aac => "m4a",
        }
    }

    /// Whether this format has an encoder available for output.
    pub fn has_encoder(&self) -> bool {
        matches!(self, AudioFormat::Wav | AudioFormat::Mp3)
    }
}

impl std::fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioFormat::Wav => write!(f, "WAV"),
            AudioFormat::Mp3 => write!(f, "MP3"),
            AudioFormat::Flac => write!(f, "FLAC"),
            AudioFormat::Aac => write!(f, "AAC"),
        }
    }
}

/// Detect audio format from file extension.
pub fn detect_format(path: &Path) -> Result<AudioFormat> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("wav") => Ok(AudioFormat::Wav),
        Some("mp3") => Ok(AudioFormat::Mp3),
        Some("flac") => Ok(AudioFormat::Flac),
        Some("aac") | Some("m4a") => Ok(AudioFormat::Aac),
        Some(ext) => Err(PolezError::UnsupportedFormat(ext.to_string())),
        None => Err(PolezError::UnsupportedFormat("no extension".to_string())),
    }
}

/// Load an audio file into an AudioBuffer.
pub fn load_audio(path: &Path) -> Result<(AudioBuffer, AudioFormat)> {
    let format = detect_format(path)?;
    match format {
        AudioFormat::Wav => load_wav(path).map(|buf| (buf, format)),
        AudioFormat::Mp3 | AudioFormat::Flac | AudioFormat::Aac => {
            load_symphonia(path, format).map(|buf| (buf, format))
        }
    }
}

/// Save an AudioBuffer to a file in the specified format.
/// Note: FLAC and AAC encoding are not supported; use WAV or MP3 instead.
pub fn save_audio(buffer: &AudioBuffer, path: &Path, format: AudioFormat) -> Result<()> {
    match format {
        AudioFormat::Wav => save_wav(buffer, path),
        AudioFormat::Mp3 => save_mp3(buffer, path),
        AudioFormat::Flac | AudioFormat::Aac => Err(PolezError::UnsupportedFormat(format!(
            "{format} encoding is not supported; use WAV or MP3 output format"
        ))),
    }
}

// --- WAV ---

fn load_wav(path: &Path) -> Result<AudioBuffer> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| PolezError::AudioIo(format!("Failed to open WAV: {e}")))?;

    let spec = reader.spec();
    let channels = spec.channels as usize;
    let sample_rate = spec.sample_rate;

    let samples_f32: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| PolezError::AudioIo(format!("WAV decode error: {e}")))?
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| PolezError::AudioIo(format!("WAV decode error: {e}")))?,
    };

    Ok(AudioBuffer::from_interleaved(
        &samples_f32,
        channels,
        sample_rate,
    ))
}

fn save_wav(buffer: &AudioBuffer, path: &Path) -> Result<()> {
    let spec = hound::WavSpec {
        channels: buffer.num_channels() as u16,
        sample_rate: buffer.sample_rate,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| PolezError::AudioIo(format!("Failed to create WAV: {e}")))?;

    let max_val = (1u32 << 23) as f32; // 24-bit
    let interleaved = buffer.to_interleaved();
    for sample in interleaved {
        let clamped = sample.clamp(-1.0, 1.0);
        let int_sample = (clamped * max_val) as i32;
        writer
            .write_sample(int_sample)
            .map_err(|e| PolezError::AudioIo(format!("WAV write error: {e}")))?;
    }

    writer
        .finalize()
        .map_err(|e| PolezError::AudioIo(format!("WAV finalize error: {e}")))?;

    Ok(())
}

// --- Symphonia-based loader (MP3, FLAC, AAC) ---

fn load_symphonia(path: &Path, format: AudioFormat) -> Result<AudioBuffer> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let format_name = format.to_string();

    let file = std::fs::File::open(path)
        .map_err(|e| PolezError::AudioIo(format!("Failed to open {format_name}: {e}")))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| PolezError::AudioIo(format!("{format_name} probe error: {e}")))?;

    let mut format_reader = probed.format;
    let track = format_reader
        .default_track()
        .ok_or_else(|| PolezError::AudioIo("No audio track found".to_string()))?;

    let track_id = track.id;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| PolezError::AudioIo(format!("{format_name} decoder error: {e}")))?;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format_reader.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let spec = *decoded.spec();
        let num_frames = decoded.capacity();
        let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        all_samples.extend_from_slice(sample_buf.samples());
    }

    if all_samples.is_empty() {
        return Err(PolezError::AudioIo("No audio samples decoded".to_string()));
    }

    Ok(AudioBuffer::from_interleaved(
        &all_samples,
        channels,
        sample_rate,
    ))
}

fn save_mp3(buffer: &AudioBuffer, path: &Path) -> Result<()> {
    use mp3lame_encoder::{Builder, FlushNoGap, InterleavedPcm};

    let channels = buffer.num_channels();
    let mut builder = Builder::new()
        .ok_or_else(|| PolezError::AudioIo("Failed to create MP3 encoder".to_string()))?;

    builder
        .set_sample_rate(buffer.sample_rate)
        .map_err(|e| PolezError::AudioIo(format!("MP3 encoder config error: {e:?}")))?;

    builder
        .set_num_channels(channels as u8)
        .map_err(|e| PolezError::AudioIo(format!("MP3 encoder config error: {e:?}")))?;

    builder
        .set_brate(mp3lame_encoder::Bitrate::Kbps320)
        .map_err(|e| PolezError::AudioIo(format!("MP3 encoder config error: {e:?}")))?;

    builder
        .set_quality(mp3lame_encoder::Quality::Best)
        .map_err(|e| PolezError::AudioIo(format!("MP3 encoder config error: {e:?}")))?;

    let mut encoder = builder
        .build()
        .map_err(|e| PolezError::AudioIo(format!("MP3 encoder build error: {e:?}")))?;

    // Convert f32 [-1,1] to i16 for LAME
    let interleaved = buffer.to_interleaved();
    let pcm_i16: Vec<i16> = interleaved
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect();

    let input = InterleavedPcm(&pcm_i16);

    let mut mp3_out = Vec::with_capacity(pcm_i16.len());
    encoder
        .encode_to_vec(input, &mut mp3_out)
        .map_err(|e| PolezError::AudioIo(format!("MP3 encode error: {e:?}")))?;

    encoder
        .flush_to_vec::<FlushNoGap>(&mut mp3_out)
        .map_err(|e| PolezError::AudioIo(format!("MP3 flush error: {e:?}")))?;

    std::fs::write(path, &mp3_out)
        .map_err(|e| PolezError::AudioIo(format!("Failed to write MP3: {e}")))?;

    Ok(())
}
