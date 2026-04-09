# FORMAT_MSSN.md -- Mission Definition File Format

Covers `MSSN01.DAT` through `MSSN16.DAT` in `DATA/`. Plaintext, Windows line endings (`\r\n`). File terminated by `~` on a line by itself.

All values are whitespace-delimited integers unless noted. `-1` is the universal "null / not applicable" sentinel for index fields. `null` (literal string) is the sentinel for unused sprite slots.

---

## 1. Animation Files

```
Animation Files:
Good Guys: <filename.cor>
Bad Guys:  <filename.cor>
Dogs:      <filename.cor | null>
NPC1:      <filename.cor | null>
NPC2:      <filename.cor | null>
NPC3/VHC1: <filename.cor | null>
NPC4/VHC2: <filename.cor | null>
```

Sprite/animation corpus files (`.cor`) loaded for the mission. Seven slots, fixed order:

| Slot | Purpose | Examples observed |
|------|---------|-------------------|
| Good Guys | Player merc sprites | `jungsld.cor`, `dsrtsld.cor` |
| Bad Guys | Enemy combatant sprites | `jungemy.cor`, `dsrtemy.cor`, `camboemy.cor` |
| Dogs | Guard dog sprites | `guarddog.cor` or `null` |
| NPC1 | First NPC type | `woman.cor`, `suitguy.cor`, `worker.cor`, `salvitor.cor` or `null` |
| NPC2 | Second NPC type | `sciguy.cor` or `null` |
| NPC3/VHC1 | NPC or vehicle #1 | `copter01.cor`, `copter02.cor`, `truck1.cor` or `null` |
| NPC4/VHC2 | NPC or vehicle #2 | `truck1.cor` or `null` |

**Variation:** The sprite set is biome-dependent. Jungle missions use `jungsld`/`jungemy`, desert uses `dsrtsld`/`dsrtemy`. Dogs and NPCs vary per mission narrative.

---

## 2. Contract

```
Contract:
Date:  <day_of_year> <year>
From:
<client_name_string>

Terms:
<terms_text>
Bonus:
<bonus_terms_text>
Advance/Bonus/Deadline:  <advance_$> <bonus_$> <deadline_day> <deadline_year>
```

| Field | Type | Notes |
|-------|------|-------|
| Date | `u16 u16` | Day-of-year (1-365) and year when contract is offered |
| From | free-text line | Client name / org. May span one line. |
| Terms | free-text line | Mission objective description |
| Bonus | free-text line | Bonus condition description |
| Advance | `u32` | Cash advance paid on accepting the contract (dollars) |
| Bonus | `u32` | Bonus paid on mission success |
| Deadline day | `u16` | Day-of-year by which mission must be complete |
| Deadline year | `u16` | Year of deadline |

**Variation:** Advance ranges from 324,000 (MSSN01) to 993,000+ (MSSN08). Later missions have higher payouts. Day-of-year dates are Julian-style (1-365). All dates observed are in the 2001-2003 range.

---

## 3. Contract Negotiation

Two sub-tables: the player's counter-offer ladder, and the AI's counter-response probability matrix.

### 3a. Player Counter-Offer Ladder

```
Contract Negotiation:
Advance:  <a1> <a2> <a3> <a4>
Bonus:    <b1> <b2> <b3> <b4>
Deadline: <d1> <d2> <d3> <d4>
Chance:   <c1> <c2> <c3> <c4>
```

Four escalating tiers. The player can demand progressively better terms but with decreasing chance of acceptance.

| Row | Values | Meaning |
|-----|--------|---------|
| Advance | 4 x `u32` | Counter-offer advance amounts, ascending |
| Bonus | 4 x `u32` | Counter-offer bonus amounts, ascending |
| Deadline | 4 x `u16` | Counter-offer deadline days, ascending (more time) |
| Chance | 4 x `u8` | Acceptance probability (percent), descending |

**Pattern:** Advance/Bonus increase by a fixed step per tier (typically 25,000 or 30,000). Deadline extends by 1-2 days per tier. Chance drops sharply (e.g., 76 -> 52 -> 28 -> 04).

**Special case:** MSSN16 has all zeros -- no negotiation allowed (non-negotiable government contract).

### 3b. AI Counter-Response Table

```
Counter:  <cv1> <cv2> <cv3> <cv4>  <cd1> <cd2> <cd3> <cd4>
Advance:  <p1>  <p2>  <p3>  <p4>  <p5>  <p6>  <p7>  <p8>
Bonus:    <p1>  <p2>  <p3>  <p4>  <p5>  <p6>  <p7>  <p8>
Deadline: <p1>  <p2>  <p3>  <p4>  <p5>  <p6>  <p7>  <p8>
```

The `Counter` row defines 8 values: 4 dollar amounts and 4 day amounts that the client may counter-offer with. The remaining 3 rows (`Advance`, `Bonus`, `Deadline`) are 8-element probability/weight arrays governing the AI's counter-offer behavior. The values appear to be paired: each pair consists of a threshold and a weight (or a "chance to counter" and a "counter amount index").

| Field | Observed range | Notes |
|-------|---------------|-------|
| Counter dollars | 25000-120000 | Cash amounts the AI counters with |
| Counter days | 1-8 | Day amounts the AI counters with |
| Probability values | 10-80 | Appear to be percentage weights |

**Pattern:** The dollar counter-values are 4 ascending amounts (e.g., 25000/50000/75000/100000). The day values are also ascending (e.g., 2/4/6/8). The probability rows have a repeating `low high low high...` pattern (e.g., `10 40 10 40 10 30 10 30`), suggesting alternating "try this amount / if rejected try next" logic.

---

## 4. Prestige

```
Prestige:
Mission Type/Entrance/# MAPS/Success1/Success2/WIA/MIA/KIA: <type> <entrance> <maps> <s1> <s2> <wia> <mia> <kia>
```

Single line, 8 integers:

| Field | Type | Observed values | Meaning |
|-------|------|----------------|---------|
| Mission Type | `u8` | 1, 2, 3 | Mission category (1=rescue, 2=retrieval/assassination, 3=special/story) |
| Entrance | `u8` | 1, 2 | Map entrance point index |
| # MAPS | `u8` | 1 | Number of map segments (always 1 in observed data) |
| Success1 | `i16` | 20-240 | Prestige gained on mission success |
| Success2 | `i16` | 0 | Secondary success modifier (always 0 in observed data) |
| WIA | `i8` | -1 to -9 | Prestige penalty per wounded-in-action |
| MIA | `i8` | -2 to -18 | Prestige penalty per missing-in-action |
| KIA | `i8` | -2 to -18 | Prestige penalty per killed-in-action |

**Variation:** Later missions have much higher Success1 values (MSSN16: 240) and harsher penalties (WIA -9, MIA/KIA -18). MIA and KIA penalties are always equal to each other. WIA is always half (or close to half) of KIA.

---

## 5. Intelligence

```
Intelligence:
Information Consultants:  <cost> <per_item>
Intelligence, Inc:        <cost> <per_item>
Global Intelligence:      <cost> <per_item>
Men/Exp/FirePower/Success/Casualties/Scene Type: <men> <exp> <fp> <suc> <cas> <scene>
```

Three tiers of purchasable intel, each with a base cost and a per-item cost:

| Tier | Role | Cost range | Per-item range |
|------|------|-----------|---------------|
| Information Consultants | Cheapest / least detail | 40000-70000 | 1500-5000 |
| Intelligence, Inc | Mid-tier | 70000-100000 | 2500-7500 |
| Global Intelligence | Most expensive / best detail | 100000-140000 | 3500-10000 |

The `Men/Exp/FirePower/Success/Casualties/Scene Type` line provides the actual intel content (what the player receives if they purchase):

| Field | Type | Meaning |
|-------|------|---------|
| Men | `u8` | Enemy headcount (approximate) |
| Exp | `u8` | Enemy experience level indicator |
| FirePower | `u8` | Enemy firepower rating |
| Success | `u8` | Estimated success chance (percent) |
| Casualties | `u8` | Expected casualty level |
| Scene Type | `u8` | Terrain/biome type (0=desert, 1=jungle) |

**Special case:** MSSN16 has all zeros for intel costs and content -- intel unavailable (classified mission).

Following this is a standalone line:

```
Attachments: <count>
```

Number of intel attachments (0-3). Likely controls how many detail pages the intel report has.

---

## 6. Enemy Ratings Chart

```
Enemy Ratings Chart:
Number:  <total_entries>
NPCs:    <npc_count>
Rating  DPR  EXP  STR  AGL  WIL  WSK  HHC  TCH  ENC  APS  There  Type
<row per enemy>
```

Header declares `Number` (total combatant entries in the table, NOT including NPCs appended below) and `NPCs` (number of NPC entries appended after combatants).

**Important:** The actual row count is `Number + NPCs` for missions with Type 3/4/7 NPCs appended. MSSN01 has `Number: 10, NPCs: 1` with 11 rows total. MSSN08 has `Number: 15, NPCs: 3` with 18 rows total.

### Stat columns

| Column | Type | Range | Meaning |
|--------|------|-------|---------|
| Rating | `u8` | 9-86 | Overall enemy rating / level |
| DPR | `u8` | 113-186 | Damage Power Rating (base hit damage) |
| EXP | `u8` | 5-94 | Experience points (awarded to player on kill) |
| STR | `u8` | 5-91 | Strength |
| AGL | `u8` | 13-90 | Agility |
| WIL | `u8` | 0-95 | Willpower (0 for Type 7 entries) |
| WSK | `u8` | 0-85 | Weapon Skill (0 for Type 7 entries) |
| HHC | `u8` | 0-83 | Hand-to-Hand Combat skill (0 for Type 7 entries) |
| TCH | `u8` | 0-95 | Technical skill |
| ENC | `u16` | 0-375 | Encumbrance capacity (lbs x some factor); values: 0, 225, 300, 375 |
| APS | `u8` | 30-72 | Action Points per turn |
| There | `u8` | 20-100 | Spawn probability (percent chance this enemy appears) |
| Type | `u8` | 2, 3, 4, 7 | Unit type (see below) |

### Type values

| Type | Meaning | Notes |
|------|---------|-------|
| 2 | Regular enemy combatant | Most common |
| 3 | NPC (non-hostile) | Appended after enemy rows; often has same stats as a combatant but different AI |
| 4 | NPC variant (escort target?) | Seen in MSSN05 |
| 7 | Special unit (vehicle/dog) | WIL, WSK, HHC, TCH, ENC are 0; high APS (70-72) |

**Pattern:** Type 7 entries always have `WIL=0, WSK=0, HHC=0, TCH=0, ENC=0` and very high APS. These likely represent dogs or vehicles. Type 3/4 NPCs are appended after the main enemy list and have weapon chart entries of `-1 -1 0 0 -1 -1` (unarmed).

**Variation across missions:** Enemy stats scale dramatically. MSSN01 ratings range 9-43; MSSN16 ranges 49-86. APS also increases (30-40 early, 38-48 late).

---

## 7. Enemy Weapons Chart

```
Enemy Weapons Chart:   Weapon 1/Weapon 2/Ammo 1/Ammo 2/Weapon 3
<row per enemy, matching order of Ratings Chart>
```

One row per entry in the Ratings Chart (including NPC rows). Six whitespace-delimited integers per row:

| Column | Type | Meaning |
|--------|------|---------|
| Weapon 1 | `i8` | Primary weapon index (-1 = none) |
| Weapon 2 | `i8` | Secondary weapon index (-1 = none) |
| Ammo 1 | `u8` | Ammo magazine count for weapon 1 |
| Ammo 2 | `u8` | Ammo magazine count for weapon 2 |
| Weapon 3 | `i8` | Tertiary item index (-1 = none); could be grenade/sidearm |
| Column 6 | `i8` | Additional equipment/item index (-1 = none) |

**Note:** The header says `Weapon 1/Weapon 2/Ammo 1/Ammo 2/Weapon 3` (5 fields) but there are consistently **6** values per row. The 6th column is unlabeled -- likely an equipment slot (medkit, armor, tool) or a secondary ammo type.

**Pattern:** NPC and Type 7 rows typically have `-1 -1 0 0 -1 -1` (unarmed) or `57 -1 0 0 -1 -1` (weapon index 57 only, possibly a special/scripted weapon for dogs or vehicles). Weapon indices reference the weapon definitions table (likely `WEAPONS.DAT` or similar).

---

## 8. PreLoaded Equipment

```
PreLoaded Equipment (Weapons/Ammo/Equipment): <weapons> <ammo> <equipment>
```

Three integers specifying counts of pre-loaded gear given to the player at mission start. Always `0 0 0` in all observed missions -- equipment is player-supplied.

---

## 9. Recommended Equipment

```
Recommended Equipment (Weapons/Ammo/Equipment): <weapons> <ammo> <equipment>
[Equipment Amount/Number:  <item_id> <count>]
```

Three integers. When equipment value is non-zero (e.g., `0 0 1`), followed by an additional line:

```
Equip Amount/Number:   <item_id> <count>
```

or

```
Equipment Amount/Number:  <item_id> <count>
```

**Note:** The label varies between files (`Equip Amount/Number` in MSSN01, `Equipment Amount/Number` in MSSN08/16). Parser should accept both.

| Field | Observed values | Meaning |
|-------|----------------|---------|
| item_id | 5, 49 | Equipment item index to recommend |
| count | 1 | Quantity recommended |

**Variation:** Most missions have `0 0 0` with no follow-up line. Missions with recommended equipment (MSSN01, 08, 16) suggest specific items for the biome.

---

## 10. Start Time

```
Start Time: <hour> <minute>
```

Two integers: mission start time in 24-hour format.

| Mission | Time | Context |
|---------|------|---------|
| MSSN01 | 10:00 | Mid-morning |
| MSSN02 | 9:45 | Morning |
| MSSN05 | 6:00 | Dawn |
| MSSN08 | 16:12 | Afternoon |
| MSSN16 | 8:15 | Morning |

---

## 11. Weather Table

```
Weather Table:
Clear/Foggy/OverCast/LtRain/HvyRain/Storm: <c> <f> <o> <lr> <hr> <s>
```

Six integers representing probability weights for weather conditions. **These must sum to 100.**

| Condition | MSSN01 | MSSN02 | MSSN05 | MSSN08 | MSSN16 |
|-----------|--------|--------|--------|--------|--------|
| Clear | 10 | 90 | 20 | 90 | 70 |
| Foggy | 10 | 0 | 50 | 0 | 0 |
| OverCast | 50 | 0 | 10 | 10 | 15 |
| LtRain | 30 | 0 | 10 | 0 | 5 |
| HvyRain | 0 | 0 | 10 | 0 | 5 |
| Storm | 0 | 10 | 0 | 0 | 5 |

**Pattern:** Desert missions (MSSN02) favor clear/storm. Jungle missions have more rain/fog/overcast. Weather affects visibility and combat modifiers.

---

## 12. Travel Table

```
Travel Table:
Cost1/Cost2/Cost3/Days1/Days2/Days3: <c1> <c2> <c3> <d1> <d2> <d3>
```

Three travel options (cheap/medium/expensive) with associated costs and travel durations:

| Field | Meaning | Typical range |
|-------|---------|---------------|
| Cost1 | Cheapest travel option cost | 15000-50000 |
| Cost2 | Mid-tier travel cost | 25000-60000 |
| Cost3 | Premium travel cost | 40000-80000 |
| Days1 | Travel time for cheapest (days) | 4-5 |
| Days2 | Travel time for mid-tier | 3-4 |
| Days3 | Travel time for premium | 1-3 |

**Pattern:** Higher cost = fewer travel days. Days consumed reduce time before the deadline. Later missions have higher travel costs.

---

## 13. Special Turns / Type / Item / Damage

```
Special Turns (# Turns to Complete Action): <turns>
Special Type: <type>
Special Item: <item_id>

Special Damage: <damage_type>
[<damage_message>]
```

| Field | Type | Observed values | Meaning |
|-------|------|----------------|---------|
| Special Turns | `u8` | 0 | Number of turns for a special action (0 = none) |
| Special Type | `u8` | 0 | Special event type (0 = none) |
| Special Item | `u8` | 0, 17 | Item ID involved in special event (0 = none, 17 = radiation source in MSSN02) |
| Special Damage | `u8` | 0, 2 | Environmental damage type (0 = none, 2 = environmental hazard) |

When `Special Damage` is non-zero, the next line contains a `printf`-style damage message string with `%s` as the merc name placeholder:

| Mission | Damage message |
|---------|---------------|
| MSSN02 | `%s has received radiation burns!` |
| MSSN05, 08, 16 | `%s has been bitten by a snake!` |

**Pattern:** MSSN01 has `Special Damage: 0` with no message line. All jungle missions have snake bites. MSSN02 (desert/nuclear) has radiation. The damage message is the last content line before the `~` terminator.

---

## 14. File Terminator

```
~
```

Single tilde on the last line. This is the EOF sentinel for the parser.

---

## Cross-Mission Comparison Summary

### Structural constants (identical across all files)
- Section ordering is always the same (Animation -> Contract -> Negotiation -> Prestige -> Intel -> Enemy Ratings -> Enemy Weapons -> PreLoaded -> Recommended -> Start Time -> Weather -> Travel -> Special -> `~`)
- PreLoaded Equipment is always `0 0 0`
- Success2 in Prestige is always `0`
- `# MAPS` is always `1`
- Number of negotiation tiers is always 4
- Counter-offer table is always 8 values wide
- Weather probabilities always sum to 100

### Fields that vary per mission
- **Animation sprites**: biome-dependent (jungle vs desert vs cambo)
- **Contract amounts**: scale with mission progression (324k-1.1M+ advance)
- **Negotiation**: may be zeroed out for non-negotiable contracts (MSSN16)
- **Prestige rewards/penalties**: scale with mission difficulty
- **Intel costs**: scale with mission; may be zeroed for classified missions
- **Enemy count**: 9-15 combatants
- **Enemy stats**: scale dramatically with mission progression
- **NPC count**: 0-3
- **Unit types**: varied (2=enemy, 3=NPC, 4=NPC variant, 7=vehicle/animal)
- **Weather distribution**: biome-dependent
- **Travel costs/times**: vary per mission location
- **Special damage**: biome-dependent environmental hazards
- **Start time**: varies per mission narrative

---

## Parser Notes

1. **Line endings**: Windows `\r\n`. Strip `\r` before parsing.
2. **Whitespace**: Fields are tab-and-space delimited inconsistently. Use `split_whitespace()`.
3. **Labels**: Section headers contain the field names as labels. Parse by recognizing the label prefix, then extract values after the colon.
4. **Label inconsistency**: `Equip Amount/Number` vs `Equipment Amount/Number` and `# MAPS` vs `#Maps`. Parser must be lenient.
5. **Trailing whitespace**: Many lines have trailing spaces/tabs. Trim before parsing.
6. **Null strings**: The literal string `null` (lowercase) indicates an unused slot.
7. **Enemy table row count**: Total rows = `Number` + `NPCs`. The `Number` field counts only hostile combatants; NPC rows are appended after.
8. **Weapon chart alignment**: Rows in the weapon chart correspond 1:1 with rows in the ratings chart (same order, same count).
9. **Optional lines**: The `Equipment Amount/Number` line only appears when recommended equipment count > 0. The damage message line only appears when `Special Damage` > 0.
10. **Free-text fields**: Contract `From`, `Terms`, and `Bonus` are free-form strings. `From` may contain commas and special characters.
