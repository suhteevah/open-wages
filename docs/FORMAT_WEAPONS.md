# FORMAT: WEAPONS.DAT

## Overview

`WEAPONS.DAT` is a plaintext, whitespace-delimited data file containing all weapon definitions for *Wages of War*. The file uses Windows-style line endings (CR/LF).

## File Structure

```
* Comment / header lines (ignored by parser)
*
* NAME, WR, DC, PEN, ENC, ADF, PNC, AOI, JDB, COST, AMMO_per_clip, ammo_enc, ammo_cost, ammo_name, weapon_type
*
<weapon record>
<weapon record>
...
* <separator between weapon categories>
<weapon record>
...
~
```

### Comments and Separators

- Lines beginning with `*` are comments or category separators. The first four lines form a header block that includes a field name legend.
- A lone `*` line (or `*` followed by whitespace) separates weapon categories (pistols, SMGs, shotguns, rifles, etc.).
- A line beginning with `*` immediately before a weapon name (e.g., `*M203`) comments out that weapon — it exists in the data but is disabled.

### Record Terminator

The file ends with a single `~` on its own line, signaling end-of-data.

## Field Definitions

Each weapon record is a single line. Fields are separated by whitespace (spaces/tabs). The `ADF` field uses a hyphenated range (`MIN-MAX`).

| Position | Field Name       | Type       | Example           | Description |
|----------|------------------|------------|-------------------|-------------|
| 1        | `NAME`           | String     | `Colt_Python`     | Weapon name. Underscores replace spaces. May contain apostrophes, ampersands, periods, and hyphens. |
| 2        | `WR`             | Integer    | `6`               | **Weapon Range** — maximum effective range in tiles. Higher = longer range. Pistols 1-9, rifles 12-20, MGs 20, mortars 30. |
| 3        | `DC`             | Integer    | `3`               | **Damage Class** — base damage die/tier. Scales from 1 (weak) to 10 (mortar). Melee weapons use 2-3. |
| 4        | `PEN`            | Integer    | `12`              | **Penetration** — armor-piercing capability. Higher values defeat heavier armor. Ranges from 0 (smoke grenade) to 76 (Panzerfaust). |
| 5        | `ENC`            | Integer    | `35`              | **Encumbrance** — weight/bulk of the weapon in inventory units. Affects carry capacity. Pistols ~16-44, rifles ~108-161, MGs ~296-324. |
| 6        | `ADF`            | Range      | `1-2`             | **Action Die Formula** — min and max number of attacks per action. Format: `MIN-MAX`. Single-shot weapons use `1-1` or `1-2`; burst weapons use `3-10`, `3-15`, etc. `0-0` for melee. |
| 7        | `PNC`            | Integer    | `8`               | **Penalty Cost** — AP (Action Point) cost to fire the weapon. Common values: 2 (crossbow/smoke), 8 (single shot), 15 (semi-auto), 38 (short burst), 45 (long burst), 60 (grenades/launchers), 90 (rockets). |
| 8        | `AOI`            | Integer    | `1`               | **Area of Impact** — splash/blast radius indicator. 0 = melee/none, 1 = single target, 3 = burst fire, 7 = explosive, 12 = mortar/grenade launcher. -1 = smoke (special effect). |
| 9        | `JDB`            | Integer    | `1`               | **Job/Delivery Behavior** — how the projectile is delivered. 0 = melee/hitscan, 1 = revolver (no casing), 2 = semi-auto (ejects casing), 3 = automatic, 4 = explosive/lobbed, 5 = rocket/thrown. |
| 10       | `COST`           | Integer    | `3200`            | **Weapon Cost** — purchase price in game currency. 0 for unobtainable items (dog teeth). |
| 11       | `AMMO_per_clip`  | Integer    | `6`               | **Ammo Per Clip** — rounds per magazine/reload. 0 for single-use items (grenades, rockets). |
| 12       | `ammo_enc`       | Integer    | `16`              | **Ammo Encumbrance** — weight/bulk of one ammo clip. 0 for items with no separate ammo. |
| 13       | `ammo_cost`      | Integer    | `44`              | **Ammo Cost** — price per clip of ammunition. 0 for items with no purchasable ammo. |
| 14       | `ammo_name`      | String     | `9_x_33mmR`       | **Ammo Name** — caliber/type identifier. May contain spaces (e.g., `4.7_x_33mm DM11 Caseless`). `None` for melee weapons. Underscores replace spaces within caliber designations. |
| 15       | `weapon_type`    | Integer    | `2`               | **Weapon Type** — category enumeration (see table below). |

## Weapon Type Enumeration

| Value | Category                  | Examples |
|-------|---------------------------|----------|
| 0     | Rifle / Assault Rifle     | M16A1, AKM-47, KAR-98K |
| 1     | Crossbow                  | Crossbow |
| 2     | Pistol / Handgun          | Colt Python, Makarov PM |
| 3     | Shotgun / Grenade Launcher| Remington 870P, M79 |
| 4     | Machine Gun               | PKMS, M-60 |
| 5     | Submachine Gun            | Ingram M10, Uzi, MP5A2 |
| 7     | Frag / HE Grenade         | M26 A2, GR 24, M2 Pineapple |
| 8     | Melee Weapon              | Bowie Knife, Machete, DOG_TEETH |
| 9     | Mortar                    | M19 |
| 10    | Land Mine                 | Land Mine |
| 12    | Smoke Grenade             | AN-M8 HC Smoke |
| 13    | Satchel Charge            | Satchel Charge |
| 14    | Disposable Rocket (drop)  | Panzerfaust 100 |
| 15    | Disposable Rocket (keep)  | M72 A2 |

*Note: Values 6 and 11 are not observed in the data file.*

## Weapon Categories (by `*` separator blocks)

The records are grouped into categories separated by `*` lines, in this order:

1. **Pistols** (type 2) — 10 entries
2. **Submachine Guns** (type 5) — 8 entries
3. **Shotguns** (type 3) — 5 entries
4. **Rifles & Assault Rifles** (type 0) — 13 entries
5. **Machine Guns** (type 4) — 2 entries
6. **Grenade Launchers** (type 3) — 1 entry (M79); M203 is commented out
7. **Rockets / Anti-Tank** (types 14, 15) — 2 entries
8. **Crossbow** (type 1) — 1 entry
9. **Grenades & Explosives** (types 7, 12, 13) — 8 entries
10. **Melee Weapons** (type 8) — 5 entries
11. **Mortars** (type 9) — 1 entry
12. **Mines & Special** (types 8, 10) — 2 entries (Land Mine, DOG_TEETH)

## Parsing Notes

1. **Delimiter:** Fields are separated by variable whitespace (spaces and/or tabs). A robust parser should split on `\s+`.
2. **Ammo name parsing:** The `ammo_name` field (position 14) can contain embedded spaces (`4.7_x_33mm DM11 Caseless`). Since `weapon_type` is always the last field and is a single integer, parse the line by: extracting field 1 (name), fields 2-11 (integers, with field 6 as a range), fields 12-13 (integers), then everything between field 13 and the last whitespace-delimited token is the ammo name, and the final token is weapon_type.
3. **Commented-out weapons:** Lines starting with `*` followed immediately by a weapon name (no space after `*`) represent disabled entries. The `*M203` line has full field data but should be skipped.
4. **Range field (`ADF`):** Must be parsed as two integers split on `-`.
5. **Line endings:** CR/LF (`\r\n`). Strip `\r` before parsing.
6. **Encoding:** ASCII / Windows-1252.
7. **Record count:** 57 weapon entries (including DOG_TEETH, excluding commented-out M203).
