//! Parser for `LOCK*.DAT`, `SERG*.DAT`, `ABDULS*.DAT`, and `FITZ*.DAT` —
//! per-mission equipment shop inventory files.
//!
//! Each item occupies two lines:
//! - Line 1: item name (plain text, may contain spaces and punctuation)
//! - Line 2: `STOCK: <n>  PRICE: <n>  STATUS:<status>  TYPE:<type>`
//!
//! The file terminates with an `Empty` item followed by two `~` sentinel lines.

use std::path::Path;

use tracing::{debug, info, trace};

/// Availability status for a shop item.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ItemStatus {
    /// Available for purchase (STOCK > 0).
    Stocked,
    /// Temporarily unavailable (STOCK = 0, may restock later).
    OutOfStock,
    /// Not available at this point in the campaign.
    Unavailable,
    /// Will become available in a future mission.
    ComingSoon,
    /// Permanently removed from inventory.
    Discontinued,
}

impl ItemStatus {
    /// Parse from the raw STATUS field value (case-insensitive).
    fn from_str(s: &str) -> Option<ItemStatus> {
        match s.to_uppercase().as_str() {
            "STOCKED" => Some(ItemStatus::Stocked),
            "OUTOFSTOCK" => Some(ItemStatus::OutOfStock),
            "UNAVAILABLE" => Some(ItemStatus::Unavailable),
            "COMINGSOON" => Some(ItemStatus::ComingSoon),
            "DISCONTINUED" => Some(ItemStatus::Discontinued),
            _ => None,
        }
    }
}

/// A single item in a shop inventory file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShopItem {
    /// Item display name (trimmed whitespace, preserves internal spaces/punctuation).
    pub name: String,
    /// Current quantity in stock.
    pub stock: u32,
    /// Cost per unit in game currency.
    pub price: u32,
    /// Availability status.
    pub status: ItemStatus,
    /// Item category (e.g., "WEAPON", "AMMO", "WEAPON2", "EQUIPMENT").
    pub item_type: String,
}

/// Complete parsed shop inventory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShopInventory {
    /// All items in the shop (excluding the `Empty` terminator entry).
    pub items: Vec<ShopItem>,
}

/// Errors that can occur while parsing a shop inventory file.
#[derive(Debug, thiserror::Error)]
pub enum ShopError {
    #[error("I/O error reading shop file: {0}")]
    Io(#[from] std::io::Error),
    #[error("line {line}: missing data line for item '{name}'")]
    MissingDataLine { line: usize, name: String },
    #[error("line {line}: missing STOCK field in '{text}'")]
    MissingStock { line: usize, text: String },
    #[error("line {line}: failed to parse STOCK value from '{text}'")]
    InvalidStock { line: usize, text: String },
    #[error("line {line}: missing PRICE field in '{text}'")]
    MissingPrice { line: usize, text: String },
    #[error("line {line}: failed to parse PRICE value from '{text}'")]
    InvalidPrice { line: usize, text: String },
    #[error("line {line}: missing STATUS field in '{text}'")]
    MissingStatus { line: usize, text: String },
    #[error("line {line}: unknown STATUS value '{value}'")]
    UnknownStatus { line: usize, value: String },
    #[error("line {line}: missing TYPE field in '{text}'")]
    MissingType { line: usize, text: String },
}

/// Extract the value after a `KEY:` token from a whitespace-separated attribute line.
///
/// Handles both `KEY:<value>` (no space) and `KEY: <value>` (with space) forms.
fn extract_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let upper = text.to_uppercase();
    let key_upper = key.to_uppercase();

    // Find the key position (case-insensitive).
    let key_with_colon = format!("{}:", key_upper);
    let pos = upper.find(&key_with_colon)?;
    let after_key = &text[pos + key_with_colon.len()..];

    // The value is the next whitespace-delimited token.
    let trimmed = after_key.trim_start();
    let end = trimmed.find(|c: char| c.is_whitespace()).unwrap_or(trimmed.len());
    if end == 0 {
        return None;
    }
    Some(&trimmed[..end])
}

/// Parse a shop inventory file (`LOCK*.DAT`, `SERG*.DAT`, `ABDULS*.DAT`, `FITZ*.DAT`).
///
/// Reads line pairs (item name + attribute data) until the `~` sentinel or
/// an `Empty` terminator entry is reached.
pub fn parse_shop_inventory(path: &Path) -> Result<ShopInventory, ShopError> {
    info!(path = %path.display(), "Parsing shop inventory file");

    let raw = std::fs::read_to_string(path)?;

    // Collect lines, stripping \r. Keep track of original 1-based line numbers.
    let all_lines: Vec<(usize, &str)> = raw
        .split('\n')
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_end_matches('\r')))
        .collect();

    // Filter to non-empty, non-sentinel lines, but stop at the first `~`.
    let mut lines: Vec<(usize, &str)> = Vec::new();
    for &(lineno, line) in &all_lines {
        let trimmed = line.trim();
        if trimmed == "~" {
            debug!(line = lineno, "Hit ~ sentinel, stopping");
            break;
        }
        if trimmed.is_empty() {
            continue;
        }
        lines.push((lineno, trimmed));
    }

    let mut items = Vec::new();
    let mut idx = 0;

    while idx + 1 < lines.len() {
        let (name_lineno, name_raw) = lines[idx];
        let (data_lineno, data_raw) = lines[idx + 1];

        let name = name_raw.trim().to_string();

        // Skip the Empty terminator entry.
        if name.eq_ignore_ascii_case("Empty") {
            debug!(line = name_lineno, "Reached Empty terminator entry, stopping");
            break;
        }

        trace!(line = name_lineno, name = %name, "Parsing shop item");

        // Parse STOCK.
        let stock_str = extract_field(data_raw, "STOCK").ok_or_else(|| ShopError::MissingStock {
            line: data_lineno,
            text: data_raw.to_string(),
        })?;
        let stock: u32 =
            stock_str
                .parse()
                .map_err(|_| ShopError::InvalidStock {
                    line: data_lineno,
                    text: stock_str.to_string(),
                })?;

        // Parse PRICE.
        let price_str = extract_field(data_raw, "PRICE").ok_or_else(|| ShopError::MissingPrice {
            line: data_lineno,
            text: data_raw.to_string(),
        })?;
        let price: u32 =
            price_str
                .parse()
                .map_err(|_| ShopError::InvalidPrice {
                    line: data_lineno,
                    text: price_str.to_string(),
                })?;

        // Parse STATUS.
        let status_str =
            extract_field(data_raw, "STATUS").ok_or_else(|| ShopError::MissingStatus {
                line: data_lineno,
                text: data_raw.to_string(),
            })?;
        let status = ItemStatus::from_str(status_str).ok_or_else(|| ShopError::UnknownStatus {
            line: data_lineno,
            value: status_str.to_string(),
        })?;

        // Parse TYPE.
        let item_type_str =
            extract_field(data_raw, "TYPE").ok_or_else(|| ShopError::MissingType {
                line: data_lineno,
                text: data_raw.to_string(),
            })?;
        let item_type = item_type_str.to_string();

        debug!(
            line = name_lineno,
            name = %name,
            stock,
            price,
            status = ?status,
            item_type = %item_type,
            "Parsed shop item"
        );

        items.push(ShopItem {
            name,
            stock,
            price,
            status,
            item_type,
        });

        idx += 2;
    }

    info!(count = items.len(), "Finished parsing shop inventory");
    Ok(ShopInventory { items })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_file(contents: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn parse_basic_inventory() {
        let data = "\
Colt Python\r\n\
STOCK: 5    PRICE: 3500    STATUS:STOCKED   TYPE:WEAPON\r\n\
9_x_33mmR\r\n\
STOCK: 20    PRICE: 44      STATUS:STOCKED   TYPE:AMMO\r\n\
Empty\r\n\
STOCK: 0    PRICE: 0       STATUS:EMPTY     TYPE:EMPTY\r\n\
~\r\n\
~\r\n";
        let f = write_temp_file(data);
        let inv = parse_shop_inventory(f.path()).unwrap();
        assert_eq!(inv.items.len(), 2);

        assert_eq!(inv.items[0].name, "Colt Python");
        assert_eq!(inv.items[0].stock, 5);
        assert_eq!(inv.items[0].price, 3500);
        assert_eq!(inv.items[0].status, ItemStatus::Stocked);
        assert_eq!(inv.items[0].item_type, "WEAPON");

        assert_eq!(inv.items[1].name, "9_x_33mmR");
        assert_eq!(inv.items[1].stock, 20);
        assert_eq!(inv.items[1].price, 44);
        assert_eq!(inv.items[1].status, ItemStatus::Stocked);
        assert_eq!(inv.items[1].item_type, "AMMO");
    }

    #[test]
    fn parse_all_status_variants() {
        let data = "\
Item A\r\n\
STOCK: 1 PRICE: 100 STATUS:STOCKED TYPE:WEAPON\r\n\
Item B\r\n\
STOCK: 0 PRICE: 200 STATUS:OUTOFSTOCK TYPE:AMMO\r\n\
Item C\r\n\
STOCK: 0 PRICE: 300 STATUS:UNAVAILABLE TYPE:WEAPON2\r\n\
Item D\r\n\
STOCK: 0 PRICE: 400 STATUS:COMINGSOON TYPE:EQUIPMENT\r\n\
Item E\r\n\
STOCK: 0 PRICE: 500 STATUS:DISCONTINUED TYPE:WEAPON\r\n\
Empty\r\n\
STOCK: 0 PRICE: 0 STATUS:EMPTY TYPE:EMPTY\r\n\
~\r\n\
~\r\n";
        let f = write_temp_file(data);
        let inv = parse_shop_inventory(f.path()).unwrap();
        assert_eq!(inv.items.len(), 5);
        assert_eq!(inv.items[0].status, ItemStatus::Stocked);
        assert_eq!(inv.items[1].status, ItemStatus::OutOfStock);
        assert_eq!(inv.items[2].status, ItemStatus::Unavailable);
        assert_eq!(inv.items[3].status, ItemStatus::ComingSoon);
        assert_eq!(inv.items[4].status, ItemStatus::Discontinued);
    }

    #[test]
    fn parse_variable_whitespace() {
        let data = "\
HK P7M13\r\n\
STOCK: 0   PRICE: 4400        STATUS:OUTOFSTOCK   TYPE:WEAPON\r\n\
Empty\r\n\
STOCK: 0    PRICE: 0       STATUS:EMPTY     TYPE:EMPTY\r\n\
~\r\n\
~\r\n";
        let f = write_temp_file(data);
        let inv = parse_shop_inventory(f.path()).unwrap();
        assert_eq!(inv.items.len(), 1);
        assert_eq!(inv.items[0].name, "HK P7M13");
        assert_eq!(inv.items[0].price, 4400);
    }

    #[test]
    fn parse_item_name_with_special_chars() {
        let data = "\
Colt Detective's Special\r\n\
STOCK: 3    PRICE: 2100    STATUS:STOCKED   TYPE:WEAPON\r\n\
S&W Model 29\r\n\
STOCK: 1   PRICE: 4700      STATUS:STOCKED   TYPE:WEAPON\r\n\
Empty\r\n\
STOCK: 0 PRICE: 0 STATUS:EMPTY TYPE:EMPTY\r\n\
~\r\n\
~\r\n";
        let f = write_temp_file(data);
        let inv = parse_shop_inventory(f.path()).unwrap();
        assert_eq!(inv.items.len(), 2);
        assert_eq!(inv.items[0].name, "Colt Detective's Special");
        assert_eq!(inv.items[1].name, "S&W Model 29");
    }

    #[test]
    fn stops_at_tilde_without_empty() {
        let data = "\
Uzi\r\n\
STOCK: 2 PRICE: 3700 STATUS:STOCKED TYPE:WEAPON\r\n\
~\r\n";
        let f = write_temp_file(data);
        let inv = parse_shop_inventory(f.path()).unwrap();
        assert_eq!(inv.items.len(), 1);
        assert_eq!(inv.items[0].name, "Uzi");
    }

    #[test]
    fn unknown_status_is_error() {
        let data = "\
Bad Item\r\n\
STOCK: 0 PRICE: 100 STATUS:BOGUS TYPE:WEAPON\r\n\
~\r\n";
        let f = write_temp_file(data);
        let err = parse_shop_inventory(f.path()).unwrap_err();
        assert!(matches!(err, ShopError::UnknownStatus { .. }));
    }

    #[test]
    fn empty_file_yields_empty_inventory() {
        let data = "~\r\n~\r\n";
        let f = write_temp_file(data);
        let inv = parse_shop_inventory(f.path()).unwrap();
        assert!(inv.items.is_empty());
    }
}
