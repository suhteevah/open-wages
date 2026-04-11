# Palette Analysis

## Summary

The OFFPIC2.PCX / SCENPIC.PCX palette is the **correct and only** master palette for the engine. The tiles are dark because SCEN1 is a jungle scenario that genuinely uses dark green/brown palette indices. There is no palette bug.

## Sources Investigated

### PCX Files (Primary Palette Source)
- **OFFPIC2.PCX** and **SCENPIC.PCX** have 100% identical palettes (768/768 bytes match).
- The palette is **8-bit** (max value 255), NOT 6-bit VGA. No `*4` scaling needed.
- Most other game-screen PCX files (MAINPIC, menus, catalog screens) differ by only 2-27 bytes from OFFPIC2, confined to indices 247-254 (UI overlay colors). The shared "system palette" covering indices 0-246 and 255 is consistent across all non-cutscene PCX files.
- Cutscene PCX files (CUT*.PCX, SUC*.PCX, OFFICE.PCX) have completely different palettes (700+ byte diffs) -- these are standalone fullscreen images with their own color sets.

### WOW.PAL
- 9 bytes total: `rem ...\r\n` -- a no-op placeholder, not a real palette file.
- The game's palette loading code falls through from wow.pal to PCX extraction.

### WINGPAL.IMS
- Located at `data/extracted/Group10/WINGPAL.IMS`.
- 5024-byte MZ DOS executable (WinG DLL stub).
- Does NOT contain embedded palette data matching the game palette (best match: 10/256 entries).
- Used only for WinGSetDIBColorTable API calls -- it applies palettes, it doesn't store them.

### TIL / OBJ / SPR Files
- Use the custom sprite container format (32-byte file header, 8-byte offset table entries, 24-byte per-sprite headers, RLE-compressed pixel data).
- **No embedded palette data** of any kind. No FLIC chunk signatures (type 4 COLOR_256, type 11 COLOR_64) found.
- All sprite containers are palette-indexed; they depend entirely on the externally-loaded master palette.

### FLC/FLI/FLIC Files
- None present in the game data. The game does not use Autodesk FLIC animation format.

## Tile Brightness Analysis

### SCEN1 (Jungle) -- The "Dark Tiles" Scenario
- 512 tiles, each 128x63 pixels.
- Uses only 73 unique palette indices out of 256.
- **Average pixel brightness: 17.1/255** -- intentionally very dark.
- Top indices by usage:
  - `[219]` (56,24,4) dark brown -- 13.2% of pixels
  - `[78]` (0,32,0) deep green -- 12.7%
  - `[76]` (4,60,8) dark green -- 10.9%
  - `[30]` (16,16,16) near-black gray -- 10.9%
  - `[1]` (1,1,1) near-black -- 9.6%
  - `[79]` (0,16,0) very dark green -- 8.3%
- The tiles exclusively use the **dark end** of each color ramp:
  - Green ramp [70-79]: only indices 72-79 used (the darker half)
  - Brown ramp [170-175]: indices 172-175 dominate
  - Earth ramp [215-223]: indices 217-221 dominate

### Comparison With Other Scenarios
| Scenario | Theme | Avg Brightness |
|----------|-------|---------------|
| SCEN1 | Jungle | 17/255 |
| SCEN3 | Desert/Sand | 135/255 |
| SCEN7 | Arid terrain | 103/255 |

This confirms SCEN1 is simply the darkest scenario -- its tiles are intentionally low-brightness jungle terrain.

## Palette Structure

The 256-color palette is organized into color ramps:
- **[0]** Black (transparent key for sprites)
- **[1]** Near-black (1,1,1) -- used as ground shadow
- **[2]** Near-white (254,254,254)
- **[3-15]** System/UI colors (red, green, purple, etc.)
- **[16-31]** Gray ramp (bright to dark: 240 down to 4)
- **[32-47]** Pastel/UI accent range
- **[48-63]** Yellow/gold range
- **[64-79]** Green ramp (bright forest green to near-black)
- **[80-95]** Teal/cyan-green ramps
- **[96-111]** Olive/yellow-green ramps
- **[112-127]** Blue ranges
- **[128-143]** Cyan/ice ranges
- **[144-159]** Purple/mauve ranges
- **[160-175]** Warm brown ramp (bright to dark)
- **[176-191]** Skin/flesh and dark red ramps
- **[192-207]** Stone gray and tan/sand ramps
- **[208-223]** Dark earth/soil ramp
- **[224-246]** Additional accent colors
- **[247-254]** Vary per PCX file (UI overlay / screen-specific)
- **[255]** White (255,255,255)

## Conclusion

1. **The palette is correct.** OFFPIC2.PCX/SCENPIC.PCX provides the proper 8-bit master palette.
2. **No embedded palettes** exist in TIL/OBJ/SPR files. They are pure palette-indexed data.
3. **SCEN1 tiles are intentionally dark** -- it's a deep jungle scenario. Other scenarios (desert, arid) render much brighter with the same palette.
4. **No FLC palette chunks** -- the game uses its own sprite format, not FLIC.
5. If tiles appear "too dark" in the viewer, the issue is NOT the palette -- it's either (a) the specific scenario chosen for testing, (b) monitor gamma/brightness, or (c) a rendering pipeline issue (e.g., incorrect pixel format causing channel swizzling, which would shift colors but not brightness).
