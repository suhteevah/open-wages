//! Parser for `TARGET.DAT` — combat hit probability lookup table.
//!
//! The file contains multiple sections separated by blank lines. The first
//! section is a 2D grid of space-separated integers (percentages 0–100) forming
//! the primary hit-probability table. Each row represents one axis of the
//! to-hit calculation (e.g. range or skill differential) and each column the
//! other axis. The cell value is the base hit probability before situational
//! modifiers.
//!
//! After the primary table there are auxiliary sections with varying column
//! counts (range lookup tables, distance multipliers, etc.). These are stored
//! as raw `Vec<Vec<i64>>` since their exact semantics are still under
//! investigation.
//!
//! Observed primary table: 141 rows x 20 columns. Row 0 is all 98s
//! (point-blank / maximum skill). Values decrease as row index increases.

use std::path::Path;

use tracing::{debug, info, trace, warn};

/// A single section (table) from `TARGET.DAT`.
///
/// The primary hit table is always the first section. Auxiliary sections follow
/// and may have different column counts, including non-integer tokens
/// (period-separated floats). Non-parseable sections are skipped with a warning.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TargetSection {
    /// Row-major grid of values.
    pub rows: Vec<Vec<i64>>,
    /// Column count (consistent within this section).
    pub col_count: usize,
    /// 1-based line number where this section starts in the file.
    pub start_line: usize,
}

/// 2D hit-probability lookup table parsed from `TARGET.DAT`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HitTable {
    /// Row-major grid of hit percentages. `rows[r][c]` gives the probability.
    /// This is the primary table (first section in the file).
    rows: Vec<Vec<u32>>,
    /// Auxiliary sections that follow the primary table. These have varying
    /// column counts and may represent range tables, distance multipliers, etc.
    /// Their exact semantics are TBD.
    pub aux_sections: Vec<TargetSection>,
}

impl HitTable {
    /// Look up a hit probability by `(row, col)`. Returns `None` if out of bounds.
    pub fn lookup(&self, row: usize, col: usize) -> Option<u32> {
        self.rows.get(row).and_then(|r| r.get(col)).copied()
    }

    /// Number of rows in the primary table.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Number of columns in the primary table.
    pub fn col_count(&self) -> usize {
        self.rows.first().map_or(0, |r| r.len())
    }

    /// Number of auxiliary sections parsed after the primary table.
    pub fn aux_section_count(&self) -> usize {
        self.aux_sections.len()
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
/// The file is split into sections by blank lines. The first section must be a
/// consistent-width grid of unsigned integers — the primary hit table. Subsequent
/// sections are parsed as signed integer grids where possible; sections containing
/// non-integer tokens (e.g. period-separated floats) are skipped with a warning.
pub fn parse_hit_table(path: &Path) -> Result<HitTable, TargetError> {
    info!(path = %path.display(), "Parsing TARGET.DAT");

    let raw = std::fs::read_to_string(path)?;

    // Split file into sections separated by one or more blank lines.
    // Each section is a Vec of (1-based line number, trimmed line content).
    let mut sections: Vec<Vec<(usize, &str)>> = Vec::new();
    let mut current_section: Vec<(usize, &str)> = Vec::new();

    for (i, line) in raw.lines().enumerate() {
        let trimmed = line.trim_end_matches('\r').trim();
        let file_line = i + 1;

        if trimmed.is_empty() {
            if !current_section.is_empty() {
                sections.push(std::mem::take(&mut current_section));
            }
        } else {
            current_section.push((file_line, trimmed));
        }
    }
    if !current_section.is_empty() {
        sections.push(current_section);
    }

    if sections.is_empty() {
        return Err(TargetError::Empty);
    }

    // --- Parse the primary hit table (first section) ---
    let primary_lines = &sections[0];
    let mut rows: Vec<Vec<u32>> = Vec::new();
    let mut expected_cols: Option<usize> = None;

    for &(file_line, line) in primary_lines {
        let row: Vec<u32> = line
            .split_whitespace()
            .map(|tok| {
                tok.parse::<u32>().map_err(|_| TargetError::InvalidInteger {
                    line: file_line,
                    token: tok.to_string(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        match expected_cols {
            None => {
                expected_cols = Some(row.len());
                debug!(columns = row.len(), "Primary table column count");
            }
            Some(exp) if row.len() != exp => {
                warn!(
                    line = file_line,
                    expected = exp,
                    actual = row.len(),
                    "Inconsistent column count in primary table"
                );
                return Err(TargetError::InconsistentColumns {
                    line: file_line,
                    expected: exp,
                    actual: row.len(),
                });
            }
            _ => {}
        }

        trace!(line = file_line, cols = row.len(), "Parsed primary row");
        rows.push(row);
    }

    info!(
        rows = rows.len(),
        cols = expected_cols.unwrap_or(0),
        "Parsed primary hit table"
    );

    // --- Parse auxiliary sections (sections[1..]) ---
    // These have variable column counts and may contain non-integer tokens.
    let mut aux_sections: Vec<TargetSection> = Vec::new();

    for (sec_idx, sec_lines) in sections[1..].iter().enumerate() {
        let start_line = sec_lines.first().map_or(0, |&(l, _)| l);
        let mut sec_rows: Vec<Vec<i64>> = Vec::new();
        let mut sec_cols: Option<usize> = None;
        let mut skip = false;

        for &(file_line, line) in sec_lines {
            let tokens: Vec<&str> = line.split_whitespace().collect();

            // Try to parse all tokens as i64. If any fail, skip this section
            // (it likely contains period-separated float values).
            let parsed: Result<Vec<i64>, _> = tokens
                .iter()
                .map(|tok| tok.parse::<i64>())
                .collect();

            match parsed {
                Ok(row) => {
                    // Validate consistent column count within this section.
                    match sec_cols {
                        None => {
                            sec_cols = Some(row.len());
                        }
                        Some(exp) if row.len() != exp => {
                            // Column count changed mid-section — end this section
                            // and the remaining lines will be dropped.
                            warn!(
                                line = file_line,
                                section = sec_idx + 1,
                                expected = exp,
                                actual = row.len(),
                                "Aux section column count changed; truncating section"
                            );
                            break;
                        }
                        _ => {}
                    }
                    sec_rows.push(row);
                }
                Err(_) => {
                    // Non-integer tokens (e.g. "154.156.157.158..." floats).
                    trace!(
                        line = file_line,
                        section = sec_idx + 1,
                        "Skipping aux section: non-integer token"
                    );
                    skip = true;
                    break;
                }
            }
        }

        if !skip && !sec_rows.is_empty() {
            let col_count = sec_cols.unwrap_or(0);
            debug!(
                section = sec_idx + 1,
                rows = sec_rows.len(),
                cols = col_count,
                start_line,
                "Parsed auxiliary section"
            );
            aux_sections.push(TargetSection {
                rows: sec_rows,
                col_count,
                start_line,
            });
        }
    }

    info!(
        primary_rows = rows.len(),
        aux_sections = aux_sections.len(),
        "Successfully parsed TARGET.DAT"
    );

    Ok(HitTable { rows, aux_sections })
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
        assert_eq!(table.aux_section_count(), 0);

        assert_eq!(table.lookup(0, 0), Some(98));
        assert_eq!(table.lookup(1, 1), Some(93));
        assert_eq!(table.lookup(2, 4), Some(95));
        assert_eq!(table.lookup(3, 0), None); // out of bounds
        assert_eq!(table.lookup(0, 5), None); // out of bounds
    }

    #[test]
    fn blank_lines_separate_sections() {
        // Primary table is 2 rows x 3 cols. A blank-line gap, then an aux
        // section with a different column count.
        let content = " 10 20 30\n 40 50 60\n\n 1 2 3 4 5\n 6 7 8 9 10\n";
        let path = write_temp_target(content);
        let table = parse_hit_table(&path).expect("should parse");

        assert_eq!(table.row_count(), 2);
        assert_eq!(table.col_count(), 3);
        assert_eq!(table.lookup(0, 0), Some(10));
        assert_eq!(table.lookup(1, 2), Some(60));

        // Aux section should be captured.
        assert_eq!(table.aux_section_count(), 1);
        assert_eq!(table.aux_sections[0].col_count, 5);
        assert_eq!(table.aux_sections[0].rows.len(), 2);
        assert_eq!(table.aux_sections[0].rows[0], vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn non_integer_aux_sections_skipped() {
        // Primary table followed by a section with period-separated floats
        // (mimics the real TARGET.DAT structure).
        let content = "\
10 20 30\n\
40 50 60\n\
\n\
154.156.157.158\n\
141.142.143.145\n\
\n\
1 2 3\n\
4 5 6\n";
        let path = write_temp_target(content);
        let table = parse_hit_table(&path).expect("should parse");

        assert_eq!(table.row_count(), 2);
        assert_eq!(table.col_count(), 3);
        // The float section should be skipped; only the integer section kept.
        assert_eq!(table.aux_section_count(), 1);
        assert_eq!(table.aux_sections[0].rows[0], vec![1, 2, 3]);
    }

    #[test]
    fn inconsistent_columns_in_primary_is_error() {
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
    fn invalid_integer_in_primary_is_error() {
        let content = "10 abc 30\n";
        let path = write_temp_target(content);
        let err = parse_hit_table(&path).unwrap_err();
        assert!(matches!(err, TargetError::InvalidInteger { .. }));
    }

    #[test]
    fn multiple_aux_sections() {
        // Primary table, then three aux sections with different widths.
        let content = "\
10 20\n\
30 40\n\
\n\
1 2 3\n\
\n\
100 200 300 400\n\
500 600 700 800\n\
\n\
-1 -2\n";
        let path = write_temp_target(content);
        let table = parse_hit_table(&path).expect("should parse");

        assert_eq!(table.row_count(), 2);
        assert_eq!(table.col_count(), 2);
        assert_eq!(table.aux_section_count(), 3);
        assert_eq!(table.aux_sections[0].col_count, 3);
        assert_eq!(table.aux_sections[1].col_count, 4);
        assert_eq!(table.aux_sections[1].rows.len(), 2);
        assert_eq!(table.aux_sections[2].col_count, 2);
        assert_eq!(table.aux_sections[2].rows[0], vec![-1, -2]);
    }
}
