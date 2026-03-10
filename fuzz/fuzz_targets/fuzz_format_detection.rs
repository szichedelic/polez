#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fuzz_detect");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(data).unwrap();
    }
    let _ = polez::audio::io::detect_format(&path);
});
