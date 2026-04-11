# Tile Cell Format — Verified Against On-Disk MAP Data

**Date:** 2026-04-09
**Source:** RE analysis of `Wow.exe` `PackCellWord1` at `0x41AF7B` + empirical validation against `SCEN1A.MAP`

## Summary

The MAP file tile grid stores each cell as a **single little-endian u32** containing three 9-bit tile layer indices and a 5-bit flag field. The previous parser incorrectly read each cell as two separate u16 values (`cell_flags` + `tile_index`), producing wrong tile indices.

## Confirmed Bit Layout (Cell Word 1)

```
Bit 31                                                     Bit 0
┌─────────────┬─────────────┬─────────────┬─────┐
│  layer0 (9) │  layer1 (9) │  layer2 (9) │fl(5)│
│  [31..23]   │  [22..14]   │  [13..5]    │[4.0]│
└─────────────┴─────────────┴─────────────┴─────┘
```

| Field | Bits | Width | Range | Purpose |
|-------|------|-------|-------|---------|
| `layer0` | 31..23 | 9 | 0-511 | Primary terrain sprite index |
| `layer1` | 22..14 | 9 | 0-511 | Secondary terrain overlay |
| `layer2` | 13..5 | 9 | 0-511 | Tertiary terrain detail |
| `flag_A` | 4 | 1 | 0-1 | Wall/obstacle |
| `flag_B` | 3 | 1 | 0-1 | Explored |
| `flag_C` | 2 | 1 | 0-1 | Roof |
| `flag_D` | 1 | 1 | 0-1 | Walkable |
| padding | 0 | 1 | 0 | Unused |

## Empirical Validation

Tested against `SCEN1/SCEN1A.MAP` (248,384 bytes, 200x252 grid).

### Sample cells (u32 LE interpretation)

| Cell | Raw u32 | L0 | L1 | L2 | Flags |
|------|---------|----|----|-----|-------|
| (0,0) | `0x01000000` | 2 | 0 | 0 | 0x00 |
| (1,0) | `0x07800000` | 15 | 0 | 0 | 0x00 |
| (11,5) | `0x07FEFF7C` | 15 | 507 | 507 | 0x1C |
| (90,6) | `0x13FDFEE0` | 39 | 503 | 503 | 0x00 |
| (91,6) | `0x130000E8` | 38 | 0 | 7 | 0x08 |

### What was wrong before

The old parser read each cell as two u16 LE values and extracted `tile_word & 0x7FFF` as the tile index. For the common cell value `0x07800000`:

- **Old result:** `tile_word = 0x0780`, `tile_index = 1920`, renderer applied `& 0x1FF` = **384** (wrong)
- **Correct result:** `layer0 = (0x07800000 >> 23) & 0x1FF` = **15** (correct primary terrain tile)

### Layer usage statistics (SCEN1A.MAP active area)

- Total active cells: 40,400
- Cells with layer1 or layer2 non-zero: **3,014** (7.5%)
- Layer1/layer2 values cluster around 503-507, suggesting overlay/detail sprite indices in the upper range of the TIL sheet

## Border Cell Detection

Border cells (rows 202-251) are identified by byte 1 of the on-disk 4-byte cell equaling `0xFF`. In the u32 LE representation, this is `(raw >> 8) & 0xFF == 0xFF`.

## On-Disk vs In-Memory Layout

The RE analysis documents **five parallel 32-bit arrays** in the engine's memory (at `0x59D8C0` through `0x5E42C0`). The on-disk MAP file stores only Cell Word 1 in the tile grid region (offset `0x00000`, 201,600 bytes). The remaining cell words (terrain type, elevation, objects) are stored in the metadata footer region starting at offset `0x31610`.

## Files Modified

- `crates/ow-data/src/map_loader.rs` — `MapTile` struct updated with `layer0/1/2` + `flags`; parser reads u32 LE
- `crates/ow-render/src/tile_renderer.rs` — Uses `tile.layer0` directly instead of `tile.tile_index & 0x1FF`
- `crates/ow-app/src/game_loop.rs` — Minimap color lookup uses `tile.layer0`
