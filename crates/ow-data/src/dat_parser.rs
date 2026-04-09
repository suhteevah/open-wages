//! Generic .dat file parser for Wages of War text-based data files.
//!
//! The original game's .dat files are plaintext (confirmed editable with Notepad++).
//! This module provides a general-purpose delimited text parser that will be
//! specialized per file once schemas are documented in Phase 1.

use std::collections::HashMap;
use std::path::Path;
use tracing::{info, debug, trace};

/// A single parsed record from a .dat file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatRecord {
    pub fields: Vec<String>,
    pub line_number: usize,
}

/// A fully parsed .dat file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatFile {
    pub headers: Option<Vec<String>>,
    pub records: Vec<DatRecord>,
    pub sections: HashMap<String, Vec<DatRecord>>,
    pub comment_count: usize,
}

/// Parse a text-based .dat file with the given delimiter.
pub fn parse_text_dat(path: &Path, delimiter: char) -> anyhow::Result<DatFile> {
    info!(path = %path.display(), delimiter = %delimiter, "Parsing text .dat file");
    let content = std::fs::read_to_string(path)?;
    let mut headers = None;
    let mut records = Vec::new();
    let mut sections: HashMap<String, Vec<DatRecord>> = HashMap::new();
    let mut current_section = "__default__".to_string();
    let mut comment_count = 0usize;

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') || trimmed.starts_with(';') || trimmed.starts_with("//") {
            comment_count += 1;
            debug!(line = line_num, comment = %trimmed, "Comment");
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed[1..trimmed.len() - 1].to_string();
            info!(line = line_num, section = %current_section, "Section");
            continue;
        }

        let fields = parse_delimited(trimmed, delimiter);
        trace!(line = line_num, fields = fields.len(), "Record");

        if headers.is_none()
            && fields
                .iter()
                .all(|f| f.chars().all(|c| c.is_alphabetic() || c == '_' || c == ' '))
        {
            debug!(line = line_num, "Detected header row");
            headers = Some(fields);
            continue;
        }

        let rec = DatRecord {
            fields,
            line_number: line_num,
        };
        records.push(rec.clone());
        sections
            .entry(current_section.clone())
            .or_default()
            .push(rec);
    }

    info!(records = records.len(), sections = sections.len(), comments = comment_count, "Done");
    Ok(DatFile {
        headers,
        records,
        sections,
        comment_count,
    })
}

fn parse_delimited(line: &str, delim: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == delim && !in_quotes {
            fields.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    fields.push(current.trim().to_string());
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_delimited_simple() {
        let fields = parse_delimited("hello,world,42", ',');
        assert_eq!(fields, vec!["hello", "world", "42"]);
    }

    #[test]
    fn test_parse_delimited_quoted() {
        let fields = parse_delimited(r#""Snake" Johnson,5000,78"#, ',');
        assert_eq!(fields, vec!["Snake Johnson", "5000", "78"]);
    }
}
