# FORMAT_TEXT.md -- Text / String / Narrative Data Files

All text data files use DOS line endings (`\r\n`). All are plaintext, human-editable.

---

## ENGWOW.DAT -- Engine String Table

**Purpose:** Master string table for all UI text, error messages, status labels, menu items, format strings, and debug messages referenced by the engine at runtime.

**Format:** One string per line, **indexed by 1-based line number**. The engine loads strings by ordinal position (line 1 = string index 1, line 2 = string index 2, etc.). 483 lines total in the shipping data, terminated by a sentinel line containing only `~`.

**Encoding:** ASCII with DOS line endings (`\r\n`). Some strings contain literal `\r` escape sequences (not actual carriage returns) -- these are interpreted by the engine's text renderer as newline/line-break within a single displayed text block.

**Embedded format specifiers (C printf-style):**

| Specifier | Meaning | Example |
|-----------|---------|---------|
| `%s` | String (merc name, item name) | `"First Aid has been applied to %s."` (line 222) |
| `%d` | Integer (counts, costs, AP) | `"Action Point cost is %d."` (line 12) |
| `%hd` | Short integer (page numbers) | `"Page #%hd < MORE >"` (line 17) |

**Embedded control sequences:**

| Sequence | Meaning |
|----------|---------|
| `\r` | Line break within a rendered text block (used in multi-line formatted strings like order receipts) |
| `\r\` | Page break (seen in `\r\Page #%hd`, line 84) |

**String categories by line range (approximate):**

| Lines | Category |
|-------|----------|
| 1-4 | Debug/diagnostic messages (transparent level, WAV errors, animation sets) |
| 5-33 | Combat messages (targeting, movement, ammo, explosions, fatal errors) |
| 34-46 | HQ main menu buttons (View Files, Use Fax Machine, Hire Mercenaries, Begin Mission, etc.) |
| 47-55 | HQ warnings and mercenary hiring |
| 56-81 | Mercenary rolodex UI (stat labels: RAT, ENC, AP, EXP, STR, AGL, WIL, WSK, HHC, TCH, fees) |
| 82-92 | Equipment ordering UI (receipts, sub totals, dev/debug recorder messages) |
| 93-104 | Month names (January through December) |
| 105-142 | Tactical combat UI (pickup/drop, ammo exchange, movement, hand-to-hand, equip screen buttons) |
| 143-152 | Calculator / cheat code |
| 153-176 | Contract negotiation UI (fax, advance/bonus/deadline, offers) |
| 177-187 | Intelligence, travel, folder, exchange screen buttons |
| 188-197 | Quality ratings (Excellent, Above Average, Average, Below Average, Acceptable) and experience tiers (Elite, Crack, Line, Green, Unproven) |
| 198-222 | Vid-Phone, training, first aid, mortar/grenade targeting |
| 223-236 | Merc stat display labels (compact format), company names (The WAR Company, Attack! Inc., Alpha Force Inc., Soldiers Ltd., MERCS Inc.) |
| 237-270 | Status messages (dead, broke, out of ammo, jammed, suppressed, grenade dud, parachute, drop zone) |
| 271-282 | Stat improvement labels, radio contact, injury events, equipment canister |
| 283-293 | Unit status labels (OK, DEAD, Suppressed, Panicked, Incapacitated, Wounded, Berserk, Unconscious, Stunned, Surrendered, Escaped) |
| 294-333 | Tactical HUD buttons and actions (Name, Health, Movement, Stand, Kneel, Prone, Walk, Run, Crawl, First Aid, Carry Merc, Target Mortar, Overhead Map, etc.) |
| 334-340 | Intelligence briefing labels (Expected Resistance, Men, Experience, FirePower, Chance of Success, Expected Casualties, Suggested Mercs) |
| 341-367 | Weapon type names (Rifle, Pistol, Shotgun, Machine Gun, Uzi, Cross Bow, etc.) and stat labels (WR, DC, PEN, ENC, Type) |
| 368-388 | Weather strings (Clear, Foggy, Overcast, Light Rain, Heavy Rain, Storm, Light Snow, Heavy Snow) and wind directions (South through Southeast, 8 compass points) |
| 389-411 | Equipment catalog UI (Abdul's Armaments, Lock-N-Load Ltd., Sergeant's, weapon categories, page controls) |
| 412-431 | Mission titles (16 missions: "The CEO's Kid(nap)", "The BRIEFest of Moments", "The Stinger Sting", etc.) and misc combat strings |
| 432-457 | Main menu / pause menu (Campaign, Scenario, Load Game, Save Game, Options, Quit, Resume Game, Credits) |
| 458-471 | Options screen labels (SOUND, MUSIC, DETAIL, DIFFICULTY) and save/load error messages |
| 472-482 | Misc (Enter Your Name, mine detection, evacuation, mission over, starting balance, cyborg) |
| 483 | Sentinel: `~` |

**Parsing rules for the engine reimplementation:**
1. Read file line-by-line. Each line (after stripping `\r\n`) is one string entry.
2. Index strings 1-based (line 1 = index 1).
3. Stop reading at the `~` sentinel line.
4. At runtime, look up strings by index. Apply `printf`-style formatting with `%s`, `%d`, `%hd` specifiers.
5. When rendering, interpret literal `\r` sequences as line breaks within the text display area.

---

## Narrative Text Files (Mission Briefings / Outcomes)

These files share a common format: **line-count-prefixed text blocks**.

### Common Format

```
<line_count>\r\n
<text line 1>\r\n
<text line 2>\r\n
...
<text line N>\r\n
```

Each block begins with an integer on its own line specifying how many text lines follow. Multiple blocks are concatenated sequentially. There is no sentinel; parsing ends at EOF.

The line count tells the engine how many lines to read for that block, which determines the height of the text display area.

---

### CUTTXT.DAT -- Cutscene / Mission Intro Text

**Purpose:** Pre-mission briefing text shown during cutscene screens before each mission begins. One block per mission, in mission order.

**Format:** Alternating line-count + text blocks. Each block starts with an integer (line count), then a label line (e.g., `Mission 1:`), followed by narrative text lines.

**Example:**
```
4
Mission 1:
Your team is in position on the high ground surrounding Salvitore's
Hacienda.  After a quick check of the compound, you lay out your
plan of attack...
3
Mission 2:
The chopper lands in the Dead Zone and as your team disembarks,
you warn them of their time constraints...
```

**Block inventory:** 17 blocks total (Missions 1-15, plus one "Surprise!" block for the base defense mission, plus a final mission block). The line count includes the label line.

**Note:** Some blocks contain curly/smart quote artifacts (`M-^R` = 0x92, a Windows-1252 right single quote used as apostrophe in "Salvitore's").

---

### SUCTXT.DAT -- Mission Success Text

**Purpose:** Text displayed when a mission is completed successfully.

**Format:** Single block. Line count prefix, then congratulatory text.

**Content:**
```
2
Congratulations!
You have successfully completed your mission.
```

This is a generic success message used for all missions. (Mission-specific success details come from GSUCTXT.DAT for campaign end.)

---

### LOSETXT.DAT -- Mission Failure Text

**Purpose:** Text displayed when a mission fails. Multiple blocks for different failure conditions.

**Format:** Line-count-prefixed blocks, one per failure reason.

**Block inventory (12 blocks = 6 failure scenarios):**

| Block | Condition |
|-------|-----------|
| 1 | Team thwarted (generic failure) |
| 2 | Did not reach objective |
| 3 | Reputation too low -- forced out of business |
| 4 | Out of funds -- forced out of business |
| 5 | Can't pay Vinnie -- forced out of business |
| 6 | ARM blackball for high body counts |
| 7 | Base defended but can't reorganize in time |
| 8 | Hostile takeover by rival |

Each block is 2 lines of text preceded by the count `2`.

---

### GLOSETXT.DAT -- Campaign/Game Loss Text

**Purpose:** End-of-campaign failure epilogues (game over screens).

**Format:** Same line-count-prefixed blocks.

**Block inventory (4 blocks):**

| Block | Scenario |
|-------|----------|
| 1 | Lack of business skills -- land sold to Japanese investors (3 lines) |
| 2 | Don't mess with Vinney! (1 line) |
| 3 | Name becomes synonymous with "Loser" (2 lines) |
| 4 | ARM blackball for body counts (2 lines) |

---

### GSUCTXT.DAT -- Campaign/Game Success Text

**Purpose:** End-of-campaign victory epilogues. Multiple endings based on performance tier.

**Format:** Same line-count-prefixed blocks.

**Block inventory (3 blocks, each 4 lines):**

| Block | Ending |
|-------|--------|
| 1 | Best ending: exceptional strategist, retires wealthy in 2022 |
| 2 | Worst ending: jokes about fighting skills, retires 2012 after Newstime article |
| 3 | Middle ending: comfortable living, retires 2017, lack-luster career |

---

## SPEECH01.DAT -- Contract Speech Scripts

**Purpose:** Dialogue scripts for the first mission's contract negotiation vid-phone calls. Contains the client's spoken lines during hiring.

**Format:** Free-form labeled text (not line-count-prefixed). Sections marked with labels:

```
Speech For Mission 01:

Initial Call:
"<dialogue text>"

Acceptance Script:
"<dialogue text>"

Last Offer Script:
"<dialogue text>"
```

**Note:** Only SPEECH01.DAT was found. Other missions likely have SPEECH02.DAT through SPEECH15.DAT (or speech is hardcoded / shared). The dialogue text includes quoted speech with proper punctuation.

---

## TARGET.DAT -- Hit Probability Lookup Table

**Purpose:** NOT a text/narrative file. This is a **numeric lookup table** for combat hit probability calculations.

**Format:** 2D grid of space-separated integers, with rows indexed 0-based. The table appears to be approximately 100+ rows by 20 columns. Values range from 1 to 98 (percentages).

**Structure:** Each row represents one axis of the to-hit calculation (likely range or skill differential), each column represents the other axis. The cell value is the hit probability percentage.

**Dimensions:** Rows 0-99+ (likely indexed by attacker weapon skill or range), columns 0-19 (likely indexed by a derived modifier). Row 0 is all 98s (point blank / maximum skill). Values decrease as row index increases and column index decreases.

**Parsing:** Read as whitespace-delimited integer grid. Each line is one row. Lines are `\r\n` terminated.

---

## TEXTRECT.DAT, TEXTREC2.DAT, TXTRECT3.DAT, TXTRECT4.DAT -- UI Text Rectangle Definitions

**Purpose:** Define screen-space bounding rectangles for UI text elements. These map ENGWOW.DAT string indices to pixel coordinates for rendering.

**Format:** Each file begins with a line count (e.g., `24 #lines to read` or `40 #lines to read`), followed by pairs of lines defining label and value rectangles. Lines starting with `#` are comments.

**Line format:**
```
<x1> <y1> <x2> <y2> <string_index> #comment
```

Where:
- `x1, y1` = top-left corner of the text rectangle (pixels)
- `x2, y2` = bottom-right corner of the text rectangle (pixels)
- `string_index` = index into ENGWOW.DAT string table (or a merc data field index)
- `#comment` = developer comment (ignored by parser)

**Rectangle pairs:** Each UI element has two lines -- one for the label (e.g., "STR") and one for the value display area. The label line references the ENGWOW.DAT string index for the label text. The value line references the data field index for the actual stat value.

**Files by purpose:**

| File | Purpose | Line count |
|------|---------|------------|
| TEXTREC2.DAT | Merc card stat display (compact/rolodex view): APS, DPS, ENC, STR, EXP, HHC, WIL, AGL, TCH, WSK, rating | 24 entries |
| TEXTRECT.DAT | Merc detail stat display (full view): age, height, weight, missions, completed, nationality, name, nickname, rating, APS, DPS, ENC, all stats, status | 40 entries |
| TXTRECT3.DAT | Weapon info display: weapon name, WR, DC, PEN, ENC, Type | 12 entries |
| TXTRECT4.DAT | Weather/wind display: Weather label+value, Wind label+value | 4 entries |

**Parsing rules:**
1. Read the first line to get the entry count.
2. Read entry lines in pairs (label rect, value rect), skipping blank lines and `#` comment lines.
3. Parse 5 integers per entry line: x1, y1, x2, y2, string_index.

---

## TEMP.DAT -- Mission Definition Template

**Purpose:** NOT a text/string file. This is a **mission configuration file** containing all parameters for a single mission (Mission 1 in the shipped data). Documented here because it contains embedded text strings.

**Format:** Labeled sections with key-value data. Sections include:
- Animation file references (`.cor` files for friendlies, enemies, dogs, NPCs)
- Contract terms (client name, objective text, advance/bonus/deadline values)
- Contract negotiation parameters (counter-offer probabilities)
- Prestige modifiers (success/WIA/MIA/KIA reputation effects)
- Intelligence costs and estimates
- Enemy roster (ratings, stats, weapon loadouts)
- Weather probability table
- Travel cost/time options
- Special action parameters (safe cracking, snake bites)

**Sentinel:** `~` on final line.

**Embedded strings include:**
- Contract "From:" and "Terms:" / "Bonus:" text (freeform narrative)
- Special action result messages (`%s has opened the safe!`, `%s broke the drill bit!`)
- Special damage messages (`%s has been bitten by a snake!`)

See FORMAT_MISSIONS.md (if created) for full mission file documentation.

---

## Summary of All Text Files

| File | Type | Format | Purpose |
|------|------|--------|---------|
| ENGWOW.DAT | String table | 1 string/line, 1-based index, `~` sentinel | All engine UI strings |
| CUTTXT.DAT | Narrative | Line-count-prefixed blocks | Mission intro cutscene text |
| SUCTXT.DAT | Narrative | Line-count-prefixed block | Generic mission success message |
| LOSETXT.DAT | Narrative | Line-count-prefixed blocks | Mission failure messages (multiple causes) |
| GLOSETXT.DAT | Narrative | Line-count-prefixed blocks | Campaign loss epilogues |
| GSUCTXT.DAT | Narrative | Line-count-prefixed blocks | Campaign victory epilogues (tiered endings) |
| SPEECH01.DAT | Dialogue | Free-form labeled sections | Contract vid-phone dialogue |
| TARGET.DAT | Numeric | Space-delimited integer grid | Hit probability lookup table |
| TEXTREC2.DAT | UI layout | Rect definitions with string indices | Merc card stat rectangles |
| TEXTRECT.DAT | UI layout | Rect definitions with string indices | Merc detail stat rectangles |
| TXTRECT3.DAT | UI layout | Rect definitions with string indices | Weapon info rectangles |
| TXTRECT4.DAT | UI layout | Rect definitions with string indices | Weather/wind display rectangles |
| TEMP.DAT | Mission def | Labeled sections, `~` sentinel | Mission 1 full configuration |
