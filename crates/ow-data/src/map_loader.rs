//! # MAP File Parser
//!
//! Parses the binary `.MAP` files used by Wages of War for scenario tile grids.
//! All MAP files are exactly 248,384 bytes with a fixed layout:
//!
//! | Region | Offset | Size | Content |
//! |--------|--------|------|---------|
//! | Tile grid | `0x00000` | 201,600 B | 200 x 252 cells, 4 bytes each |
//! | String table | `0x31380` | 656 B | 4 null-padded path strings (164 B each) |
//! | Metadata | `0x31610` | 46,128 B | Map properties + elevation/terrain layer |
//!
//! ## Tile grid encoding
//!
//! Each cell is 4 bytes interpreted as a single little-endian `u32`.
//! The 32-bit word packs **three 9-bit tile layer indices** plus **5 flag bits**:
//!
//! ```text
//! [31..23] tile_layer_0 (9 bits — primary terrain sprite index, 0-511)
//! [22..14] tile_layer_1 (9 bits — secondary terrain overlay, 0-511)
//! [13..5]  tile_layer_2 (9 bits — tertiary terrain detail, 0-511)
//! [4]      flag_A (wall/obstacle)
//! [3]      flag_B (explored)
//! [2]      flag_C (roof)
//! [1]      flag_D (walkable)
//! [0]      padding
//! ```
//!
//! This layout was confirmed by RE analysis of the `PackCellWord1` function
//! at `0x41AF7B` in `Wow.exe`, which uses three successive 9-bit SHL-OR
//! operations followed by five 1-bit flag insertions.
//!
//! Border cells are identified by the raw u32 value having the high byte
//! of the first on-disk u16 equal to `0xFF` (legacy detection preserved
//! for compatibility — equivalent to checking `(raw_bytes[1] == 0xFF)`).
//!
//! The grid is 200 columns x 252 rows. Rows 0..201 contain active map data;
//! rows 202..251 are border padding filled with `0xFF` in byte 1.
//!
//! ## String table
//!
//! Four 164-byte null-padded fields referencing original build paths:
//! 1. Tile sprite sheet (`.TIL`)
//! 2. Tile metadata (`TILES*.DAT`)
//! 3. Object sprite sheet (`.OBJ`)
//! 4. Object metadata (`OBJ*.DAT`)

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor};
use std::path::Path;
use tracing::{debug, trace};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Total file size of every MAP file.
const MAP_FILE_SIZE: usize = 248_384;

/// Grid width in cells.
const GRID_WIDTH: usize = 200;

/// Grid height in cells (including border padding rows).
const GRID_HEIGHT: usize = 252;

/// Active (non-border) rows in the grid.
const GRID_ACTIVE_ROWS: usize = 202;

/// Bytes per tile cell.
const CELL_SIZE: usize = 4;

/// Total tile grid region size in bytes.
const TILE_GRID_SIZE: usize = GRID_WIDTH * GRID_HEIGHT * CELL_SIZE;

/// File offset where the string table begins.
const STRING_TABLE_OFFSET: usize = 0x31380;

/// Fixed width of each string table entry (null-padded).
const STRING_ENTRY_SIZE: usize = 164;

/// Number of string table entries.
const STRING_ENTRY_COUNT: usize = 4;

/// File offset where the metadata footer begins.
const METADATA_OFFSET: usize = STRING_TABLE_OFFSET + STRING_ENTRY_COUNT * STRING_ENTRY_SIZE;

/// Size of the metadata section.
const METADATA_SIZE: usize = MAP_FILE_SIZE - METADATA_OFFSET;

/// Cell flag byte 0 value indicating an unused/border cell.
const BORDER_MARKER: u8 = 0xFF;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur when parsing MAP files.
#[derive(Debug, thiserror::Error)]
pub enum MapError {
    #[error("I/O error reading MAP file: {0}")]
    Io(#[from] io::Error),

    #[error("invalid MAP file size: expected {MAP_FILE_SIZE} bytes, got {0}")]
    BadFileSize(usize),
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Header / dimensional info for a parsed map.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MapHeader {
    /// Grid width in cells (always 200).
    pub width: u32,
    /// Grid height in cells including padding (always 252).
    pub height: u32,
    /// Number of active (non-border) rows (always 202).
    pub active_rows: u32,
}

/// A single tile cell from the grid.
///
/// Each cell is a packed 32-bit word containing three 9-bit tile layer
/// indices and 5 flag bits. See module docs for the bit layout.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct MapTile {
    /// Primary terrain tile index (bits 31..23, 9 bits, 0-511).
    pub layer0: u16,
    /// Secondary terrain overlay index (bits 22..14, 9 bits, 0-511).
    pub layer1: u16,
    /// Tertiary terrain detail index (bits 13..5, 9 bits, 0-511).
    pub layer2: u16,
    /// Five-bit flag field (bits 4..0).
    pub flags: u8,
    /// Whether this cell is a border/unused cell (byte 1 of the on-disk cell == 0xFF).
    pub is_border: bool,
}

/// References to associated asset files, extracted from the string table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MapAssetRefs {
    /// Path to the tile sprite sheet (`.TIL`).
    pub tileset_path: String,
    /// Path to the tile metadata file (`TILES*.DAT`).
    pub tile_meta_path: String,
    /// Path to the object sprite sheet (`.OBJ`).
    pub object_sprite_path: String,
    /// Path to the object metadata file (`OBJ*.DAT`).
    pub object_meta_path: String,
}

/// A fully parsed MAP file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameMap {
    /// Dimensional header.
    pub header: MapHeader,
    /// Tile grid as a flat vec, row-major order (width * height entries).
    pub tiles: Vec<MapTile>,
    /// Asset file references from the string table.
    pub asset_refs: MapAssetRefs,
    /// Raw metadata footer (unparsed — format partially understood).
    pub metadata: Vec<u8>,
}

impl GameMap {
    /// Get the tile at grid position `(x, y)`. Returns `None` if out of bounds.
    pub fn get_tile(&self, x: usize, y: usize) -> Option<&MapTile> {
        if x >= GRID_WIDTH || y >= GRID_HEIGHT {
            return None;
        }
        Some(&self.tiles[y * GRID_WIDTH + x])
    }

    /// Grid width in cells.
    pub fn width(&self) -> usize {
        GRID_WIDTH
    }

    /// Grid height in cells (including border rows).
    pub fn height(&self) -> usize {
        GRID_HEIGHT
    }

    /// Number of active (non-border) rows.
    pub fn active_rows(&self) -> usize {
        GRID_ACTIVE_ROWS
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse a MAP file from disk.
///
/// The file must be exactly 248,384 bytes.
pub fn parse_map(path: &Path) -> Result<GameMap, MapError> {
    debug!(path = %path.display(), "parsing MAP file");

    let data = std::fs::read(path)?;
    parse_map_bytes(&data, path)
}

/// Parse a MAP file from a byte buffer. `path` is used only for log messages.
pub fn parse_map_bytes(data: &[u8], path: &Path) -> Result<GameMap, MapError> {
    if data.len() != MAP_FILE_SIZE {
        return Err(MapError::BadFileSize(data.len()));
    }

    // --- Tile grid ---
    let tiles = parse_tile_grid(&data[..TILE_GRID_SIZE]);

    // Count non-border tiles for logging
    let active_count = tiles.iter().filter(|t| !t.is_border).count();
    debug!(
        total_cells = GRID_WIDTH * GRID_HEIGHT,
        active_cells = active_count,
        border_cells = tiles.len() - active_count,
        "tile grid parsed"
    );

    // --- String table ---
    let asset_refs = parse_string_table(&data[STRING_TABLE_OFFSET..]);
    debug!(
        tileset = %asset_refs.tileset_path,
        tile_meta = %asset_refs.tile_meta_path,
        obj_sprite = %asset_refs.object_sprite_path,
        obj_meta = %asset_refs.object_meta_path,
        "asset references parsed"
    );

    // --- Metadata footer ---
    let metadata = data[METADATA_OFFSET..].to_vec();
    debug_assert_eq!(metadata.len(), METADATA_SIZE);
    trace!(metadata_size = metadata.len(), "metadata footer captured (unparsed)");

    let header = MapHeader {
        width: GRID_WIDTH as u32,
        height: GRID_HEIGHT as u32,
        active_rows: GRID_ACTIVE_ROWS as u32,
    };

    debug!(path = %path.display(), "MAP file parsed successfully");

    Ok(GameMap {
        header,
        tiles,
        asset_refs,
        metadata,
    })
}

/// Parse the tile grid region into a vec of `MapTile`.
///
/// Each cell is read as a single little-endian `u32` and unpacked into
/// three 9-bit tile layer indices plus a 5-bit flag field.
fn parse_tile_grid(data: &[u8]) -> Vec<MapTile> {
    let cell_count = GRID_WIDTH * GRID_HEIGHT;
    let mut tiles = Vec::with_capacity(cell_count);

    for i in 0..cell_count {
        let offset = i * CELL_SIZE;
        let mut cursor = Cursor::new(&data[offset..offset + CELL_SIZE]);

        let raw = cursor.read_u32::<LittleEndian>().unwrap();

        // Unpack three 9-bit layer indices and 5 flag bits:
        //   [31..23] layer0, [22..14] layer1, [13..5] layer2, [4..0] flags
        let layer0 = ((raw >> 23) & 0x1FF) as u16;
        let layer1 = ((raw >> 14) & 0x1FF) as u16;
        let layer2 = ((raw >> 5) & 0x1FF) as u16;
        let flags = (raw & 0x1F) as u8;

        // Border detection: byte 1 of the on-disk 4-byte cell == 0xFF.
        // In the LE u32, byte 1 is bits [15..8].
        let is_border = ((raw >> 8) & 0xFF) as u8 == BORDER_MARKER;

        tiles.push(MapTile {
            layer0,
            layer1,
            layer2,
            flags,
            is_border,
        });

        if i < 4 {
            trace!(
                cell = i,
                raw = format_args!("0x{raw:08X}"),
                layer0,
                layer1,
                layer2,
                flags = format_args!("0x{flags:02X}"),
                is_border,
                "tile cell"
            );
        }
    }

    tiles
}

/// Parse the string table (4 x 164-byte null-padded entries).
fn parse_string_table(data: &[u8]) -> MapAssetRefs {
    let read_entry = |idx: usize| -> String {
        let start = idx * STRING_ENTRY_SIZE;
        let end = start + STRING_ENTRY_SIZE;
        let slice = &data[start..end];
        // Find first null byte
        let len = slice.iter().position(|&b| b == 0).unwrap_or(STRING_ENTRY_SIZE);
        String::from_utf8_lossy(&slice[..len]).to_string()
    };

    MapAssetRefs {
        tileset_path: read_entry(0),
        tile_meta_path: read_entry(1),
        object_sprite_path: read_entry(2),
        object_meta_path: read_entry(3),
    }
}

/// Extract just the filename from a Windows-style path string.
///
/// Useful for resolving the original `C:\WOW\...` paths to local filenames.
pub fn filename_from_build_path(build_path: &str) -> &str {
    build_path
        .rsplit('\\')
        .next()
        .unwrap_or(build_path)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid MAP buffer for testing.
    fn make_test_map() -> Vec<u8> {
        let mut data = vec![0u8; MAP_FILE_SIZE];

        // Write a known tile at (0, 0):
        //   layer0=7, layer1=3, layer2=1, flags=0x0A
        //   Packed: (7 << 23) | (3 << 14) | (1 << 5) | 0x0A
        //         = 0x0380_C02A
        let cell0: u32 = (7 << 23) | (3 << 14) | (1 << 5) | 0x0A;
        data[0..4].copy_from_slice(&cell0.to_le_bytes());

        // Write a border cell at row 202, col 0
        // Border is detected by byte 1 == 0xFF, i.e. bits [15..8] of the u32.
        let border_offset = (202 * GRID_WIDTH) * CELL_SIZE;
        let border_cell: u32 = 0x0000_FF00; // byte 1 = 0xFF
        data[border_offset..border_offset + 4].copy_from_slice(&border_cell.to_le_bytes());

        // Write string table entries
        let tileset = b"C:\\WOW\\SPR\\SCEN1\\TILSCN01.TIL";
        let tile_meta = b"C:\\WOW\\SPR\\SCEN1\\TILES1.DAT";
        let obj_sprite = b"C:\\WOW\\SPR\\SCEN1\\SCEN1.OBJ";
        let obj_meta = b"C:\\WOW\\SPR\\SCEN1\\OBJ01.DAT";

        for (i, entry) in [tileset.as_ref(), tile_meta, obj_sprite, obj_meta].iter().enumerate() {
            let offset = STRING_TABLE_OFFSET + i * STRING_ENTRY_SIZE;
            data[offset..offset + entry.len()].copy_from_slice(entry);
        }

        data
    }

    #[test]
    fn parse_known_tile() {
        let data = make_test_map();
        let map = parse_map_bytes(&data, Path::new("test.MAP")).unwrap();

        let tile = map.get_tile(0, 0).unwrap();
        assert_eq!(tile.layer0, 7);
        assert_eq!(tile.layer1, 3);
        assert_eq!(tile.layer2, 1);
        assert_eq!(tile.flags, 0x0A);
        assert!(!tile.is_border);
    }

    #[test]
    fn parse_border_cell() {
        let data = make_test_map();
        let map = parse_map_bytes(&data, Path::new("test.MAP")).unwrap();

        let tile = map.get_tile(0, 202).unwrap();
        assert!(tile.is_border);
    }

    #[test]
    fn parse_string_table_entries() {
        let data = make_test_map();
        let map = parse_map_bytes(&data, Path::new("test.MAP")).unwrap();

        assert_eq!(map.asset_refs.tileset_path, r"C:\WOW\SPR\SCEN1\TILSCN01.TIL");
        assert_eq!(map.asset_refs.tile_meta_path, r"C:\WOW\SPR\SCEN1\TILES1.DAT");
        assert_eq!(map.asset_refs.object_sprite_path, r"C:\WOW\SPR\SCEN1\SCEN1.OBJ");
        assert_eq!(map.asset_refs.object_meta_path, r"C:\WOW\SPR\SCEN1\OBJ01.DAT");
    }

    #[test]
    fn filename_extraction() {
        assert_eq!(filename_from_build_path(r"C:\WOW\SPR\SCEN1\TILSCN01.TIL"), "TILSCN01.TIL");
        assert_eq!(filename_from_build_path("TILES1.DAT"), "TILES1.DAT");
        assert_eq!(filename_from_build_path(""), "");
    }

    #[test]
    fn header_dimensions() {
        let data = make_test_map();
        let map = parse_map_bytes(&data, Path::new("test.MAP")).unwrap();

        assert_eq!(map.header.width, 200);
        assert_eq!(map.header.height, 252);
        assert_eq!(map.header.active_rows, 202);
        assert_eq!(map.width(), 200);
        assert_eq!(map.height(), 252);
        assert_eq!(map.active_rows(), 202);
    }

    #[test]
    fn out_of_bounds_returns_none() {
        let data = make_test_map();
        let map = parse_map_bytes(&data, Path::new("test.MAP")).unwrap();

        assert!(map.get_tile(200, 0).is_none());
        assert!(map.get_tile(0, 252).is_none());
        assert!(map.get_tile(300, 300).is_none());
    }

    #[test]
    fn bad_file_size() {
        let data = vec![0u8; 1000];
        let err = parse_map_bytes(&data, Path::new("bad.MAP")).unwrap_err();
        assert!(matches!(err, MapError::BadFileSize(1000)));
    }

    #[test]
    fn metadata_captured() {
        let data = make_test_map();
        let map = parse_map_bytes(&data, Path::new("test.MAP")).unwrap();

        assert_eq!(map.metadata.len(), METADATA_SIZE);
    }

    #[test]
    fn parse_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.MAP");
        let data = make_test_map();
        std::fs::write(&path, &data).unwrap();

        let map = parse_map(&path).unwrap();
        assert_eq!(map.header.width, 200);
        assert_eq!(map.asset_refs.tileset_path, r"C:\WOW\SPR\SCEN1\TILSCN01.TIL");
    }
}
