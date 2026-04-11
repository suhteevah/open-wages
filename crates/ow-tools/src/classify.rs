use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::debug;

/// Known file format magic bytes
const MAGIC_TABLE: &[(&[u8], &str)] = &[
    (b"PK", "ZIP"),
    (b"\x1f\x8b", "GZIP"),
    (b"BM", "BMP"),
    (b"RIFF", "RIFF"),
    (b"MThd", "MIDI"),
    (b"\x89PNG", "PNG"),
    (b"MZ", "PE/DOS_EXE"),
    (b"\x00\x00\x01\x00", "ICO"),
    (b"\xff\xd8\xff", "JPEG"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileType {
    Text,
    Binary,
    Mixed,
    Known(String),
    Empty,
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileType::Text => write!(f, "text"),
            FileType::Binary => write!(f, "binary"),
            FileType::Mixed => write!(f, "mixed"),
            FileType::Known(fmt) => write!(f, "known:{fmt}"),
            FileType::Empty => write!(f, "empty"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub ext: String,
    pub size: u64,
    pub ascii_ratio: f64,
    pub null_ratio: f64,
    pub file_type: FileType,
    pub header_hex: String,
    pub header_ascii: String,
}

/// Detect known format from magic bytes in header.
pub fn detect_magic(header: &[u8]) -> Option<&'static str> {
    for (magic, fmt) in MAGIC_TABLE {
        if header.len() >= magic.len() && header.starts_with(magic) {
            return Some(fmt);
        }
    }
    None
}

/// Compute ratio of printable ASCII bytes in a buffer.
pub fn ascii_ratio(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let printable = data
        .iter()
        .filter(|&&b| (32..=126).contains(&b) || b == 9 || b == 10 || b == 13)
        .count();
    printable as f64 / data.len() as f64
}

/// Compute ratio of null bytes in a buffer.
pub fn null_ratio(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let nulls = data.iter().filter(|&&b| b == 0).count();
    nulls as f64 / data.len() as f64
}

/// Format bytes as hex string with spaces (e.g., "4d 5a 90 00").
pub fn hex_display(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format bytes as ASCII with non-printable replaced by '.'.
pub fn ascii_display(data: &[u8]) -> String {
    data.iter()
        .map(|&b| {
            if (32..=126).contains(&b) {
                b as char
            } else {
                '.'
            }
        })
        .collect()
}

/// Classify a file by reading its header.
pub fn classify_file(path: &Path, game_dir: &Path) -> anyhow::Result<FileInfo> {
    let metadata = std::fs::metadata(path)?;
    let size = metadata.len();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let rel_path = path
        .strip_prefix(game_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let mut header = vec![0u8; 512];
    let header_len = {
        use std::io::Read;
        let mut f = std::fs::File::open(path)?;
        f.read(&mut header)?
    };
    header.truncate(header_len);

    if header.is_empty() {
        return Ok(FileInfo {
            path: rel_path,
            ext,
            size,
            ascii_ratio: 0.0,
            null_ratio: 0.0,
            file_type: FileType::Empty,
            header_hex: String::new(),
            header_ascii: String::new(),
        });
    }

    let ar = ascii_ratio(&header);
    let nr = null_ratio(&header);
    let display_bytes = &header[..header.len().min(16)];

    let file_type = if let Some(fmt) = detect_magic(&header) {
        debug!(path = %rel_path, format = fmt, "Known format detected");
        FileType::Known(fmt.to_string())
    } else if ar > 0.85 && nr < 0.02 {
        FileType::Text
    } else if nr > 0.15 {
        FileType::Binary
    } else {
        FileType::Mixed
    };

    Ok(FileInfo {
        path: rel_path,
        ext,
        size,
        ascii_ratio: (ar * 1000.0).round() / 1000.0,
        null_ratio: (nr * 1000.0).round() / 1000.0,
        file_type,
        header_hex: hex_display(display_bytes),
        header_ascii: ascii_display(display_bytes),
    })
}
