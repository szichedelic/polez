use std::path::Path;

/// Generate a small test WAV file (1 second, 44100 Hz, mono, 16-bit sine wave)
pub fn generate_test_wav(path: &Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("Failed to create WAV writer");
    let amplitude = i16::MAX as f32 * 0.5;
    let freq = 440.0;
    for i in 0..44100 {
        let t = i as f32 / 44100.0;
        let sample = (t * freq * 2.0 * std::f32::consts::PI).sin() * amplitude;
        writer.write_sample(sample as i16).unwrap();
    }
    writer.finalize().unwrap();
}
