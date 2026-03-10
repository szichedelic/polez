use std::path::Path;

use crate::audio::AudioBuffer;
use crate::error::{PolezError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Wav,
    Mp3,
    Flac,
    Ogg,
    Aac,
}

impl AudioFormat {
    pub fn extension(&self) -> &str {
        match self {
            AudioFormat::Wav => "wav",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Flac => "flac",
            AudioFormat::Ogg => "ogg",
            AudioFormat::Aac => "m4a",
        }
    }

    /// Whether this format has an encoder available for output.
    pub fn has_encoder(&self) -> bool {
        matches!(
            self,
            AudioFormat::Wav | AudioFormat::Mp3 | AudioFormat::Flac | AudioFormat::Ogg
        )
    }
}

impl std::fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioFormat::Wav => write!(f, "WAV"),
            AudioFormat::Mp3 => write!(f, "MP3"),
            AudioFormat::Flac => write!(f, "FLAC"),
            AudioFormat::Ogg => write!(f, "OGG"),
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
        Some("ogg") | Some("oga") => Ok(AudioFormat::Ogg),
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
        AudioFormat::Mp3 | AudioFormat::Flac | AudioFormat::Ogg | AudioFormat::Aac => {
            load_symphonia(path, format).map(|buf| (buf, format))
        }
    }
}

/// Save an AudioBuffer to a file in the specified format.
/// `bit_depth` only applies to WAV output (16, 24, or 32). Ignored for other formats.
pub fn save_audio(
    buffer: &AudioBuffer,
    path: &Path,
    format: AudioFormat,
    bit_depth: Option<u16>,
) -> Result<()> {
    match format {
        AudioFormat::Wav => save_wav(buffer, path, bit_depth.unwrap_or(24)),
        AudioFormat::Mp3 => save_mp3(buffer, path),
        AudioFormat::Flac => save_flac(buffer, path),
        AudioFormat::Ogg => save_ogg(buffer, path),
        AudioFormat::Aac => Err(PolezError::UnsupportedFormat(
            "AAC encoding is not supported; use wav, mp3, flac, or ogg".into(),
        )),
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

fn save_wav(buffer: &AudioBuffer, path: &Path, bit_depth: u16) -> Result<()> {
    let interleaved = buffer.to_interleaved();

    if bit_depth == 32 {
        // 32-bit float output
        let spec = hound::WavSpec {
            channels: buffer.num_channels() as u16,
            sample_rate: buffer.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec)
            .map_err(|e| PolezError::AudioIo(format!("Failed to create WAV: {e}")))?;
        for sample in interleaved {
            writer
                .write_sample(sample.clamp(-1.0, 1.0))
                .map_err(|e| PolezError::AudioIo(format!("WAV write error: {e}")))?;
        }
        writer
            .finalize()
            .map_err(|e| PolezError::AudioIo(format!("WAV finalize error: {e}")))?;
    } else {
        // Integer output (16 or 24 bit)
        let spec = hound::WavSpec {
            channels: buffer.num_channels() as u16,
            sample_rate: buffer.sample_rate,
            bits_per_sample: bit_depth,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec)
            .map_err(|e| PolezError::AudioIo(format!("Failed to create WAV: {e}")))?;

        let max_val = (1u32 << (bit_depth - 1)) as f32;

        // TPDF dithering for 16-bit to reduce quantization artifacts
        let use_dither = bit_depth == 16;
        let mut rng = rand::thread_rng();

        for sample in interleaved {
            let clamped = sample.clamp(-1.0, 1.0);
            let scaled = if use_dither {
                use rand::Rng;
                // TPDF dither: sum of two uniform random values in [-0.5, 0.5]
                let d1: f32 = rng.gen_range(-0.5..0.5);
                let d2: f32 = rng.gen_range(-0.5..0.5);
                (clamped * max_val + d1 + d2).round() as i32
            } else {
                (clamped * max_val) as i32
            };
            writer
                .write_sample(scaled)
                .map_err(|e| PolezError::AudioIo(format!("WAV write error: {e}")))?;
        }
        writer
            .finalize()
            .map_err(|e| PolezError::AudioIo(format!("WAV finalize error: {e}")))?;
    }

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

// --- FLAC ---

fn save_flac(buffer: &AudioBuffer, path: &Path) -> Result<()> {
    use flacenc::bitsink::ByteSink;
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;

    let channels = buffer.num_channels();
    let bits_per_sample = 16_u32;
    let sample_rate = buffer.sample_rate;

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| PolezError::AudioIo(format!("FLAC encoder config error: {e:?}")))?;

    // Convert f32 [-1,1] to i32 (16-bit range) interleaved
    let interleaved = buffer.to_interleaved();
    let max_val = (1u32 << (bits_per_sample - 1)) as f32;
    let samples_i32: Vec<i32> = interleaved
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * (max_val - 1.0)) as i32)
        .collect();

    let source = flacenc::source::MemSource::from_samples(
        &samples_i32,
        channels,
        bits_per_sample as usize,
        sample_rate as usize,
    );

    let flac_stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| PolezError::AudioIo(format!("FLAC encode error: {e}")))?;

    let mut sink = ByteSink::new();
    flac_stream
        .write(&mut sink)
        .map_err(|e| PolezError::AudioIo(format!("FLAC write error: {e:?}")))?;

    std::fs::write(path, sink.as_slice())
        .map_err(|e| PolezError::AudioIo(format!("Failed to write FLAC: {e}")))?;

    Ok(())
}

// --- OGG Vorbis ---

fn save_ogg(buffer: &AudioBuffer, path: &Path) -> Result<()> {
    use std::io::BufWriter;
    use std::num::{NonZeroU32, NonZeroU8};
    use vorbis_rs::VorbisEncoderBuilder;

    let channels = buffer.num_channels();
    let sample_rate = buffer.sample_rate;

    let file = std::fs::File::create(path)
        .map_err(|e| PolezError::AudioIo(format!("Failed to create OGG file: {e}")))?;
    let writer = BufWriter::new(file);

    let mut encoder = VorbisEncoderBuilder::new(
        NonZeroU32::new(sample_rate).unwrap_or(NonZeroU32::new(44100).unwrap()),
        NonZeroU8::new(channels as u8).unwrap_or(NonZeroU8::new(1).unwrap()),
        writer,
    )
    .map_err(|e| PolezError::AudioIo(format!("OGG encoder init error: {e}")))?
    .build()
    .map_err(|e| PolezError::AudioIo(format!("OGG encoder build error: {e}")))?;

    // VorbisEncoder expects audio blocks as &[Vec<f32>] where outer = channels
    let block_size = 4096;
    let total_samples = buffer.channel(0).len();

    for start in (0..total_samples).step_by(block_size) {
        let end = (start + block_size).min(total_samples);
        let block: Vec<Vec<f32>> = (0..channels)
            .map(|ch| {
                buffer
                    .channel(ch)
                    .iter()
                    .skip(start)
                    .take(end - start)
                    .copied()
                    .collect()
            })
            .collect();
        encoder
            .encode_audio_block(block)
            .map_err(|e| PolezError::AudioIo(format!("OGG encode error: {e}")))?;
    }

    encoder
        .finish()
        .map_err(|e| PolezError::AudioIo(format!("OGG finalize error: {e}")))?;

    Ok(())
}
