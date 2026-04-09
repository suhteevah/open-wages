# Binary File Format Survey

Initial triage of all binary file formats in the Wages of War game data.
Generated 2026-04-09 via `ow-tools triage`.

---

## File Inventory Summary

| Format | Extension | Count | Size Range | Location |
|--------|-----------|-------|------------|----------|
| Map data | `.MAP` | 52 | 248,384 (all identical) | `MAPS/SCENxx/` |
| Tile sprites | `.TIL` | 16 | 241 KB -- 1.7 MB | `SPR/SCENxx/` |
| Object sprites | `.OBJ` | 67 | 3 KB -- 1.4 MB | `SPR/` and `SPR/SCENxx/` |
| Sprite sheets | `.SPR` | 3 | 29 KB -- 1.2 MB | `SPR/` |
| Tile metadata | `TILESxx.DAT` | 16 | ~20 KB | `SPR/SCENxx/` |
| Object metadata | `OBJxx.DAT` | 16 | ~25 KB | `SPR/SCENxx/` |
| Animation sprites | `ANIM/*.DAT` | 34 | 2 KB -- 15.5 MB | `ANIM/` |
| Animation defs | `.COR` | 33 | (text, paired with .DAT) | `ANIM/` |
| PCX images | `.PCX` | 133 | 11 KB -- 319 KB | `PIC/` |
| Voice+lip audio | `.VLA` | 44 | 80 B -- 671 KB | `WAV/` |
| Voice+subtitle | `.VLS` | 68 | varies | `WAV/` |
| Sound effects | `.WAV` | 7+ | standard WAV | `SND/`, `WAV/` |
| MIDI music | `.MID` | 18 | standard MIDI | `MIDI/` |
| Video | `.AVI` | 10 | standard AVI | `AVI/` |
| Palette | `.PAL` | 1 | 9 bytes (text stub) | root |
| Button defs | `.BTN` | 6 | (likely text) | `BUTTONS/` |
| Cursor files | `.CUR` | 3 | Windows cursor format | `CURSORS/` |

---

## 1. Sprite/Object Format (.OBJ, .SPR, .TIL, ANIM .DAT)

**These all share a common sprite container format.** This is the single most important format to crack.

### Common Header Structure (all sprite containers)

```
Offset  Size  Field
0x00    u32   sprite_count        -- number of frames/sprites in this file
0x04    u32   header_size (0x20)  -- always 32; offset to start of offset table
0x08    u32   offset_table_start  -- = sprite_count * 8 + header_size (each entry is 8 bytes?)
0x0C    u32   offset_table_end    -- = offset_table_start + 0x20 (padding?)
0x10    u32   total_pixel_data_size
0x14    12B   varies (zero for most; ANIM .DAT has extra fields here)
0x20    ...   per-sprite offset table (sprite_count entries)
...     ...   per-sprite dimension/hotspot table
...     ...   pixel data (RLE-compressed, 8-bit indexed)
```

**Evidence for shared format:**
- All files have `0x20` at offset 0x04 (header size = 32 bytes)
- First u32 matches expected sprite counts:
  - `SCENSPR.OBJ`: 0x10 = 16 sprites
  - `MENUSPR.OBJ`: 0x61 = 97 sprites  
  - `CURSORS.SPR`: 0x61 = 97 sprites
  - `ACCOUNT.OBJ`: 0x03 = 3 sprites
  - `TILSCN01.TIL`: 0x0200 = 512 tiles
  - `JUNGSLD.DAT`: 0x28E8 = 10,472 frames
  - `SCEN1.OBJ` (scene): 0x0200 = 512 objects

**Pixel data is RLE-encoded 8-bit palette indices.** The high entropy (4.6--6.8) and dominance of values in the 0x60--0x7F range (ASCII lowercase letters like `m`, `n`, `o`, `l`, `k`) strongly suggest palette-indexed pixel data with a palette centered around earth tones.

### Sub-variants

| Variant | sprite_count range | Notes |
|---------|-------------------|-------|
| `.OBJ` (UI) | 3--105 | Menu/UI sprite sheets (MENUSPR, ACCOUNT, CATALOG, etc.) |
| `.OBJ` (scene) | 512 | Per-scene map objects, paired with OBJxx.DAT metadata |
| `.SPR` | 74--105 | Same as .OBJ but different extension (CURSORS, INVEN, WOMAN) |
| `.TIL` | 512 | Isometric tile graphics, paired with TILESxx.DAT metadata |
| `ANIM/*.DAT` | 536--10,472 | Character animation frames (walk/shoot/die in 8 directions) |

### ANIM .DAT Extra Header Fields

ANIM .DAT files have additional data at offsets 0x14--0x1F that are zero in other sprite files:
```
0x14    u32   unknown (e.g. 0x052820 for JUNGSLD = 337,952)
0x18    u16   unknown (0x3A = 58 for JUNGSLD)
0x1A    u16   unknown (0x1E = 30)
0x1C    u16   unknown (0x01 = 1)
0x1E    u16   unknown
```
These likely encode animation metadata (frame dimensions, direction count, base frame size).

### Size Ranges

- **ANIM .DAT**: 2 KB (CANSTR) to 15.5 MB (LUMPY) -- massive; soldier animations are ~15 MB each
- **.TIL**: 241 KB (SCEN9) to 1.7 MB (SCEN10) -- one per scenario
- **.OBJ**: 3 KB (FAXSPR) to 1.4 MB (CATALOG, SCEN1.OBJ)
- **.SPR**: 29 KB (CURSORS) to 1.2 MB (INVEN)

### Entropy Profile

| File | Entropy | ASCII% | Interpretation |
|------|---------|--------|----------------|
| SCENSPR.OBJ | 2.70 | 34% | Small, few unique colors |
| MENUSPR.OBJ | 6.42 | 14% | Complex UI graphics |
| CURSORS.SPR | 3.93 | 15% | Simple cursor graphics |
| INVEN.SPR | 4.93 | 27% | Moderate complexity |
| WOMAN.SPR | 6.78 | 24% | High detail character art |
| TILSCN01.TIL | 5.34 | 20% | Terrain tiles |
| JUNGSLD.DAT | 4.64 | 18% | Animated character (many similar frames) |
| MISC.DAT | 6.69 | 15% | Mixed effects/objects |

---

## 2. Map Format (.MAP)

**All MAP files are exactly 248,384 bytes.** Fixed-size format.

### Structure

```
Offset      Size       Content
0x00000     201,600B   Tile grid data (big-endian u32 per cell)
0x31380     ~46,784B   String table + metadata footer
```

### Tile Grid

- The grid starts with a 4-byte value per cell
- First cell: `00 00 00 01` (big-endian, note: rest of game is LE)
- Repeating pattern `00 00 80 07` fills most of the grid (= default/empty tile?)
- 201,600 / 4 = 50,400 cells; if square: ~224x224 grid; more likely a non-square layout
- Possible: 200x252 or similar rectangular grid

### String Table (offset 0x31380)

Contains original build paths that reference associated files:
```
C:\WOW\SPR\SCEN1\TILSCN01.TIL    -- tile sprite sheet
C:\WOW\SPR\SCEN1\TILES1.DAT      -- tile metadata
C:\WOW\SPR\SCEN1\SCEN1.OBJ       -- object sprite sheet
C:\WOW\SPR\SCEN1\OBJ01.DAT       -- object metadata
```
Each path is stored in a ~164-byte fixed-width field (null-padded).

### Variants

Each scenario directory contains 2--4 MAP files:
- `SCENxxA.MAP` -- primary map (the "active" version)
- `SCENxxA0.MAP` -- backup/original of primary
- `SCENxxB.MAP`, `SCENxxC.MAP` -- alternate map variants (some scenarios only)
- `SCENxx.MAP` -- base template (some scenarios)

---

## 3. Tile/Object Metadata (.DAT in SPR/SCENxx/)

### TILES*.DAT -- Tile Properties

- **Structure**: 512 records x 40 bytes (confidence 99%)
- **Size**: ~20 KB
- **Entropy**: 2.27 (very structured, few unique values)
- **Content**: Tile properties indexed by tile ID. Likely contains:
  - Movement cost / terrain type
  - Elevation
  - Cover value
  - Line-of-sight blocking flags
  - Walk/drive passability

### OBJ*.DAT -- Object Properties

- **Structure**: 512 records x 48 bytes (confidence 100%)
- **Size**: ~25 KB  
- **Entropy**: 1.75 (extremely structured)
- **Content**: Object properties indexed by object ID. Likely contains:
  - Object type (destructible, cover, decoration)
  - Collision bounds
  - Health/destructibility
  - Cover direction flags

---

## 4. PCX Images (.PCX)

Standard ZSoft PCX format (version 5, 8-bit, 256-color palette).

### Header Fingerprint
```
0x00: 0A       -- PCX magic
0x01: 05       -- version 5 (256-color with palette)
0x02: 01       -- RLE encoding
0x03: 08       -- 8 bits per pixel
0x04: 00 00    -- xmin
0x06: 00 00    -- ymin
0x08: 7F 02    -- xmax = 639
0x0A: DF 01    -- ymax = 479
```

**Resolution**: 640x480 (confirmed from xmax/ymax for BUTTONS.PCX, MAINPIC.PCX, WORLDMAP.PCX).

### Palette

The 256-color VGA palette is embedded in the last 769 bytes of each PCX file (byte 0x0C marker + 768 bytes of RGB triplets). This is standard PCX behavior.

### Usage Categories

| Pattern | Count | Purpose |
|---------|-------|---------|
| `CUTxx.PCX` | 16 | Cutscene images |
| `SUCxx.PCX` | 16 | Mission success screens |
| `*PIC.PCX` | ~20 | UI background screens |
| `*MSK.PCX` | ~15 | UI masks (transparency) |
| `*MAS*.PCX` | ~8 | Additional masks |
| `MENU0x.PCX` | 4 | Main menu backgrounds |
| `NTRFCxx.PCX` | 14 | NPC interface portraits |
| `FAX*.PCX` | 5 | Fax machine UI |
| `WEP*.PCX` | 5 | Weapon shop screens |

**Size range**: 11 KB (masks) to 319 KB (full backgrounds).
**Entropy**: 2.3--5.0 (RLE-compressed; masks are low-entropy, photos are higher).

---

## 5. Audio: VLA/VLS (Voice + Lip/Subtitle)

### Magic Bytes

All VLA and VLS files begin with the signature:
```
56 41 4C 53 -- "VALS" (4 bytes)
```

Some files show `VALSP` (5 bytes with trailing 'P') -- may indicate variant.

### Internal Structure

```
Offset    Content
0x00      "VALS" magic (4 bytes)
0x04      u32 -- header/index size
0x08      u32 -- unknown (sometimes 0, sometimes 0xFFFFFFFF)
0x0C      u32 -- entry count or flags
...       index/metadata section
+var      "WRDS" marker -- word/subtitle timing data
+var      "WAVE" marker
+var      "RIFF" marker -- embedded standard WAV audio
+var      "WAVEfmt " -- standard WAV format chunk
+var      "data" -- raw PCM audio data
```

### Key Observations

- **VLA = Voice + Lip Animation**: Contains WAV audio with lip-sync timing data
- **VLS = Voice + Lip + Subtitles**: Identical format; some VLA/VLS pairs are byte-identical
- The embedded audio is standard PCM WAV (RIFF container)
- "WRDS" section likely contains word timing for subtitle display and lip-sync
- Entropy ~5.2--6.1 (dominated by 8-bit unsigned PCM audio samples)

### File Categories

| Pattern | Count (VLA+VLS) | Purpose |
|---------|-----------------|---------|
| `MISHNxxY` | 70+ | Mission briefing voice lines (A/B/C variants) |
| `ARTIExx` | 12 | Character "Artie" dialogue |
| `VINNIExx` | 3 | Character "Vinnie" dialogue |
| `MOMxx` | 2 | Character "Mom" dialogue |
| `PIZZA*` | 2 | Pizza shop dialogue |
| `ACCT` | 1 | Accountant dialogue |
| `SHARK` | 1 | Loan shark dialogue |
| `WOMAN` | 1 | Woman character dialogue |

---

## 6. Palette (.PAL)

**WOW.PAL is NOT a binary palette file.** It is a 9-byte text file:
```
rem ...
```
This is just a comment/placeholder. The actual palette is either:
1. Embedded in each PCX file (confirmed -- standard PCX palette at EOF)
2. Embedded in each sprite container (the sprite format likely references a shared palette)
3. Hardcoded in the original executable

The sprite format almost certainly uses a shared 256-color VGA palette. Since PCX files embed their own palettes, extracting the palette from any full-screen PCX (e.g., `MAINPIC.PCX`) will give us the game's master palette.

---

## 7. Other Binary Formats

### Button Definitions (.BTN)
- Location: `BUTTONS/`
- 6 files: ARMEXC, FULLMAP, MAIN, MAIN2, MAIN3, MANTOMAN
- Likely text/structured data defining UI button positions and actions

### Cursor Files (.CUR)
- Location: `CURSORS/`
- Standard Windows .CUR format (FEET1, FEET2, FIRE, TRAN_CUR)
- Can be loaded with standard Windows cursor APIs

### Sound Effects (.WAV)
- Location: `SND/` and `WAV/`
- Standard PCM WAV format
- 7+ files in SND (PISTOL, RIFLE, SHOTGUN, etc.)

### MIDI Music (.MID)
- Location: `MIDI/`
- 18 files, standard MIDI format
- Named by context: WOWMIS01-09 (mission music), WOWOFICE (office), WOWARIVE (arrival), etc.

### Video (.AVI)
- Location: `AVI/`
- 10 files, standard AVI format
- Mix of 320-px (low-res) and full versions of logos, opening, credits, ending

---

## RE Priority Ranking

### Priority 1 -- CRITICAL PATH (blocks all rendering)

1. **Sprite Container Format** (.OBJ/.SPR/.TIL/ANIM .DAT)
   - Single shared format; cracking this unlocks ALL visual data
   - Header is already partially decoded (sprite count, offset table structure)
   - Need to decode: offset table entries, per-sprite headers (width/height/hotspot), RLE pixel encoding
   - Start with `SCENSPR.OBJ` (16 sprites, small, simple) or `CURSORS.SPR` (known cursor shapes)

2. **Palette Extraction**
   - Extract 256-color palette from a PCX file (trivial -- standard format)
   - Verify it matches what sprites expect by rendering decoded sprite data

### Priority 2 -- REQUIRED FOR MAP RENDERING

3. **MAP Format**
   - Fixed 248,384 bytes; grid structure is regular
   - Decode tile grid (cell format: tile_id + flags?)
   - Parse string table to resolve associated asset paths
   - Need to figure out grid dimensions and cell byte layout

4. **TILES*.DAT / OBJ*.DAT Metadata**
   - 512 x 40B and 512 x 48B fixed struct arrays
   - Map tile IDs and object IDs to properties
   - Decode field meanings via cross-reference with MAP data

### Priority 3 -- REQUIRED FOR ANIMATION

5. **ANIM .COR Files** (text format -- already partially understood)
   - Define animation sequences: which frames from the .DAT for each action/direction
   - Paired 1:1 with ANIM .DAT sprite files

6. **ANIM .DAT Extra Header Fields**
   - The extra bytes at 0x14--0x1F in animation sprite files
   - Likely encode direction count, action count, frames-per-action

### Priority 4 -- AUDIO/NARRATIVE

7. **VLA/VLS Voice Format**
   - "VALS" container with embedded WAV + "WRDS" subtitle timing
   - WAV extraction is straightforward (find RIFF header, read to end)
   - WRDS timing format needs RE for subtitle display

### Priority 5 -- STANDARD FORMATS (minimal RE needed)

8. **PCX Images** -- standard format, use any PCX library
9. **WAV Sound Effects** -- standard format
10. **MIDI Music** -- standard format
11. **AVI Video** -- standard format
12. **CUR Cursors** -- standard Windows format

---

## Recommended First Steps

1. **Extract a PCX palette** from `MAINPIC.PCX` -- gives us the color table immediately
2. **Decode `SCENSPR.OBJ`** (16 sprites, 9.7 KB) -- smallest non-trivial sprite file
   - Parse offset table at 0x20
   - Identify per-sprite width/height/hotspot fields
   - Decode RLE pixel data and render with extracted palette
3. **Decode `CURSORS.SPR`** (97 sprites, 29 KB) -- we know what cursors should look like, making visual verification easy
4. **Once sprites work**: decode `TILSCN01.TIL` (512 tiles) and `SCEN1A.MAP` to render a map
