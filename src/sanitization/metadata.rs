//! Metadata tag stripping using lofty.
//!
//! Removes all embedded tags (ID3, Vorbis comments, APE, etc.) from audio files.

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::prelude::*;

use crate::error::{PolezError, Result};

/// Strips all metadata tags from audio files.
pub struct MetadataCleaner;

impl MetadataCleaner {
    /// Strip all metadata tags from an audio file.
    /// This modifies the file in-place.
    pub fn strip_all(path: &Path) -> Result<usize> {
        let mut tagged_file = lofty::read_from_path(path)
            .map_err(|e| PolezError::Metadata(format!("Failed to read tags: {e}")))?;

        let mut removed = 0;
        for tag in tagged_file.tags() {
            removed += tag.len();
        }

        // Remove all tag types
        tagged_file.clear();

        tagged_file
            .save_to_path(path, WriteOptions::default())
            .map_err(|e| PolezError::Metadata(format!("Failed to save stripped file: {e}")))?;

        Ok(removed)
    }

    /// Strip metadata and write to a new file.
    /// Copies audio data, strips tags from the copy.
    pub fn strip_to(input: &Path, output: &Path) -> Result<usize> {
        std::fs::copy(input, output)
            .map_err(|e| PolezError::Metadata(format!("Failed to copy file: {e}")))?;
        Self::strip_all(output)
    }
}
