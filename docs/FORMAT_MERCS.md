# MERCS.DAT Format Specification

## Overview

`MERCS.DAT` is the master mercenary roster file for *Wages of War: The Business of Battle*. It contains **57 mercenary records** in plaintext, CR/LF line endings, separated by `<` delimiters. Each record defines a mercenary's identity, attributes, hiring costs, and biography. This is the hiring-pool data used by the game's economy/recruitment UI.

The file is human-editable with any text editor (confirmed text/INI-style, not binary).

## Record Structure

Each mercenary record follows a fixed line layout. Blank lines appear between logical groups but are not significant to parsing. Field labels are literal string prefixes (e.g., `Name:`, `RATING:`) with values separated by whitespace.

```
Line  1:  Name:  <full_name>
Line  2:  Nickname:  <nickname>
Line  3:  Age:  <age>	Hgt:  <feet> <inches>	Wgt:  <weight> lbs.
Line  4:  Nation:  <nation>
Line  5:  (blank)
Line  6:  Missions:	Missions Completed:
Line  7:  (blank)
Line  8:  RATING:  <rating>             DPR:  <dpr>       PSG:  <psg>         AVAIL: <avail>
Line  9:  (blank)
Line 10:  EXP:  <exp>  STR:  <str>  AGL:  <agl>
Line 11:  WIL:  <wil>  WSK:  <wsk>  HHC:  <hhc>
Line 12:  TCH:  <tch>  ENC:  <enc>  APS:  <aps>
Line 13:  (blank)
Line 14:  Fees:  <fee1>	<fee2>	<fee3>
Line 15:  mail: <mail>
Line 16:  (blank or whitespace)
Line 17:  <biography text>
```

### Whitespace Notes

- Fields on multi-value lines are separated by variable amounts of whitespace (spaces and tabs mixed). Parsers must split on any whitespace.
- The `Fees:` line uses tabs between values, but tab count varies (1 or 2 tabs).
- Height is two space-separated integers on the same field (feet and inches).
- Names use underscores in place of spaces (e.g., `Maria_Hernandez`, `South_Korea`).

## Field Definitions

| Field | Line | Type | Range (observed) | Description |
|-------|------|------|-------------------|-------------|
| `Name` | 1 | String | — | Full name, underscores for spaces. `NOT_AVAILABLE` used when real name unknown. |
| `Nickname` | 2 | String | — | Short display name / callsign, underscores for spaces. |
| `Age` | 3 | Integer | 19–46 | Mercenary's age in years. |
| `Hgt` | 3 | Integer Integer | 5 2 – 6 5 | Height as `<feet> <inches>` (two separate integers). |
| `Wgt` | 3 | Integer | 110–265 | Weight in pounds. Followed by literal ` lbs.` suffix. |
| `Nation` | 4 | String | — | Nationality / country of origin. Underscores for spaces. |
| `Missions` | 6 | (empty) | — | Mission count — appears to be blank in the base roster (populated at runtime). |
| `Missions Completed` | 6 | (empty) | — | Completed mission count — blank in base roster. |
| `RATING` | 8 | Integer | 10–91 | Overall mercenary rating. Composite score; higher = better. |
| `DPR` | 8 | Integer | 112–190 | **Daily Pay Rate.** Base cost per day to employ this mercenary. |
| `PSG` | 8 | Integer | -160 – 420 | **Prestige.** Reputation score. Can be negative for inexperienced mercs. Affects contract availability and client trust. |
| `AVAIL` | 8 | Integer | 0 or 1 | **Availability.** 1 = available for hire, 0 = unavailable (presumably set at runtime when already hired or KIA). |
| `EXP` | 10 | Integer | 05–94 | **Experience.** Combat experience level. |
| `STR` | 10 | Integer | 12–93 | **Strength.** Physical power; affects melee damage, carry capacity. |
| `AGL` | 10 | Integer | 06–95 | **Agility.** Speed/dexterity; affects dodge, movement. |
| `WIL` | 11 | Integer | 03–90 | **Willpower.** Mental fortitude; affects suppression resistance, morale. |
| `WSK` | 11 | Integer | 09–90 | **Weapon Skill.** Ranged weapon accuracy/proficiency. |
| `HHC` | 11 | Integer | 14–88 | **Hand-to-Hand Combat.** Melee combat skill rating. |
| `TCH` | 12 | Integer | 05–95 | **Tech.** Technical aptitude; affects lockpicking, explosives, medical, repair. |
| `ENC` | 12 | Integer | 225, 300, 375 | **Encumbrance capacity.** Max carry weight in game units. Appears to come in 3 tiers correlated with STR/Wgt. |
| `APS` | 12 | Integer | 30–50 | **Action Points.** Base AP per combat turn. Determines how many actions a merc can take per round. |
| `Fees` | 14 | Integer x3 | varies | Three hiring fee tiers: `<fee1> <fee2> <fee3>`. Likely correspond to contract lengths or mission difficulty tiers. Tab-separated. |
| `mail` | 15 | Integer | 0 or 1 | Mail flag. 1 = has introductory mail/message available, 0 = none. Likely triggers an in-game mail from this merc. |
| `Biography` | 17 | String | — | Free-form text paragraph. Character backstory shown in the hiring UI. Single line, can be long. |

### ENC Tier Observations

| ENC Value | Typical Profile |
|-----------|-----------------|
| 225 | Lighter/smaller mercs (Wgt ~110-167) |
| 300 | Average build mercs (Wgt ~140-220) |
| 375 | Heavy/strong mercs (STR >= ~77 or Wgt >= ~195) |

### Fee Tier Observations

Fees scale linearly with RATING. Low-rated mercs (RATING 10-15) all share the same fee structure: `21000 / 9000 / 39500`. The three values likely represent different contract types or durations. Fee 2 is always the lowest (roughly 43% of Fee 1), and Fee 3 is always the highest (roughly 186% of Fee 1).

## Record Delimiter

Records are separated by a line containing only `<` (plus CR/LF). The file ends with:
```
<
~
~
~
~
~
~
```

The `<` terminates the final record. The trailing `~` lines appear to be end-of-file padding/sentinels. There are typically 4-6 `~` lines after the last record.

## Related Files

### SERG*.DAT / ABDULS*.DAT (Per-Mission Equipment Inventory)

Files like `SERG01.DAT` through `SERG16.DAT` and `ABDULS01.DAT` through `ABDULS16.DAT` are **NOT per-merc state files** — they are **per-mission equipment shop inventories**. The naming pattern is `<dealer_name><mission_number>.DAT`.

**Known dealers:**
- `SERG` — "Serg" (arms dealer), missions 01–16
- `ABDULS` — "Abdul's" (arms dealer), missions 01–16
- `FITZ` — "Fitz" (arms dealer), mission 02 only

#### Inventory File Format

Each item entry is two lines (weapon+ammo pair, or standalone for equipment/thrown weapons):

```
<item_name>
STOCK: <qty>    PRICE: <price>    STATUS:<status>   TYPE:<type>
```

**STATUS values (observed):**
| Status | Meaning |
|--------|---------|
| `STOCKED` | Available for purchase (STOCK > 0) |
| `OUTOFSTOCK` | Temporarily unavailable (STOCK = 0) |
| `UNAVAILABLE` | Not carried by this dealer |
| `COMINGSOON` | Will become available in a future mission |
| `DISCONTINUED` | No longer sold |
| `EMPTY` | Sentinel/placeholder entry |

**TYPE values (observed):**
| Type | Meaning |
|------|---------|
| `WEAPON` | Firearm (paired with AMMO line following) |
| `AMMO` | Ammunition for preceding WEAPON |
| `WEAPON2` | Thrown weapons, grenades, melee weapons, rockets (no ammo pairing) |
| `EQUIPMENT` | Gear items (armor, tools, medical kits, etc.) |
| `EMPTY` | End-of-list sentinel |

#### Key Differences Between Dealers Per Mission

- **SERG01** (mission 1): All items STOCK=0, STATUS=OUTOFSTOCK/UNAVAILABLE/DISCONTINUED. This is the "empty" baseline.
- **ABDULS01** (Abdul's, mission 1): Many items STOCKED with qty > 0. Prices are generally **lower** than SERG's prices. Abdul's is available from mission 1 with real inventory.
- **SERG02** matches SERG01 exactly — SERG likely becomes available later or has different stock per campaign state.
- **FITZ02**: Only one file exists. Fitz may be a limited-availability dealer.

The inventory files terminate with:
```
Empty
STOCK: 0    PRICE: 0       STATUS:EMPTY     TYPE:EMPTY
~
~
```

## Notes

1. **All values are zero-padded in some records** — e.g., `EXP: 05`, `WIL: 03`. The parser should treat these as plain integers.

2. **Whitespace is inconsistent.** The original developers used variable spacing (tabs and spaces mixed) to visually align columns. A robust parser must tokenize on any whitespace, not fixed column positions.

3. **PSG can be negative.** Low-rated/inexperienced mercs have negative prestige (e.g., `-160`). This likely gates which contracts they can be assigned to.

4. **RATING appears to be a derived stat** — it correlates strongly with the average of EXP, WSK, HHC, and WIL, but the exact formula is unconfirmed. Top mercs (RATING 80+) have consistently high attributes across the board.

5. **The Missions / Missions Completed fields** on line 6 are blank in the base roster. These are populated at runtime as the player completes missions. The field labels exist as placeholders with values presumably appended after the tab stops.

6. **Name underscores** — all spaces in names, nicknames, and nation names are replaced with underscores. The engine likely displays these as spaces in the UI.

7. **57 mercenaries total** in the base roster. The first merc is Maria Hernandez; the last is Natasha Oblonsky (based on file order).

8. **DPR range (112–190)** — even the cheapest mercs cost a minimum of 112/day, making the economy system a meaningful constraint from the start.

9. **APS range (30–50)** — the 20-point spread means elite mercs get ~67% more actions per turn than rookies, a massive tactical advantage.
