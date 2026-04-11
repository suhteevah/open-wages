//! Parser for `TEXTRECT*.DAT` — UI text rectangle layout definitions.
//!
//! These files map ENGWOW.DAT string indices to screen-space bounding
//! rectangles for rendering UI labels and stat values. Each entry specifies
//! pixel coordinates and the string table index to display.
//!
//! ## Format
//!
//! ```text
//! 40 #lines to read           <-- entry count (with optional comment)
//! 225 356 250 369 57 #age     <-- x1 y1 x2 y2 string_index #comment
//! ...
//! ```
//!
//! Lines starting with `#` are comments and are skipped. Blank lines are
//! skipped. The first line declares how many data entries to expect.

use std::path::Path;

use tracing::{debug, info, trace, warn};

/// A UI text rectangle: bounding box plus string table index.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextRect {
    /// Left edge (pixels).
    pub x1: i32,
    /// Top edge (pixels).
    pub y1: i32,
    /// Right edge (pixels).
    pub x2: i32,
    /// Bottom edge (pixels).
    pub y2: i32,
    /// Index into the ENGWOW.DAT string table (or merc data field index).
    pub string_index: usize,
}

/// Errors that can occur while parsing a `TEXTRECT*.DAT` file.
#[derive(Debug, thiserror::Error)]
pub enum TextRectError {
    #[error("I/O error reading textrect file: {0}")]
    Io(#[from] std::io::Error),
    #[error("file is empty or contains no data lines")]
    Empty,
    #[error("line {line}: expected entry count, got '{text}'")]
    InvalidEntryCount { line: usize, text: String },
    #[error("line {line}: expected at least 5 fields, got {count} in '{text}'")]
    TooFewFields {
        line: usize,
        count: usize,
        text: String,
    },
    #[error("line {line}: failed to parse field '{token}' as integer")]
    InvalidField { line: usize, token: String },
    #[error("entry count mismatch: header declares {expected}, parsed {actual}")]
    CountMismatch { expected: usize, actual: usize },
}

/// Parse a `TEXTRECT*.DAT` file into a list of [`TextRect`] entries.
///
/// The first non-comment, non-blank line is the declared entry count (the
/// integer before any `#` comment on that line). Subsequent data lines each
/// contain `x1 y1 x2 y2 string_index` with an optional trailing `#` comment.
pub fn parse_text_rects(path: &Path) -> Result<Vec<TextRect>, TextRectError> {
    info!(path = %path.display(), "Parsing TEXTRECT*.DAT");

    let raw = std::fs::read_to_string(path)?;

    let lines: Vec<&str> = raw.lines().map(|l| l.trim_end_matches('\r')).collect();

    // Find the first non-blank, non-comment line for the entry count.
    let mut idx = 0;
    let expected_count = loop {
        if idx >= lines.len() {
            return Err(TextRectError::Empty);
        }
        let trimmed = lines[idx].trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            idx += 1;
            continue;
        }
        // The count line may have a trailing comment like "40 #lines to read".
        let count_token = strip_comment(trimmed)
            .split_whitespace()
            .next()
            .ok_or_else(|| TextRectError::InvalidEntryCount {
                line: idx + 1,
                text: trimmed.to_string(),
            })?
            .to_string();

        let count: usize = count_token
            .parse()
            .map_err(|_| TextRectError::InvalidEntryCount {
                line: idx + 1,
                text: trimmed.to_string(),
            })?;

        debug!(expected = count, "Declared entry count");
        idx += 1;
        break count;
    };

    // Parse data lines.
    let mut rects = Vec::with_capacity(expected_count);

    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        idx += 1;

        // Skip blanks and full-line comments.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let data = strip_comment(trimmed);
        let tokens: Vec<&str> = data.split_whitespace().collect();

        if tokens.len() < 5 {
            // Could be a tab-separated variant — try splitting on tabs too.
            let tab_tokens: Vec<&str> = data.split([' ', '\t']).filter(|t| !t.is_empty()).collect();
            if tab_tokens.len() < 5 {
                warn!(
                    line = idx,
                    fields = tab_tokens.len(),
                    text = trimmed,
                    "Skipping line with too few fields"
                );
                continue;
            }
            let rect = parse_rect_tokens(&tab_tokens, idx)?;
            trace!(rect = ?rect, "Parsed text rect");
            rects.push(rect);
            continue;
        }

        let rect = parse_rect_tokens(&tokens, idx)?;
        trace!(rect = ?rect, "Parsed text rect");
        rects.push(rect);
    }

    if rects.len() != expected_count {
        warn!(
            expected = expected_count,
            actual = rects.len(),
            "Entry count mismatch"
        );
        return Err(TextRectError::CountMismatch {
            expected: expected_count,
            actual: rects.len(),
        });
    }

    info!(count = rects.len(), "Successfully parsed TEXTRECT file");
    Ok(rects)
}

/// Strip a trailing `#` comment from a line, returning the data portion.
fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(pos) => line[..pos].trim_end(),
        None => line,
    }
}

/// Parse the first 5 tokens of a data line into a [`TextRect`].
fn parse_rect_tokens(tokens: &[&str], file_line: usize) -> Result<TextRect, TextRectError> {
    if tokens.len() < 5 {
        return Err(TextRectError::TooFewFields {
            line: file_line,
            count: tokens.len(),
            text: tokens.join(" "),
        });
    }

    let parse_i32 = |idx: usize| -> Result<i32, TextRectError> {
        tokens[idx]
            .parse()
            .map_err(|_| TextRectError::InvalidField {
                line: file_line,
                token: tokens[idx].to_string(),
            })
    };

    let parse_usize = |idx: usize| -> Result<usize, TextRectError> {
        tokens[idx]
            .parse()
            .map_err(|_| TextRectError::InvalidField {
                line: file_line,
                token: tokens[idx].to_string(),
            })
    };

    Ok(TextRect {
        x1: parse_i32(0)?,
        y1: parse_i32(1)?,
        x2: parse_i32(2)?,
        y2: parse_i32(3)?,
        string_index: parse_usize(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn write_temp_textrect(content: &str) -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join("ow_data_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("test_textrect_{id}.dat"));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn parse_basic_textrect() {
        let content = "\
4 #lines to read\r\n\
225 356 250 369 57 #age\r\n\
250 356 275 369 57 # 1\r\n\
\r\n\
275 356 310 369 223 #ht\r\n\
300 356 330 369 223 # 3\r\n";

        let path = write_temp_textrect(content);
        let rects = parse_text_rects(&path).expect("should parse");

        assert_eq!(rects.len(), 4);

        assert_eq!(rects[0].x1, 225);
        assert_eq!(rects[0].y1, 356);
        assert_eq!(rects[0].x2, 250);
        assert_eq!(rects[0].y2, 369);
        assert_eq!(rects[0].string_index, 57);

        assert_eq!(rects[2].x1, 275);
        assert_eq!(rects[2].string_index, 223);
    }

    #[test]
    fn skips_comment_lines() {
        let content = "\
2 #count\n\
#this is a full comment line\n\
10 20 30 40 5 #label\n\
#another comment\n\
50 60 70 80 10\n";

        let path = write_temp_textrect(content);
        let rects = parse_text_rects(&path).expect("should parse");

        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].x1, 10);
        assert_eq!(rects[1].string_index, 10);
    }

    #[test]
    fn tab_separated_fields() {
        let content = "1 #count\n250 356 275 369\t57 # 1\n";
        let path = write_temp_textrect(content);
        let rects = parse_text_rects(&path).expect("should parse tabs");

        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].string_index, 57);
    }

    #[test]
    fn count_mismatch_is_error() {
        let content = "3 #count\n10 20 30 40 5\n";
        let path = write_temp_textrect(content);
        let err = parse_text_rects(&path).unwrap_err();
        assert!(matches!(
            err,
            TextRectError::CountMismatch {
                expected: 3,
                actual: 1
            }
        ));
    }

    #[test]
    fn empty_file_is_error() {
        let content = "";
        let path = write_temp_textrect(content);
        let err = parse_text_rects(&path).unwrap_err();
        assert!(matches!(err, TextRectError::Empty));
    }

    #[test]
    fn invalid_count_is_error() {
        let content = "abc #count\n10 20 30 40 5\n";
        let path = write_temp_textrect(content);
        let err = parse_text_rects(&path).unwrap_err();
        assert!(matches!(err, TextRectError::InvalidEntryCount { .. }));
    }
}
