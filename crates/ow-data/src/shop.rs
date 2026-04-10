//! Parser for `LOCK*.DAT`, `SERG*.DAT`, `ABDULS*.DAT`, and `FITZ*.DAT` —
//! per-mission equipment shop inventory files.
//!
//! # Shop System Overview
//!
//! Wages of War has 3 arms dealers plus a personal locker, each with a separate
//! inventory file per mission:
//!
//! - **SERG*.DAT** — Serg's shop. General-purpose arms dealer, broadest selection.
//! - **ABDULS*.DAT** — Abdul's shop. Tends toward heavier/exotic hardware.
//! - **FITZ*.DAT** — Fitz's shop. Specialty items, often mission-specific.
//! - **LOCK*.DAT** — The player's personal locker. Stores purchased equipment
//!   between missions. Items bought at any shop end up here if not equipped.
//!
//! The `*` suffix is a zero-padded mission number (e.g., `SERG01.DAT` for mission 1).
//! Each shop's inventory changes between missions — items cycle through status
//! values (Stocked -> OutOfStock -> ComingSoon -> Stocked again, or Discontinued
//! permanently). This creates urgency: buy gear when it's available, because it
//! may vanish next mission.
//!
//! # File Format
//!
//! Each item occupies two lines:
//! - Line 1: item name (plain text, may contain spaces and punctuation)
//! - Line 2: `STOCK: <n>  PRICE: <n>  STATUS:<status>  TYPE:<type>`
//!
//! The file terminates with an `Empty` item followed by two `~` sentinel lines.
//! The `~` sentinel is a common pattern across Wages of War data files — it marks
//! the definitive end of a data section and prevents runaway parsing.

use std::path::Path;

use tracing::{debug, info, trace};

/// Availability status for a shop item.
///
/// Items cycle through these statuses across missions to simulate a dynamic
/// arms market. The original game uses this to gate progression — late-game
/// weapons appear as ComingSoon early on, then become Stocked, while early
/// weapons may eventually be Discontinued as the war escalates.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ItemStatus {
    /// Available for purchase (STOCK > 0).
    Stocked,
    /// Temporarily unavailable (STOCK = 0, may restock later).
    OutOfStock,
    /// Not available at this point in the campaign.
    Unavailable,
    /// Will become available in a future mission. Shown in the UI as a teaser
    /// so the player knows to check back — creates anticipation for upgrades.
    ComingSoon,
    /// Permanently removed from inventory. Once discontinued, the item never
    /// returns to this shop — the player must rely on looted copies or the locker.
    Discontinued,
}

impl ItemStatus {
    /// Parse from the raw STATUS field value (case-insensitive).
    ///
    /// We don't implement `std::str::FromStr` because the original data files
    /// are inconsistent about casing — some missions use "Stocked", others
    /// "STOCKED". A manual case-insensitive match is more robust here.
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
///
/// Names can contain apostrophes, ampersands, and underscores — e.g.,
/// "Colt Detective's Special", "S&W Model 29", "9_x_33mmR". The parser
/// preserves these exactly as they appear in the data file because they
/// serve as lookup keys into the weapon/ammo definition files.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShopItem {
    /// Item display name (trimmed whitespace, preserves internal spaces/punctuation).
    /// This must match the name in the weapon/ammo .DAT files exactly for cross-referencing.
    pub name: String,
    /// Current quantity in stock. The original game decrements this on purchase
    /// and does not allow buying more than what's stocked.
    pub stock: u32,
    /// Cost per unit in game currency (dollars). Prices vary per shop —
    /// the same weapon can cost different amounts at Serg's vs Abdul's.
    pub price: u32,
    /// Availability status. Determines whether the item appears in the shop UI
    /// and whether the "Buy" button is enabled.
    pub status: ItemStatus,
    /// Item category (e.g., "WEAPON", "AMMO", "WEAPON2", "EQUIPMENT").
    /// "WEAPON2" appears to denote secondary/sidearm-class weapons in some files,
    /// but the distinction is not consistently applied across all missions.
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
/// The original data files are inconsistent about whitespace around colons —
/// STATUS and TYPE typically have no space (`STATUS:STOCKED`), while STOCK and
/// PRICE usually do (`STOCK: 5`). This function handles both transparently.
fn extract_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let upper = text.to_uppercase();
    let key_upper = key.to_uppercase();

    // Case-insensitive search: we uppercase both the line and the key, then
    // find the key position in the uppercased copy but slice from the original
    // text to preserve the original casing of the value.
    let key_with_colon = format!("{}:", key_upper);
    let pos = upper.find(&key_with_colon)?;
    let after_key = &text[pos + key_with_colon.len()..];

    // The value is the next whitespace-delimited token. We trim leading spaces
    // to handle both `KEY:val` and `KEY: val` formats uniformly.
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

    // Collect lines, stripping \r for Windows line endings (the original game
    // shipped on DOS/Windows, so all data files use CRLF). We track 1-based
    // line numbers for error reporting so users can find problems in Notepad++.
    let all_lines: Vec<(usize, &str)> = raw
        .split('\n')
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_end_matches('\r')))
        .collect();

    // Filter to non-empty, non-sentinel lines, but stop at the first `~`.
    // The `~` sentinel marks end-of-data. Some files have two `~` lines (one
    // after the Empty terminator, one as a true EOF marker), but we stop at
    // the first one since everything after it is padding.
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

    // Process lines in pairs: odd lines are item names, even lines are attribute
    // data. This two-line-per-item format is consistent across all shop files.
    while idx + 1 < lines.len() {
        let (name_lineno, name_raw) = lines[idx];
        let (data_lineno, data_raw) = lines[idx + 1];

        let name = name_raw.trim().to_string();

        // The "Empty" entry is a sentinel that marks the end of real inventory.
        // It always has STOCK:0, PRICE:0, STATUS:EMPTY, TYPE:EMPTY — none of
        // which are valid gameplay values. We stop here rather than trying to
        // parse the EMPTY status (which would fail our status enum).
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

        // Advance by 2 lines (name + data) to reach the next item pair.
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
