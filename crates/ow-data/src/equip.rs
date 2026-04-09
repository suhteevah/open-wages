//! Parser for `EQUIP.DAT` — non-weapon equipment definitions.
//!
//! Each equipment item occupies two lines:
//! - Line 1: item name (plain text)
//! - Line 2: `PEN: <value>    ENC: <value>`
//!
//! The file terminates with a `~` sentinel line.

use std::path::Path;

use tracing::{debug, info, trace, warn};

/// A single equipment item parsed from `EQUIP.DAT`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Equipment {
    /// Item name (trimmed of surrounding whitespace).
    pub name: String,
    /// Penetration resistance (armor protection value). Zero for non-armor items.
    pub penetration: u32,
    /// Encumbrance (weight/bulk in inventory units).
    pub encumbrance: u32,
}

/// Errors that can occur while parsing `EQUIP.DAT`.
#[derive(Debug, thiserror::Error)]
pub enum EquipError {
    #[error("I/O error reading equipment file: {0}")]
    Io(#[from] std::io::Error),
    #[error("line {line}: missing PEN/ENC data line for item '{name}'")]
    MissingDataLine { line: usize, name: String },
    #[error("line {line}: failed to parse PEN value from '{text}'")]
    InvalidPen { line: usize, text: String },
    #[error("line {line}: failed to parse ENC value from '{text}'")]
    InvalidEnc { line: usize, text: String },
    #[error("line {line}: missing PEN field in '{text}'")]
    MissingPen { line: usize, text: String },
    #[error("line {line}: missing ENC field in '{text}'")]
    MissingEnc { line: usize, text: String },
}

/// Parse an `EQUIP.DAT` file into a list of [`Equipment`] items.
///
/// Reads line pairs (name + PEN/ENC data) until the `~` sentinel is reached.
/// CR/LF line endings are handled; item names are trimmed.
pub fn parse_equipment(path: &Path) -> Result<Vec<Equipment>, EquipError> {
    info!(path = %path.display(), "Parsing EQUIP.DAT");

    let raw = std::fs::read_to_string(path)?;

    // Strip \r and collect non-empty lines up to the ~ sentinel.
    let mut lines: Vec<(usize, &str)> = Vec::new();
    for (i, raw_line) in raw.split('\n').enumerate() {
        let line = raw_line.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed == "~" {
            debug!(line = i + 1, "Hit ~ sentinel, stopping");
            break;
        }
        if trimmed.is_empty() {
            continue;
        }
        lines.push((i + 1, line));
    }

    if !lines.len().is_multiple_of(2) {
        warn!(
            total_lines = lines.len(),
            "Odd number of content lines — last item may be incomplete"
        );
    }

    let mut items = Vec::new();
    let mut idx = 0;
    while idx + 1 < lines.len() {
        let (name_lineno, name_raw) = lines[idx];
        let (data_lineno, data_raw) = lines[idx + 1];

        let name = name_raw.trim().to_string();
        let pen = parse_pen(data_lineno, data_raw)?;
        let enc = parse_enc(data_lineno, data_raw)?;

        trace!(line = name_lineno, name = %name, pen, enc, "Parsed equipment item");

        items.push(Equipment {
            name,
            penetration: pen,
            encumbrance: enc,
        });

        idx += 2;
    }

    info!(count = items.len(), "Finished parsing EQUIP.DAT");
    Ok(items)
}

/// Extract the `PEN:` value from a data line.
fn parse_pen(line: usize, text: &str) -> Result<u32, EquipError> {
    let upper = text.to_uppercase();
    let pen_pos = upper
        .find("PEN:")
        .ok_or_else(|| EquipError::MissingPen {
            line,
            text: text.to_string(),
        })?;
    let after = &text[pen_pos + 4..];
    let token = after.split_whitespace().next().ok_or_else(|| EquipError::InvalidPen {
        line,
        text: text.to_string(),
    })?;
    token.parse::<u32>().map_err(|_| EquipError::InvalidPen {
        line,
        text: text.to_string(),
    })
}

/// Extract the `ENC:` value from a data line.
fn parse_enc(line: usize, text: &str) -> Result<u32, EquipError> {
    let upper = text.to_uppercase();
    let enc_pos = upper
        .find("ENC:")
        .ok_or_else(|| EquipError::MissingEnc {
            line,
            text: text.to_string(),
        })?;
    let after = &text[enc_pos + 4..];
    let token = after.split_whitespace().next().ok_or_else(|| EquipError::InvalidEnc {
        line,
        text: text.to_string(),
    })?;
    token.parse::<u32>().map_err(|_| EquipError::InvalidEnc {
        line,
        text: text.to_string(),
    })
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
    fn test_parse_basic() {
        let data = "Kevlar Vest\r\nPEN: 10    ENC: 20\r\nFirst Aid Kit\r\nPEN: 0    ENC: 5\r\n~\r\n";
        let f = write_temp_file(data);
        let items = parse_equipment(f.path()).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "Kevlar Vest");
        assert_eq!(items[0].penetration, 10);
        assert_eq!(items[0].encumbrance, 20);
        assert_eq!(items[1].name, "First Aid Kit");
        assert_eq!(items[1].penetration, 0);
        assert_eq!(items[1].encumbrance, 5);
    }

    #[test]
    fn test_trailing_whitespace_trimmed() {
        let data = "Parachute Canister   \r\nPEN: 0    ENC: 750\r\n~\r\n";
        let f = write_temp_file(data);
        let items = parse_equipment(f.path()).unwrap();
        assert_eq!(items[0].name, "Parachute Canister");
    }

    #[test]
    fn test_tab_separated_values() {
        let data = "Casino Chip\r\nPEN: 0\t  ENC: 0\r\n~\r\n";
        let f = write_temp_file(data);
        let items = parse_equipment(f.path()).unwrap();
        assert_eq!(items[0].penetration, 0);
        assert_eq!(items[0].encumbrance, 0);
    }

    #[test]
    fn test_blank_lines_after_sentinel() {
        let data = "Helmet\r\nPEN: 3    ENC: 7\r\n~\r\n\r\n\r\n";
        let f = write_temp_file(data);
        let items = parse_equipment(f.path()).unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_empty_file_with_sentinel() {
        let data = "~\r\n";
        let f = write_temp_file(data);
        let items = parse_equipment(f.path()).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_missing_enc_field() {
        let data = "Broken Item\r\nPEN: 5\r\n~\r\n";
        let f = write_temp_file(data);
        let result = parse_equipment(f.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let item = Equipment {
            name: "Test Armor".to_string(),
            penetration: 14,
            encumbrance: 8,
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: Equipment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, item.name);
        assert_eq!(back.penetration, item.penetration);
        assert_eq!(back.encumbrance, item.encumbrance);
    }
}
