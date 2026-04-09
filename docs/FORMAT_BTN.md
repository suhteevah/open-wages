# FORMAT: *.BTN (Button Layout Definitions)

## Overview

`.BTN` files are plaintext, line-oriented data files that define UI button layouts for *Wages of War*. Each file describes a set of clickable regions for a specific game screen (main combat HUD, armaments exchange, man-to-man combat, full map view, etc.).

Buttons reference sprite rectangles from a companion sprite sheet for their visual states (normal, hover, pressed, disabled). Uses Windows-style line endings (CR/LF).

## File Structure

```
[NrButtons]
<button_count>
[Button]
<field_1>
<field_2>
<field_3>
<button_id>
<hit_rect>
<sprite_normal>
<sprite_pressed>
<sprite_hover>
<sprite_disabled>
<param_1>
<param_2>
<param_3>
<param_4>
...                   # repeat [Button] blocks
[End]
```

## Header

| Line | Content | Description |
|------|---------|-------------|
| 1 | `[NrButtons]` | Literal section marker |
| 2 | Integer | Total number of button definitions to follow |

## Button Block

Each button is a `[Button]` marker followed by exactly 13 data lines:

| Line | Field | Type | Description |
|------|-------|------|-------------|
| 1 | `[Button]` | Marker | Literal section marker |
| 2 | `field_1` | Integer | Always `0` in observed data. Possibly button type or parent group. |
| 3 | `field_2` | Integer | Always `0` in observed data. Possibly layer/z-order. |
| 4 | `field_3` | Integer | Button page/tab group. `0` or `1` observed. Buttons with `1` appear to belong to an alternate toolbar state (e.g., different weapon action set). |
| 5 | `button_id` | Integer | 1-based button identifier. Unique within the file. Referenced by game logic to determine which action was clicked. |
| 6 | `hit_rect` | `x1,y1,x2,y2` | Screen-space clickable rectangle. `(x1,y1)` = top-left, `(x2,y2)` = bottom-right. Coordinates are in the game's native resolution (640x480). `0,0,0,0` = invisible/disabled button. |
| 7 | `sprite_normal` | `x1,y1,x2,y2` | Source rectangle in the button sprite sheet for the **normal/up** state. `0,0,0,0` = no sprite (text-only or invisible region). |
| 8 | `sprite_pressed` | `x1,y1,x2,y2` | Source rectangle for the **pressed/down** state. `0,0,0,0` = no visual change on press. |
| 9 | `sprite_hover` | `x1,y1,x2,y2` | Source rectangle for the **hover/highlight** state. `0,0,0,0` = no hover effect. |
| 10 | `sprite_disabled` | `x1,y1,x2,y2` | Source rectangle for the **disabled/grayed** state. `0,0,0,0` = not applicable. |
| 11 | `param_1` | Integer | Always `0` in observed data. |
| 12 | `param_2` | Integer | Always `0` in observed data. |
| 13 | `param_3` | Integer | Always `0` in observed data. |
| 14 | `param_4` | Integer | Always `0` in observed data. |

## Terminator

The file ends with `[End]` on its own line, optionally followed by a blank line.

## Rectangle Format

All rectangles use the format `x1,y1,x2,y2` (no spaces around commas):
- `(x1, y1)` = top-left corner
- `(x2, y2)` = bottom-right corner
- Coordinates are pixel positions in 640x480 resolution (for hit rects) or within the button sprite sheet (for sprite source rects)
- `0,0,0,0` = null/empty rectangle

Some lines have trailing whitespace after the rectangle values (observed in ARMEXC.BTN, MANTOMAN.BTN). Parsers should trim trailing whitespace.

## Button Pages (field_3)

In `MAIN.BTN`, buttons are split into two groups by `field_3`:
- **Page 0** (buttons 1-23, 35-40): Default toolbar buttons -- movement, combat actions, stance changes, map view, info panel, etc.
- **Page 1** (buttons 24-34): Alternate toolbar -- likely weapon-specific actions that swap in when a weapon is selected or a specific mode is active.

The engine shows one page at a time and swaps between them based on game state.

## Sprite States

Buttons can have up to four visual states, each referencing a different rectangle in the button sprite sheet:

| State | Typical Usage |
|-------|--------------|
| Normal | Default appearance when button is idle |
| Pressed | Shown while mouse button is held down on the button |
| Hover | Shown when mouse cursor is over the button |
| Disabled | Shown when the button's action is unavailable |

When all four sprite rects are `0,0,0,0`, the button is an invisible clickable region -- used for text list items, the main viewport click area, portrait panels, etc.

When `sprite_hover` and `sprite_disabled` are `0,0,0,0` but normal/pressed are set, the button only has two visual states (common for simple icon buttons in ARMEXC and MANTOMAN).

## Known Button Files

| File | Screen | Buttons | Notes |
|------|--------|---------|-------|
| `MAIN.BTN` | Combat HUD | 40 | Full combat interface with two button pages. Includes viewport (button 35, 0-636x320), info panel (36), action buttons (7-34), scroll arrows (37). |
| `MAIN2.BTN` | Alternate combat HUD | 11 | Simplified variant, possibly for non-combat movement or cinematic mode. |
| `MAIN3.BTN` | Third combat variant | -- | Another HUD layout variant. |
| `ARMEXC.BTN` | Armaments Exchange | 13 | Equipment management screen. Includes scroll arrows (1-6), OK/Cancel buttons (7-8), close (9), scroll handles (10-11), and navigation (12-13). |
| `MANTOMAN.BTN` | Man-to-Man screen | 17 | Individual merc management. Similar scroll/list UI. Two columns of scroll arrows (1-6 left, 9-14 right), with action buttons (7-8, 15-16) and close (17). |
| `FULLMAP.BTN` | Full Map View | 1 | Single full-screen clickable region (0,0,639,479). Click-to-dismiss overlay. |

## Example: FULLMAP.BTN (Simplest Case)

```
[NrButtons]
1
[Button]
0
0
0
1
0,0,639,479          <-- full screen hit area
0,0,0,0              <-- no sprite (transparent overlay)
0,0,0,0
0,0,0,0
0,0,0,0
0
0
0
0
[End]
```

## Example: ARMEXC.BTN Button 7 (OK Button)

```
[Button]
0
0
0
7
344,432,414,455       <-- screen position: 70x23 pixel hit area
432,25,502,48         <-- normal state: 70x23 from sprite sheet
504,25,574,48         <-- pressed state
0,0,0,0               <-- no hover sprite
0,0,0,0               <-- no disabled sprite
0
0
0
0
```

## Example: MAIN.BTN Button 16 (Icon Button with All States)

```
[Button]
0
0
0
16
429,328,452,350       <-- 23x22 hit area
79,1,102,24           <-- normal
131,1,154,24          <-- pressed
105,1,128,24          <-- hover
105,1,128,24          <-- disabled (same as hover)
0
0
0
0
```

Note: When `sprite_hover` equals `sprite_disabled`, the button uses the same graphic for both states (hover highlight doubles as disabled indicator, or the distinction is not needed).

## Parser Notes

- All values are integers or comma-separated integer tuples.
- Strip `\r` (CR) before parsing.
- Trim trailing whitespace from lines (some files have trailing spaces).
- The button count in `[NrButtons]` must match the number of `[Button]` blocks.
- Button IDs are 1-based and sequential within a file.
- The `[End]` terminator uses title-case (capital E), unlike `[END]` in .COR files.
- Resolution assumption: 640x480 (standard VGA, the game's native resolution). Hit rects near (639, 479) confirm this.
- Sprite sheet coordinates reference a separate bitmap file (not specified within the .BTN file itself -- determined by the game code for each screen).
