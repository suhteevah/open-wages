//! Parser for `ENGWOW.DAT` — engine string table.
//!
//! One string per line, 1-based indexing (line 1 = index 1).
//! File terminates with a `~` sentinel line.
//!
//! Literal `\r` sequences and printf-style format specifiers (`%s`, `%d`, `%hd`)
//! are preserved verbatim in the parsed strings — they are interpreted at runtime.

use std::path::Path;

use tracing::{debug, info, trace};

/// A 1-based indexed string table loaded from `ENGWOW.DAT`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StringTable {
    /// Strings stored in order; index 0 corresponds to string index 1.
    entries: Vec<String>,
}

impl StringTable {
    /// Look up a string by its 1-based index.
    ///
    /// Returns `None` if the index is 0 or out of range.
    pub fn get(&self, index: usize) -> Option<&str> {
        if index == 0 {
            return None;
        }
        self.entries.get(index - 1).map(|s| s.as_str())
    }

    /// Total number of strings in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Errors that can occur while parsing `ENGWOW.DAT`.
#[derive(Debug, thiserror::Error)]
pub enum StringsError {
    #[error("I/O error reading string table: {0}")]
    Io(#[from] std::io::Error),
    #[error("file has no ~ sentinel — may be truncated")]
    NoSentinel,
}

/// Parse an `ENGWOW.DAT` string table file.
///
/// Each line (after stripping the CR/LF line ending) becomes one string entry.
/// Literal `\r` text, `%s`, `%d`, and `%hd` format specifiers are preserved.
/// Parsing stops at the `~` sentinel line.
pub fn parse_string_table(path: &Path) -> Result<StringTable, StringsError> {
    info!(path = %path.display(), "Parsing string table");

    let raw = std::fs::read_to_string(path)?;
    let mut entries = Vec::new();
    let mut found_sentinel = false;

    for (i, raw_line) in raw.split('\n').enumerate() {
        // Strip the actual carriage-return from CR/LF line endings,
        // but leave literal "\r" text sequences intact in the string content.
        let line = raw_line.trim_end_matches('\r');

        if line.trim() == "~" {
            debug!(line = i + 1, "Hit ~ sentinel, stopping");
            found_sentinel = true;
            break;
        }

        trace!(index = i + 1, len = line.len(), "String entry");
        entries.push(line.to_string());
    }

    if !found_sentinel {
        return Err(StringsError::NoSentinel);
    }

    info!(count = entries.len(), "Finished parsing string table");
    Ok(StringTable { entries })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_file(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn test_one_based_indexing() {
        let data = "Alpha\r\nBravo\r\nCharlie\r\n~\r\n";
        let f = write_temp_file(data);
        let table = parse_string_table(f.path()).unwrap();

        assert_eq!(table.len(), 3);
        assert_eq!(table.get(1), Some("Alpha"));
        assert_eq!(table.get(2), Some("Bravo"));
        assert_eq!(table.get(3), Some("Charlie"));
    }

    #[test]
    fn test_index_zero_returns_none() {
        let data = "Hello\r\n~\r\n";
        let f = write_temp_file(data);
        let table = parse_string_table(f.path()).unwrap();
        assert_eq!(table.get(0), None);
    }

    #[test]
    fn test_index_out_of_range() {
        let data = "Only one\r\n~\r\n";
        let f = write_temp_file(data);
        let table = parse_string_table(f.path()).unwrap();
        assert_eq!(table.get(2), None);
        assert_eq!(table.get(999), None);
    }

    #[test]
    fn test_format_specifiers_preserved() {
        let data = "Action Point cost is %d.\r\nFirst Aid has been applied to %s.\r\nPage #%hd  < MORE >\r\n~\r\n";
        let f = write_temp_file(data);
        let table = parse_string_table(f.path()).unwrap();

        assert!(table.get(1).unwrap().contains("%d"));
        assert!(table.get(2).unwrap().contains("%s"));
        assert!(table.get(3).unwrap().contains("%hd"));
    }

    #[test]
    fn test_literal_backslash_r_preserved() {
        // The literal text \r (two chars: backslash + r) should survive parsing.
        // This is NOT an actual carriage return byte — it's the text sequence.
        let data = "\\r           %s \\r Description\r\n~\r\n";
        let f = write_temp_file(data);
        let table = parse_string_table(f.path()).unwrap();

        let s = table.get(1).unwrap();
        assert!(s.contains("\\r"), "literal \\r should be preserved: {s}");
    }

    #[test]
    fn test_cr_lf_endings_stripped() {
        // The actual \r\n line ending should be stripped, leaving clean strings.
        let data = "Hello World\r\n~\r\n";
        let f = write_temp_file(data);
        let table = parse_string_table(f.path()).unwrap();
        assert_eq!(table.get(1), Some("Hello World"));
    }

    #[test]
    fn test_empty_table() {
        let data = "~\r\n";
        let f = write_temp_file(data);
        let table = parse_string_table(f.path()).unwrap();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn test_no_sentinel_is_error() {
        let data = "Orphan line\r\nAnother line\r\n";
        let f = write_temp_file(data);
        let result = parse_string_table(f.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_lines_preserved() {
        // Empty lines between content are valid string entries (empty strings).
        let data = "Line one\r\n\r\nLine three\r\n~\r\n";
        let f = write_temp_file(data);
        let table = parse_string_table(f.path()).unwrap();
        assert_eq!(table.len(), 3);
        assert_eq!(table.get(1), Some("Line one"));
        assert_eq!(table.get(2), Some(""));
        assert_eq!(table.get(3), Some("Line three"));
    }

    #[test]
    fn test_serde_roundtrip() {
        let table = StringTable {
            entries: vec!["hello".into(), "world".into()],
        };
        let json = serde_json::to_string(&table).unwrap();
        let back: StringTable = serde_json::from_str(&json).unwrap();
        assert_eq!(back.get(1), Some("hello"));
        assert_eq!(back.get(2), Some("world"));
    }
}
