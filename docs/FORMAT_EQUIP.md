# FORMAT: EQUIP.DAT

## Overview

`EQUIP.DAT` is a plaintext data file containing all non-weapon equipment definitions for *Wages of War*. The file uses Windows-style line endings (CR/LF). Each item occupies exactly two lines.

## File Structure

```
<item name>
PEN: <value>    ENC: <value>
<item name>
PEN: <value>    ENC: <value>
...
~
```

### Record Terminator

The file ends with a single `~` on its own line, identical to `WEAPONS.DAT`.

## Record Format

Each equipment item is defined by a consecutive pair of lines:

### Line 1: Item Name

| Field     | Type   | Description |
|-----------|--------|-------------|
| Name      | String | Full item name in plain text (spaces allowed, no underscore substitution). May contain apostrophes, hyphens, and other punctuation. |

### Line 2: Properties

A fixed-format key-value line with two labeled fields:

| Field | Type    | Format              | Description |
|-------|---------|---------------------|-------------|
| `PEN` | Integer | `PEN: <value>`      | **Penetration resistance** — the armor protection value this item provides. Only body armor items have non-zero values. See interpretation below. |
| `ENC` | Integer | `ENC: <value>`      | **Encumbrance** — weight/bulk in inventory units. Same unit system as `WEAPONS.DAT` encumbrance values. |

The two fields are separated by variable whitespace (spaces and/or tabs). The format uses labeled fields (`PEN:` and `ENC:`) rather than positional parsing.

## PEN Field Interpretation

The `PEN` field on equipment represents **armor penetration resistance** (i.e., protection rating), not a penalty. Evidence:

- Only body armor items have non-zero `PEN` values.
- The values form a clear armor tier progression:
  - Light Flexible Body Armor: PEN 6
  - Heavy Flexible Body Armor: PEN 14
  - Rigid Body Armor: PEN 25
- These values are on the same scale as the `PEN` (penetration) field in `WEAPONS.DAT`. Combat resolution likely compares weapon PEN vs. armor PEN to determine if a hit penetrates.
- All non-armor items have PEN 0.

## Complete Item List

| Item Name                              | PEN | ENC | Category |
|----------------------------------------|-----|-----|----------|
| Enhanced Infantry Medical Kit          | 0   | 5   | Medical |
| Light Flexible Body Armor              | 6   | 8   | Armor |
| Heavy Flexible Body Armor              | 14  | 14  | Armor |
| Rigid Body Armor                       | 25  | 45  | Armor |
| Entrenching Tool                       | 0   | 32  | Tool |
| Fence Cutters                          | 0   | 22  | Tool |
| Night Vision Goggles                   | 0   | 14  | Gear |
| High Altitude Parachute System         | 0   | 65  | Gear |
| Parachute Released Equipment Canister  | 0   | 750 | Gear |
| Rappelling Equipment                   | 0   | 5   | Gear |
| Safe Cracker's Tool Kit                | 0   | 31  | Tool |
| Field Radio                            | 0   | 40  | Gear |
| Explosive's Timer                      | 0   | 6   | Tool |
| Parachute Illumination Flare           | 0   | 18  | Gear |
| BluePrints                             | 0   | 0   | Mission Item |
| Laser Rifle                            | 0   | 0   | Mission Item |
| BriefCase                              | 0   | 0   | Mission Item |
| Computer Disk                          | 0   | 0   | Mission Item |
| Airline Peanuts                        | 0   | 0   | Mission Item |
| Note From Mother                       | 0   | 0   | Mission Item |
| Viking Helmet                          | 0   | 0   | Mission Item |
| Casino Chip                            | 0   | 0   | Mission Item |
| Dog Collar                             | 0   | 0   | Mission Item |
| Canned Moose                           | 0   | 0   | Mission Item |
| Narf Narf Fishy Fishy                  | 0   | 0   | Mission Item |

*Note: The "Category" column is editorial — it is not present in the data file. Items with PEN=0 and ENC=0 are mission-specific collectibles or joke items.*

## Parsing Notes

1. **Two-line records:** Read lines in pairs. Line N is the name, line N+1 is the PEN/ENC data.
2. **Label-based parsing:** The property line uses `PEN:` and `ENC:` labels. Parse by finding these tokens rather than relying on column positions, as whitespace varies (spaces vs. tabs, variable padding).
3. **Terminator:** Stop reading when a line contains only `~` (optionally followed by whitespace/CR).
4. **Trailing blank lines:** The file has blank lines after the `~` terminator. Ignore them.
5. **Line endings:** CR/LF (`\r\n`). Strip `\r` before parsing.
6. **Trailing whitespace:** Some name lines have trailing spaces (e.g., `Parachute Released Equipment Canister `). Trim item names.
7. **Encoding:** ASCII / Windows-1252.
8. **Record count:** 25 equipment items.
9. **No cost field:** Unlike `WEAPONS.DAT`, equipment items have no price in this file. Pricing is likely handled elsewhere (shop/economy data).
