use std::path::Path;

use lofty::prelude::*;
use serde::Serialize;

use crate::error::Result;

/// Metadata scan results
#[derive(Debug, Clone, Default, Serialize)]
pub struct MetadataScanResult {
    pub tags: Vec<TagInfo>,
    pub suspicious_chunks: Vec<ChunkInfo>,
    pub anomalies: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagInfo {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChunkInfo {
    pub description: String,
    pub offset: u64,
}

pub struct MetadataScanner;

impl MetadataScanner {
    pub fn scan(path: &Path) -> Result<MetadataScanResult> {
        let mut result = MetadataScanResult::default();

        // Use lofty to read tags
        if let Ok(tagged_file) = lofty::read_from_path(path) {
            for tag in tagged_file.tags() {
                for item in tag.items() {
                    result.tags.push(TagInfo {
                        key: format!("{:?}", item.key()),
                        value: format!("{:?}", item.value()),
                    });
                }
            }
        }

        // Binary scan for suspicious patterns
        if let Ok(data) = std::fs::read(path) {
            scan_binary_patterns(&data, &mut result);
        }

        Ok(result)
    }
}

fn scan_binary_patterns(data: &[u8], result: &mut MetadataScanResult) {
    let patterns: &[(&[u8], &str)] = &[
        (b"SUNO", "AI watermark marker"),
        (b"UDIO", "Udio AI marker"),
        (b"AudioCraft", "Meta AudioCraft marker"),
        (b"MusicGen", "MusicGen marker"),
        (b"Stable Audio", "Stable Audio marker"),
        (b"APETAGEX", "APE tag"),
        (b"ID3", "ID3 tag header"),
    ];

    for (pattern, description) in patterns {
        for (offset, window) in data.windows(pattern.len()).enumerate() {
            if window == *pattern {
                result.suspicious_chunks.push(ChunkInfo {
                    description: description.to_string(),
                    offset: offset as u64,
                });
            }
        }
    }
}
