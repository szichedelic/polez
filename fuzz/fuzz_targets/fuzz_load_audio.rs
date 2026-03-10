#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();

    // Try with .wav extension
    let wav_path = dir.path().join("fuzz.wav");
    {
        let mut f = std::fs::File::create(&wav_path).unwrap();
        f.write_all(data).unwrap();
    }
    let _ = polez::audio::io::load_audio(&wav_path);

    // Try with .mp3 extension
    let mp3_path = dir.path().join("fuzz.mp3");
    {
        let mut f = std::fs::File::create(&mp3_path).unwrap();
        f.write_all(data).unwrap();
    }
    let _ = polez::audio::io::load_audio(&mp3_path);

    // Try with .flac extension
    let flac_path = dir.path().join("fuzz.flac");
    {
        let mut f = std::fs::File::create(&flac_path).unwrap();
        f.write_all(data).unwrap();
    }
    let _ = polez::audio::io::load_audio(&flac_path);

    // Try with no extension (pure magic-byte detection)
    let no_ext_path = dir.path().join("fuzz");
    {
        let mut f = std::fs::File::create(&no_ext_path).unwrap();
        f.write_all(data).unwrap();
    }
    let _ = polez::audio::io::load_audio(&no_ext_path);
});
