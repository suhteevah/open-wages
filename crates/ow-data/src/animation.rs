//! Parser for `.COR` animation sequence definition files.
//!
//! Each `.COR` file is a plaintext, line-oriented index into a companion `.DAT`
//! sprite sheet and `.ADD` overlay file. It describes every animation sequence
//! an entity possesses: action, weapon class, facing direction, frame count,
//! mirror flag, and associated sound effect.
//!
//! ## File layout
//!
//! ```text
//! <dat_filename>            # companion binary sprite data
//! <add_filename>            # companion additional data
//! <header_value>            # integer (1 or 4 observed)
//! [NrAnimations-...]        # field legend (skipped)
//! <total_animation_count>
//! [1. human-readable label  # comment (skipped)
//! f1,f2,f3,f4,f5,f6,f7,f8,f9
//! ...
//! [END]
//! ```

use std::path::Path;

use tracing::{debug, info, trace, warn};

/// A single animation sequence entry (nine comma-separated integers).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimationEntry {
    /// `1` = normal sprite, `2` = horizontally mirrored from opposite direction.
    pub mirror_flag: u8,
    /// Base index / offset into the sprite sheet for this animation's frames.
    pub frame_offset: u32,
    /// Action type identifier (entity-specific meaning).
    pub action_id: u32,
    /// Weapon class for soldiers, context-specific for other entity types.
    pub weapon_id: i32,
    /// Facing direction (0–7 for 8-direction entities).
    pub direction: u8,
    /// Number of animation frames. `0` = single static frame or placeholder.
    pub frame_count: u32,
    /// Sound effect ID to play during this animation. `0` = none.
    pub sound_id: i32,
    /// Reserved / unknown field (always `0` in observed data).
    pub field8: u32,
    /// Playback modifier — loop count, speed, or sub-animation grouping.
    pub field9: u32,
}

/// A fully parsed `.COR` animation set.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimationSet {
    /// Companion `.DAT` sprite-sheet filename (e.g. `"GUARDDOG.dat"`).
    pub dat_filename: String,
    /// Companion `.ADD` overlay filename (e.g. `"GUARDDOG.add"`).
    pub add_filename: String,
    /// Header integer (observed as `1` or `4`).
    pub header_value: u32,
    /// Declared total number of animation entries.
    pub total_animations: usize,
    /// Parsed animation entries.
    pub entries: Vec<AnimationEntry>,
}

/// Errors that can occur while parsing a `.COR` file.
#[derive(Debug, thiserror::Error)]
pub enum AnimationError {
    #[error("I/O error reading animation file: {0}")]
    Io(#[from] std::io::Error),
    #[error("line {line}: expected header value integer, got '{text}'")]
    InvalidHeaderValue { line: usize, text: String },
    #[error("line {line}: expected total animation count, got '{text}'")]
    InvalidTotalCount { line: usize, text: String },
    #[error("line {line}: expected 9 comma-separated fields, got {count} in '{text}'")]
    WrongFieldCount {
        line: usize,
        count: usize,
        text: String,
    },
    #[error("line {line}: failed to parse field {field} as integer in '{text}'")]
    InvalidField {
        line: usize,
        field: usize,
        text: String,
    },
    #[error("entry count mismatch: header declares {expected}, parsed {actual}")]
    CountMismatch { expected: usize, actual: usize },
    #[error("unexpected end of file (expected more animation entries)")]
    UnexpectedEof,
    #[error("file has fewer than 5 header lines")]
    TruncatedHeader,
}

/// Parse a `.COR` animation sequence file into an [`AnimationSet`].
///
/// The file must contain a 5-line header followed by paired comment + data
/// lines for each animation, terminated by `[END]`.
pub fn parse_animation(path: &Path) -> Result<AnimationSet, AnimationError> {
    info!(path = %path.display(), "Parsing .COR animation file");

    let raw = std::fs::read_to_string(path)?;

    // Normalise line endings and collect non-empty lines.
    let lines: Vec<&str> = raw
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .collect();

    if lines.len() < 5 {
        return Err(AnimationError::TruncatedHeader);
    }

    // --- Header (lines 0–4) ---
    let dat_filename = lines[0].trim().to_string();
    let add_filename = lines[1].trim().to_string();

    let header_value: u32 = lines[2]
        .trim()
        .parse()
        .map_err(|_| AnimationError::InvalidHeaderValue {
            line: 3,
            text: lines[2].to_string(),
        })?;

    // Line 4 is the field legend — skip it.
    trace!(legend = lines[3], "Skipping field legend line");

    let total_animations: usize = lines[4]
        .trim()
        .parse()
        .map_err(|_| AnimationError::InvalidTotalCount {
            line: 5,
            text: lines[4].to_string(),
        })?;

    debug!(
        dat = %dat_filename,
        add = %add_filename,
        header_value,
        total_animations,
        "Parsed .COR header"
    );

    // --- Animation entries ---
    let mut entries = Vec::with_capacity(total_animations);
    let mut idx = 5; // first line after header

    while idx < lines.len() {
        let line = lines[idx].trim();

        // Terminator
        if line == "[END]" {
            trace!("Reached [END] terminator at file line {}", idx + 1);
            break;
        }

        // Skip blank lines
        if line.is_empty() {
            idx += 1;
            continue;
        }

        // Comment / label line — starts with '['
        if line.starts_with('[') {
            trace!(label = line, "Skipping comment line");
            idx += 1;
            continue;
        }

        // Data line — nine comma-separated integers
        let entry = parse_data_line(line, idx + 1)?;
        trace!(entry = ?entry, "Parsed animation entry #{}", entries.len() + 1);
        entries.push(entry);
        idx += 1;
    }

    if entries.len() != total_animations {
        warn!(
            expected = total_animations,
            actual = entries.len(),
            "Animation entry count mismatch"
        );
        return Err(AnimationError::CountMismatch {
            expected: total_animations,
            actual: entries.len(),
        });
    }

    info!(
        count = entries.len(),
        "Successfully parsed .COR animation file"
    );

    Ok(AnimationSet {
        dat_filename,
        add_filename,
        header_value,
        total_animations,
        entries,
    })
}

/// Parse a single comma-separated data line into an [`AnimationEntry`].
fn parse_data_line(line: &str, file_line: usize) -> Result<AnimationEntry, AnimationError> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() != 9 {
        return Err(AnimationError::WrongFieldCount {
            line: file_line,
            count: parts.len(),
            text: line.to_string(),
        });
    }

    let parse_u8 = |idx: usize| -> Result<u8, AnimationError> {
        parts[idx]
            .trim()
            .parse()
            .map_err(|_| AnimationError::InvalidField {
                line: file_line,
                field: idx + 1,
                text: line.to_string(),
            })
    };

    let parse_u32 = |idx: usize| -> Result<u32, AnimationError> {
        parts[idx]
            .trim()
            .parse()
            .map_err(|_| AnimationError::InvalidField {
                line: file_line,
                field: idx + 1,
                text: line.to_string(),
            })
    };

    let parse_i32 = |idx: usize| -> Result<i32, AnimationError> {
        parts[idx]
            .trim()
            .parse()
            .map_err(|_| AnimationError::InvalidField {
                line: file_line,
                field: idx + 1,
                text: line.to_string(),
            })
    };

    Ok(AnimationEntry {
        mirror_flag: parse_u8(0)?,
        frame_offset: parse_u32(1)?,
        action_id: parse_u32(2)?,
        weapon_id: parse_i32(3)?,
        direction: parse_u8(4)?,
        frame_count: parse_u32(5)?,
        sound_id: parse_i32(6)?,
        field8: parse_u32(7)?,
        field9: parse_u32(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Build a minimal .COR file in a temp directory and parse it.
    fn write_temp_cor(content: &str) -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join("ow_data_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("test_anim_{id}.cor"));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn parse_guarddog_style() {
        let content = "\
GUARDDOG.dat\r\n\
GUARDDOG.add\r\n\
1\r\n\
[NrAnimations-action-weapon-direction-nrframes]\r\n\
3\r\n\
[1. dog walk S\r\n\
1,0,0,8,0,16,0,0,1\r\n\
[2. dog walk SW\r\n\
1,0,0,8,1,16,0,0,1\r\n\
[3. dog walk W\r\n\
2,0,0,8,2,16,0,0,1\r\n\
[END]\r\n";

        let path = write_temp_cor(content);
        let result = parse_animation(&path).expect("should parse successfully");

        assert_eq!(result.dat_filename, "GUARDDOG.dat");
        assert_eq!(result.add_filename, "GUARDDOG.add");
        assert_eq!(result.header_value, 1);
        assert_eq!(result.total_animations, 3);
        assert_eq!(result.entries.len(), 3);

        // First entry
        assert_eq!(result.entries[0].mirror_flag, 1);
        assert_eq!(result.entries[0].frame_offset, 0);
        assert_eq!(result.entries[0].action_id, 0);
        assert_eq!(result.entries[0].weapon_id, 8);
        assert_eq!(result.entries[0].direction, 0);
        assert_eq!(result.entries[0].frame_count, 16);
        assert_eq!(result.entries[0].sound_id, 0);
        assert_eq!(result.entries[0].field8, 0);
        assert_eq!(result.entries[0].field9, 1);

        // Third entry — mirrored
        assert_eq!(result.entries[2].mirror_flag, 2);
    }

    #[test]
    fn parse_boat_style_with_header_4() {
        let content = "\
BOAT01.dat\r\n\
BOAT01.add\r\n\
4\r\n\
[NrAnimations-action-weapon-direction-nrframes]\r\n\
2\r\n\
[1. boat idle NW]\r\n\
1,0,0,6,1,0,0,0,1\r\n\
[2. boat dest NE]\r\n\
1,0,99,0,0,0,0,0,1\r\n\
[END]\r\n";

        let path = write_temp_cor(content);
        let result = parse_animation(&path).expect("should parse successfully");

        assert_eq!(result.header_value, 4);
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[1].action_id, 99);
    }

    #[test]
    fn count_mismatch_is_error() {
        let content = "\
TEST.dat\n\
TEST.add\n\
1\n\
[NrAnimations-action-weapon-direction-nrframes]\n\
2\n\
[1. only one entry\n\
1,0,0,0,0,0,0,0,1\n\
[END]\n";

        let path = write_temp_cor(content);
        let err = parse_animation(&path).unwrap_err();
        assert!(matches!(err, AnimationError::CountMismatch { expected: 2, actual: 1 }));
    }

    #[test]
    fn wrong_field_count_is_error() {
        let content = "\
TEST.dat\n\
TEST.add\n\
1\n\
[NrAnimations-action-weapon-direction-nrframes]\n\
1\n\
[1. bad entry\n\
1,0,0,0,0\n\
[END]\n";

        let path = write_temp_cor(content);
        let err = parse_animation(&path).unwrap_err();
        assert!(matches!(err, AnimationError::WrongFieldCount { .. }));
    }

    #[test]
    fn truncated_header_is_error() {
        let content = "ONLY.dat\nONLY.add\n";
        let path = write_temp_cor(content);
        let err = parse_animation(&path).unwrap_err();
        assert!(matches!(err, AnimationError::TruncatedHeader));
    }
}
