# Sprite Format Verification

**Date:** 2026-04-09
**Purpose:** Verify whether .OBJ/.SPR/.TIL/ANIM .DAT files use Autodesk FLC/FLI format as claimed in the RE analysis.

## Verdict: NOT FLIC Format

The sprite files use a **custom container format**, not Autodesk FLC/FLI. The existing `sprite.rs` parser is correct and should not be rewritten.

## Evidence

### 1. No FLIC Magic Bytes

FLIC files have magic `0xAF11` (FLI) or `0xAF12` (FLC) at offset 4-5, and frame chunks use `0xF1FA` at offset 4 of each frame header.

Actual bytes at offset 4-5 across all sprite files:

| File | Bytes 4-5 | Expected (FLIC) |
|------|-----------|-----------------|
| MENUSPR.OBJ | `20 00` | `11 AF` or `12 AF` |
| CURSORS.SPR | `20 00` | `11 AF` or `12 AF` |
| TILSCN01.TIL | `20 00` | `11 AF` or `12 AF` |
| GUARDDOG.DAT | `20 00` | `11 AF` or `12 AF` |

The value `0x00000020` (32) at offset 4 is the `header_size` field of the custom format, not a FLIC magic number. No `0xF1FA`, `0xAF11`, or `0xAF12` bytes appear anywhere in the first 256 bytes of any sprite file.

### 2. Header Layout Does Not Match FLIC

FLIC 128-byte header:
```
Offset  Size  Field
0x00    4     File size
0x04    2     Magic (0xAF11 or 0xAF12)
0x06    2     Frame count
0x08    2     Width
0x0A    2     Height
...
```

Actual custom format 32-byte header:
```
Offset  Size  Field
0x00    4     Sprite count (varies: 3, 97, 512, 536, etc.)
0x04    4     Header size (always 0x20 = 32)
0x08    4     Offset table size (sprite_count * 8)
0x0C    4     Pixel data start (header_size + offset_table_size)
0x10    4     Pixel data size
0x14    12    Reserved (zeroes in OBJ/SPR/TIL; non-zero in some ANIM)
```

### 3. Custom Format Confirmed Across 103/104 Files

All 104 sprite-type files were checked:

- **103 files** have `header_size = 0x20` at offset 4, matching the custom container format
- **1 file** (`RIFLWALK.DAT`) has a different structure — no standard header, no .COR companion; likely raw frame data or a different sub-format
- **0 files** have any FLIC magic bytes

File type breakdown:
- `.OBJ` files: All match custom format
- `.SPR` files: All match custom format
- `.TIL` files: All match custom format (16 files across scenarios)
- `.DAT` (ANIM): All match except RIFLWALK.DAT

### 4. Representative File Headers

```
MENUSPR.OBJ (296,537 bytes):
  sprite_count = 0x61 (97)
  header_size  = 0x20 (32)
  offset_table = 0x308 (776 = 97*8)
  pixel_start  = 0x328 (808 = 32+776)
  pixel_size   = 0x48331 (295,729)

TILSCN01.TIL (1,291,613 bytes):
  sprite_count = 0x200 (512)
  header_size  = 0x20 (32)
  offset_table = 0x1000 (4096 = 512*8)
  pixel_start  = 0x1020 (4128 = 32+4096)
  pixel_size   = 0x13A53D (1,287,485)

GUARDDOG.DAT (692,888 bytes):
  sprite_count = 0x218 (536)
  header_size  = 0x20 (32)
  offset_table = 0x10C0 (4288 = 536*8)
  pixel_start  = 0x10E0 (4320 = 32+4288)
  pixel_size   = 0xA81B8 (688,568)
```

### 5. RLE Compression Is NOT ByteRun1 (PackBits)

The RE analysis claims the compression is ByteRun1 (PackBits), which is standard FLIC. However, the actual RLE scheme in the files (which our parser handles correctly) differs:

| Feature | ByteRun1 (FLIC) | Actual Format |
|---------|-----------------|---------------|
| Control byte > 128 | Run of (256 - N + 1) | `0x80` = transparent skip; `0x81-0xFF` = literal copy of (N - 0x80) bytes |
| Control byte < 128 | Literal copy of (N + 1) | RLE run: repeat next byte N times |
| Control byte == 128 | NOP | Transparent skip (paired with count byte) |
| End of scanline | Implicit (width-based) | Explicit `0x00` marker |
| Transparency | Not native | `0x80 NN` skip command + index 0 |

The sense of the control byte is **inverted** compared to ByteRun1, and the format has an explicit end-of-scanline marker (`0x00`) that ByteRun1 lacks.

## Why the RE Analysis Was Wrong

The RE analysis found:
1. `0xF1FA` magic check at code address `0x4132C3` — this is likely a FLC decoder **also present in the executable** for loading `.FLC` cutscene/animation files (common in mid-90s games), not for sprite files
2. `BYTE_RUN`, `DELTA_FLC`, etc. strings — these are string constants for the FLC animation system, which is a **separate subsystem** from the sprite loader
3. A 132-byte (`0x84`) header read at `0x4131E5` — this is the FLC animation loader, not `LoadObject`/`LoadSprite`

The executable likely contains **two separate graphics systems**:
- A **FLC animation player** (for cutscenes/intros) using standard Autodesk FLIC
- A **custom sprite container** system (for game sprites) using the 32-byte header + offset table + custom RLE format

The RE analysis conflated these two systems.

## Conclusion

The existing `sprite.rs` parser correctly implements the custom sprite container format. No changes needed. The parser successfully handles 103 out of 104 sprite files. The one exception (`RIFLWALK.DAT`) may need separate investigation as a special-case format.

## RIFLWALK.DAT Notes

- No `header_size = 0x20` field
- No `.COR` companion file (unlike all other ANIM .DAT files)
- First bytes: `24 00 00 00 98 03 00 00 06 07 00 00 ...` — possibly a different sub-format with a sequence of raw frame offsets, or a standalone animation file not loaded through the standard `LoadObject` path
