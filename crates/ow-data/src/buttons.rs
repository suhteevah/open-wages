//! Parser for `*.BTN` — UI button layout definition files.
//!
//! # What BTN Files Define
//!
//! Each `.BTN` file defines the clickable UI regions for a specific game screen.
//! For example, `COMBAT.BTN` defines the combat HUD buttons (fire, move, end turn),
//! while `ARMEXCH.BTN` defines the armaments exchange (buy/sell equipment) buttons.
//! The buttons are purely positional — they define WHERE you can click, not what
//! happens when you do (that's handled by the game's event system).
//!
//! Buttons reference sprite rectangles from a companion sprite sheet (`.SPR` file)
//! for their visual states: normal/idle, pressed/down, hover/highlight, and
//! disabled/grayed. A button with all-zero sprite rects is "invisible" — it has
//! a clickable hit area but no visual representation (used for map scrolling
//! regions and full-screen click catchers).
//!
//! # Page/Tab Grouping
//!
//! Field 3 (`page`) groups buttons into pages or tabs. Only one page is shown
//! at a time — switching tabs hides the current page's buttons and shows the
//! new page's. Page 0 buttons are typically always visible (global controls),
//! while pages 1+ are tab-specific (e.g., different weapon categories in a shop).
//!
//! # The 13-Line-Per-Button Format
//!
//! Each `[Button]` block contains exactly 13 data lines:
//! ```text
//! Line 1:  field_1       — Unknown (always 0). Possibly button type or parent.
//! Line 2:  field_2       — Unknown (always 0). Possibly z-order.
//! Line 3:  page          — Page/tab group number.
//! Line 4:  id            — 1-based button identifier, unique within the file.
//! Line 5:  hit_rect      — Clickable area (x1,y1,x2,y2 in 640x480 screen coords).
//! Line 6:  sprite_normal — Source rect for idle state in the sprite sheet.
//! Line 7:  sprite_pressed— Source rect for pressed/down state.
//! Line 8:  sprite_hover  — Source rect for mouse-over state.
//! Line 9:  sprite_disabled— Source rect for grayed-out/disabled state.
//! Line 10: param_1       — Unknown (always 0).
//! Line 11: param_2       — Unknown (always 0).
//! Line 12: param_3       — Unknown (always 0).
//! Line 13: param_4       — Unknown (always 0).
//! ```
//!
//! ## Full File Structure
//!
//! ```text
//! [NrButtons]
//! <count>
//! [Button]
//! <13 data lines per button>
//! ...
//! [End]
//! ```

use std::path::Path;

use tracing::{debug, info, trace, warn};

/// A pixel-space rectangle defined by its top-left and bottom-right corners.
///
/// Coordinates are in the original 640x480 screen space. The renderer must
/// scale these to the actual window size. Note: these are NOT sprite-sheet
/// pixel coordinates for hit_rect — they're screen positions. But for
/// sprite_normal/pressed/hover/disabled, they ARE sprite-sheet source rects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl Rect {
    /// Returns `true` if all coordinates are zero (null/empty rectangle).
    ///
    /// An all-zero rect indicates an "invisible" button with no sprite — used
    /// for click regions that have no visual representation (e.g., the full-screen
    /// click catcher in FULLMAP.BTN that spans 0,0,639,479 with zero sprite rects).
    pub fn is_empty(&self) -> bool {
        self.x1 == 0 && self.y1 == 0 && self.x2 == 0 && self.y2 == 0
    }
}

/// A single button definition from a `.BTN` file.
///
/// The 13 fields map directly to the 13 data lines in each `[Button]` block.
/// Fields marked "always 0 in observed data" are preserved for round-trip
/// fidelity — we don't know if modded data files use them, so we parse them
/// faithfully rather than discarding.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Button {
    /// Unknown field (always 0 in observed data). Possibly button type or parent group.
    /// Preserved for round-trip fidelity with original data.
    pub field_1: u32,
    /// Unknown field (always 0 in observed data). Possibly layer/z-order.
    /// Preserved for round-trip fidelity with original data.
    pub field_2: u32,
    /// Button page/tab group. Buttons with the same page number are shown/hidden
    /// together. Page 0 = always visible, pages 1+ = tab-specific. The game
    /// only renders buttons whose page matches the currently active tab.
    pub page: u32,
    /// 1-based button identifier, unique within the file. The game's event
    /// handler uses this ID to determine what action to take when clicked.
    pub id: u32,
    /// Screen-space clickable rectangle (640x480 coordinates). This is where
    /// the player can click to activate this button. A rect of (0,0,639,479)
    /// means the entire screen is clickable (used for full-screen click catchers).
    pub hit_rect: Rect,
    /// Sprite source rectangle for the normal/idle state. All-zero means the
    /// button is invisible (no visual representation drawn on screen).
    pub sprite_normal: Rect,
    /// Sprite source rectangle for the pressed/down state. Shown while the
    /// mouse button is held down over this button.
    pub sprite_pressed: Rect,
    /// Sprite source rectangle for the hover/highlight state. Shown when the
    /// mouse is over the button but not clicking. All-zero = no hover effect.
    pub sprite_hover: Rect,
    /// Sprite source rectangle for the disabled/grayed state. Shown when the
    /// button is inactive (e.g., "Fire" when no weapon equipped). Often shares
    /// the same rect as sprite_hover when the original designers were lazy.
    pub sprite_disabled: Rect,
    /// Unknown parameter (always 0 in observed data). May be callback/handler index.
    pub param_1: i32,
    /// Unknown parameter (always 0 in observed data).
    pub param_2: i32,
    /// Unknown parameter (always 0 in observed data).
    pub param_3: i32,
    /// Unknown parameter (always 0 in observed data).
    pub param_4: i32,
}

/// A complete parsed button layout file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ButtonLayout {
    /// All buttons defined in the file.
    pub buttons: Vec<Button>,
}

/// Errors that can occur while parsing a `.BTN` file.
#[derive(Debug, thiserror::Error)]
pub enum ButtonError {
    #[error("I/O error reading button file: {0}")]
    Io(#[from] std::io::Error),
    #[error("missing [NrButtons] header")]
    MissingHeader,
    #[error("line {line}: expected button count, got '{text}'")]
    InvalidCount { line: usize, text: String },
    #[error("line {line}: expected [Button] marker, got '{text}'")]
    ExpectedButtonMarker { line: usize, text: String },
    #[error("line {line}: expected integer, got '{text}'")]
    InvalidInteger { line: usize, text: String },
    #[error("line {line}: expected rectangle (x1,y1,x2,y2), got '{text}'")]
    InvalidRect { line: usize, text: String },
    #[error("expected {expected} buttons but found {found}")]
    CountMismatch { expected: usize, found: usize },
    #[error("missing [End] terminator")]
    MissingEnd,
}

/// Parse a comma-separated rectangle `x1,y1,x2,y2` from a line.
///
/// The original data files sometimes have trailing spaces after rect values
/// (e.g., "1,199,24,222 "), so we trim before splitting. Values are signed
/// (i32) because some sprite sheets use negative offsets for alignment.
fn parse_rect(s: &str, lineno: usize) -> Result<Rect, ButtonError> {
    let trimmed = s.trim();
    let parts: Vec<&str> = trimmed.split(',').collect();
    if parts.len() != 4 {
        return Err(ButtonError::InvalidRect {
            line: lineno,
            text: s.to_string(),
        });
    }
    let vals: Result<Vec<i32>, _> = parts.iter().map(|p| p.trim().parse::<i32>()).collect();
    let vals = vals.map_err(|_| ButtonError::InvalidRect {
        line: lineno,
        text: s.to_string(),
    })?;
    Ok(Rect {
        x1: vals[0],
        y1: vals[1],
        x2: vals[2],
        y2: vals[3],
    })
}

/// Parse an integer from a trimmed line.
fn parse_int<T: std::str::FromStr>(s: &str, lineno: usize) -> Result<T, ButtonError> {
    s.trim()
        .parse::<T>()
        .map_err(|_| ButtonError::InvalidInteger {
            line: lineno,
            text: s.to_string(),
        })
}

/// Parse a `.BTN` button layout file.
///
/// Reads the `[NrButtons]` header, then `[Button]` blocks (13 data lines each),
/// terminated by `[End]`.
pub fn parse_buttons(path: &Path) -> Result<ButtonLayout, ButtonError> {
    info!(path = %path.display(), "Parsing BTN button layout");

    let raw = std::fs::read_to_string(path)?;

    // Collect lines, stripping \r, preserving 1-based line numbers.
    let lines: Vec<(usize, &str)> = raw
        .split('\n')
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_end_matches('\r').trim()))
        .collect();

    let mut idx = 0;

    // Skip any leading blank lines.
    while idx < lines.len() && lines[idx].1.is_empty() {
        idx += 1;
    }

    // The [NrButtons] section header must appear first. This bracket-delimited
    // format is an INI-style convention used across several Wages of War file types.
    if idx >= lines.len() || !lines[idx].1.eq_ignore_ascii_case("[NrButtons]") {
        return Err(ButtonError::MissingHeader);
    }
    idx += 1;

    // Skip blanks after header marker.
    while idx < lines.len() && lines[idx].1.is_empty() {
        idx += 1;
    }

    // Read button count.
    if idx >= lines.len() {
        return Err(ButtonError::MissingHeader);
    }
    let (count_lineno, count_str) = lines[idx];
    let expected_count: usize = count_str.parse().map_err(|_| ButtonError::InvalidCount {
        line: count_lineno,
        text: count_str.to_string(),
    })?;
    debug!(count = expected_count, "Button count declared");
    idx += 1;

    let mut buttons = Vec::with_capacity(expected_count);
    let mut found_end = false;

    while idx < lines.len() {
        // Skip blank lines between buttons.
        while idx < lines.len() && lines[idx].1.is_empty() {
            idx += 1;
        }
        if idx >= lines.len() {
            break;
        }

        let (marker_lineno, marker) = lines[idx];

        // [End] is required — it marks the definitive end of the button list.
        // Without it, we can't distinguish a truncated file from a complete one.
        if marker.eq_ignore_ascii_case("[End]") {
            debug!(line = marker_lineno, "Hit [End] terminator");
            found_end = true;
            break;
        }

        // Expect [Button] marker.
        if !marker.eq_ignore_ascii_case("[Button]") {
            return Err(ButtonError::ExpectedButtonMarker {
                line: marker_lineno,
                text: marker.to_string(),
            });
        }
        idx += 1;

        // Each button block has exactly 13 data lines after the [Button] marker.
        // The order is fixed: field_1, field_2, page, id, hit_rect, then 4 sprite
        // rects (normal, pressed, hover, disabled), then 4 unknown params.
        let mut data_lines: Vec<(usize, &str)> = Vec::with_capacity(13);
        while data_lines.len() < 13 && idx < lines.len() {
            data_lines.push(lines[idx]);
            idx += 1;
        }

        if data_lines.len() < 13 {
            return Err(ButtonError::ExpectedButtonMarker {
                line: data_lines.last().map(|l| l.0).unwrap_or(marker_lineno),
                text: format!(
                    "unexpected end of file, only {} of 13 lines for button",
                    data_lines.len()
                ),
            });
        }

        // Parse the 13 data lines in order. Lines 1-4 are scalar integers,
        // line 5 is the screen-space hit rect, lines 6-9 are sprite source rects
        // for the 4 visual states, and lines 10-13 are unknown params.
        let field_1: u32 = parse_int(data_lines[0].1, data_lines[0].0)?;
        let field_2: u32 = parse_int(data_lines[1].1, data_lines[1].0)?;
        let page: u32 = parse_int(data_lines[2].1, data_lines[2].0)?;   // tab grouping
        let id: u32 = parse_int(data_lines[3].1, data_lines[3].0)?;     // unique button ID
        let hit_rect = parse_rect(data_lines[4].1, data_lines[4].0)?;   // clickable area
        let sprite_normal = parse_rect(data_lines[5].1, data_lines[5].0)?;   // idle look
        let sprite_pressed = parse_rect(data_lines[6].1, data_lines[6].0)?;  // click look
        let sprite_hover = parse_rect(data_lines[7].1, data_lines[7].0)?;    // mouseover look
        let sprite_disabled = parse_rect(data_lines[8].1, data_lines[8].0)?; // grayed look
        let param_1: i32 = parse_int(data_lines[9].1, data_lines[9].0)?;
        let param_2: i32 = parse_int(data_lines[10].1, data_lines[10].0)?;
        let param_3: i32 = parse_int(data_lines[11].1, data_lines[11].0)?;
        let param_4: i32 = parse_int(data_lines[12].1, data_lines[12].0)?;

        trace!(
            id,
            page,
            hit_rect = ?hit_rect,
            "Parsed button"
        );

        buttons.push(Button {
            field_1,
            field_2,
            page,
            id,
            hit_rect,
            sprite_normal,
            sprite_pressed,
            sprite_hover,
            sprite_disabled,
            param_1,
            param_2,
            param_3,
            param_4,
        });
    }

    if !found_end {
        warn!("No [End] terminator found in button file");
        return Err(ButtonError::MissingEnd);
    }

    // Validate that the declared count matches actual buttons parsed. A mismatch
    // usually indicates a corrupted or hand-edited file — we reject it rather
    // than silently using partial data, since missing buttons would break the UI.
    if buttons.len() != expected_count {
        warn!(
            expected = expected_count,
            found = buttons.len(),
            "Button count mismatch"
        );
        return Err(ButtonError::CountMismatch {
            expected: expected_count,
            found: buttons.len(),
        });
    }

    info!(count = buttons.len(), "Finished parsing button layout");
    Ok(ButtonLayout { buttons })
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

    const FULLMAP_BTN: &str = "\
[NrButtons]\r\n\
1\r\n\
[Button]\r\n\
0\r\n\
0\r\n\
0\r\n\
1\r\n\
0,0,639,479\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0\r\n\
0\r\n\
0\r\n\
0\r\n\
[End]\r\n";

    #[test]
    fn parse_fullmap_single_button() {
        let f = write_temp_file(FULLMAP_BTN);
        let layout = parse_buttons(f.path()).unwrap();
        assert_eq!(layout.buttons.len(), 1);

        let btn = &layout.buttons[0];
        assert_eq!(btn.id, 1);
        assert_eq!(btn.page, 0);
        assert_eq!(btn.field_1, 0);
        assert_eq!(btn.field_2, 0);
        assert_eq!(
            btn.hit_rect,
            Rect {
                x1: 0,
                y1: 0,
                x2: 639,
                y2: 479
            }
        );
        assert!(btn.sprite_normal.is_empty());
        assert!(btn.sprite_pressed.is_empty());
        assert!(btn.sprite_hover.is_empty());
        assert!(btn.sprite_disabled.is_empty());
        assert_eq!(btn.param_1, 0);
        assert_eq!(btn.param_2, 0);
        assert_eq!(btn.param_3, 0);
        assert_eq!(btn.param_4, 0);
    }

    #[test]
    fn parse_two_buttons_with_sprites() {
        let data = "\
[NrButtons]\r\n\
2\r\n\
[Button]\r\n\
0\r\n\
0\r\n\
0\r\n\
7\r\n\
344,432,414,455\r\n\
432,25,502,48\r\n\
504,25,574,48\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0\r\n\
0\r\n\
0\r\n\
0\r\n\
[Button]\r\n\
0\r\n\
0\r\n\
1\r\n\
16\r\n\
429,328,452,350\r\n\
79,1,102,24\r\n\
131,1,154,24\r\n\
105,1,128,24\r\n\
105,1,128,24\r\n\
0\r\n\
0\r\n\
0\r\n\
0\r\n\
[End]\r\n";
        let f = write_temp_file(data);
        let layout = parse_buttons(f.path()).unwrap();
        assert_eq!(layout.buttons.len(), 2);

        // First button: page 0, id 7, has normal+pressed sprites only.
        let b1 = &layout.buttons[0];
        assert_eq!(b1.id, 7);
        assert_eq!(b1.page, 0);
        assert_eq!(
            b1.hit_rect,
            Rect {
                x1: 344,
                y1: 432,
                x2: 414,
                y2: 455
            }
        );
        assert_eq!(
            b1.sprite_normal,
            Rect {
                x1: 432,
                y1: 25,
                x2: 502,
                y2: 48
            }
        );
        assert!(!b1.sprite_normal.is_empty());
        assert!(b1.sprite_hover.is_empty());

        // Second button: page 1, id 16, all four sprite states set.
        let b2 = &layout.buttons[1];
        assert_eq!(b2.id, 16);
        assert_eq!(b2.page, 1);
        assert_eq!(b2.sprite_hover, b2.sprite_disabled);
    }

    #[test]
    fn count_mismatch_is_error() {
        let data = "\
[NrButtons]\r\n\
2\r\n\
[Button]\r\n\
0\r\n\
0\r\n\
0\r\n\
1\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0\r\n\
0\r\n\
0\r\n\
0\r\n\
[End]\r\n";
        let f = write_temp_file(data);
        let err = parse_buttons(f.path()).unwrap_err();
        assert!(matches!(
            err,
            ButtonError::CountMismatch {
                expected: 2,
                found: 1
            }
        ));
    }

    #[test]
    fn missing_end_is_error() {
        let data = "\
[NrButtons]\r\n\
1\r\n\
[Button]\r\n\
0\r\n\
0\r\n\
0\r\n\
1\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0,0,0,0\r\n\
0\r\n\
0\r\n\
0\r\n\
0\r\n";
        let f = write_temp_file(data);
        let err = parse_buttons(f.path()).unwrap_err();
        assert!(matches!(err, ButtonError::MissingEnd));
    }

    #[test]
    fn missing_header_is_error() {
        let data = "garbage\r\n";
        let f = write_temp_file(data);
        let err = parse_buttons(f.path()).unwrap_err();
        assert!(matches!(err, ButtonError::MissingHeader));
    }

    #[test]
    fn rect_is_empty_check() {
        let zero = Rect {
            x1: 0,
            y1: 0,
            x2: 0,
            y2: 0,
        };
        assert!(zero.is_empty());
        let nonzero = Rect {
            x1: 1,
            y1: 2,
            x2: 3,
            y2: 4,
        };
        assert!(!nonzero.is_empty());
    }

    #[test]
    fn trailing_whitespace_in_rect() {
        // Real data has trailing spaces after some rect values (e.g., "1,199,24,222 ").
        let data = "\
[NrButtons]\r\n\
1\r\n\
[Button]\r\n\
0\r\n\
0\r\n\
0\r\n\
37\r\n\
608,448,630,470\r\n\
1,199,24,222 \r\n\
53,199,76,222\r\n\
27,199,50,222\r\n\
27,199,50,222\r\n\
0\r\n\
0\r\n\
0\r\n\
0\r\n\
[End]\r\n";
        let f = write_temp_file(data);
        let layout = parse_buttons(f.path()).unwrap();
        assert_eq!(layout.buttons[0].id, 37);
        assert_eq!(
            layout.buttons[0].sprite_normal,
            Rect {
                x1: 1,
                y1: 199,
                x2: 24,
                y2: 222
            }
        );
    }
}
