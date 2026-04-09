//! Parser for `TARGET.DAT` — combat hit probability lookup table.
//!
//! The file contains a 2D grid of space-separated integers (percentages 0–100).
//! Each row represents one axis of the to-hit calculation (e.g. range or skill
//! differential) and each column the other axis. The cell value is the base hit
//! probability before situational modifiers.
//!
//! Observed dimensions: ~885 rows x 20 columns. Row 0 is all 98s (point-blank /
//! maximum skill). Values decrease as row index increases.

use std::path::Path;

use tracing::{debug, info, trace, warn};

/// 2D hit-probability lookup table parsed from `TARGET.DAT`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HitTable {
    /// Row-major grid of hit percentages. `rows[r][c]` gives the probability.
    rows: Vec<Vec<u32>>,
}

impl HitTable {
    /// Look up a hit probability by `(row, col)`. Returns `None` if out of bounds.
    pub fn lookup(&self, row: usize, col: usize) -> Option<u32> {
        self.rows.get(row).and_then(|r| r.get(col)).copied()
    }

    /// Number of rows in the table.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Number of columns in the first row (all rows should be equal width).
    pub fn col_count(&self) -> usize {
        self.rows.first().map_or(0, |r| r.len())
    }
}

/// Errors that can occur while parsing `TARGET.DAT`.
#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("I/O error reading target file: {0}")]
    Io(#[from] std::io::Error),
    #[error("line {line}: failed to parse integer '{token}'")]
    InvalidInteger { line: usize, token: String },
    #[error("line {line}: row has {actual} columns, expected {expected}")]
    InconsistentColumns {
        line: usize,
        expected: usize,
        actual: usize,
    },
    #[error("TARGET.DAT is empty (no data rows)")]
    Empty,
}

/// Parse a `TARGET.DAT` hit-probability table into a [`HitTable`].
///
/// Each line is a row of whitespace-separated integers. Blank / whitespace-only
/// lines are skipped. CR/LF line endings are handled.
pub fn parse_hit_table(path: &Path) -> Result<HitTable, TargetError> {
    info!(path = %path.display(), "Parsing TARGET.DAT");

    let raw = std::fs::read_to_string(path)?;

    let mut rows: Vec<Vec<u32>> = Vec::new();
    let mut expected_cols: Option<usize> = None;

    for (i, line) in raw.lines().enumerate() {
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }

        let file_line = i + 1;
        let row: Vec<u32> = line
            .split_whitespace()
            .map(|tok| {
                tok.parse::<u32>().map_err(|_| TargetError::InvalidInteger {
                    line: file_line,
                    token: tok.to_string(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Validate consistent column count.
        match expected_cols {
            None => {
                expected_cols = Some(row.len());
                debug!(columns = row.len(), "Detected column count from first row");
            }
            Some(exp) if row.len() != exp => {
                warn!(
                    line = file_line,
                    expected = exp,
                    actual = row.len(),
                    "Inconsistent column count"
                );
                return Err(TargetError::InconsistentColumns {
                    line: file_line,
                    expected: exp,
                    actual: row.len(),
                });
            }
            _ => {}
        }

        trace!(line = file_line, cols = row.len(), "Parsed row");
        rows.push(row);
    }

    if rows.is_empty() {
        return Err(TargetError::Empty);
    }

    info!(
        rows = rows.len(),
        cols = expected_cols.unwrap_or(0),
        "Successfully parsed TARGET.DAT"
    );

    Ok(HitTable { rows })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn write_temp_target(content: &str) -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join("ow_data_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("test_target_{id}.dat"));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn parse_small_table() {
        let content = "\
 98 98 98 98 98\r\n\
 88 93 95 96 97\r\n\
 79 87 93 93 95\r\n";

        let path = write_temp_target(content);
        let table = parse_hit_table(&path).expect("should parse");

        assert_eq!(table.row_count(), 3);
        assert_eq!(table.col_count(), 5);

        assert_eq!(table.lookup(0, 0), Some(98));
        assert_eq!(table.lookup(1, 1), Some(93));
        assert_eq!(table.lookup(2, 4), Some(95));
        assert_eq!(table.lookup(3, 0), None); // out of bounds
        assert_eq!(table.lookup(0, 5), None); // out of bounds
    }

    #[test]
    fn skips_blank_lines() {
        let content = "\n 10 20 30\n\n 40 50 60\n\n";
        let path = write_temp_target(content);
        let table = parse_hit_table(&path).expect("should parse");

        assert_eq!(table.row_count(), 2);
        assert_eq!(table.lookup(0, 0), Some(10));
        assert_eq!(table.lookup(1, 2), Some(60));
    }

    #[test]
    fn inconsistent_columns_is_error() {
        let content = "10 20 30\n40 50\n";
        let path = write_temp_target(content);
        let err = parse_hit_table(&path).unwrap_err();
        assert!(matches!(err, TargetError::InconsistentColumns { .. }));
    }

    #[test]
    fn empty_file_is_error() {
        let content = "\n\n   \n";
        let path = write_temp_target(content);
        let err = parse_hit_table(&path).unwrap_err();
        assert!(matches!(err, TargetError::Empty));
    }

    #[test]
    fn invalid_integer_is_error() {
        let content = "10 abc 30\n";
        let path = write_temp_target(content);
        let err = parse_hit_table(&path).unwrap_err();
        assert!(matches!(err, TargetError::InvalidInteger { .. }));
    }
}
