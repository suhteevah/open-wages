# FORMAT_CONTRACTS.md — Contract, Briefing & Mission File Formats

## Overview

Each mission in Wages of War (numbered 01–16) is defined by a cluster of related files:

| File Pattern | Format | Purpose |
|---|---|---|
| `MSSN##.DAT` | Plaintext, labeled fields | Master mission definition (mechanics, enemies, contract terms, weather, etc.) |
| `CONTR##.DAT` | Plaintext prose | Contract description — the job offer narrative shown to the player |
| `CONTR##.WRI` | Windows Write (binary rich text) | Same contract text as `.DAT`, formatted for the in-game rich text viewer |
| `BRIEF##.DAT` | Plaintext prose | Pre-mission briefing — tactical/operational details |
| `BRIEF##A.DAT` | Plaintext prose | Briefing variant A (higher intelligence tier) |
| `BRIEF##B.DAT` | Plaintext prose | Briefing variant B (highest intelligence tier) |
| `BRIEF##.WRI` | Windows Write (binary rich text) | Formatted briefing for the in-game viewer |
| `BRIEF##A.WRI` | Windows Write (binary rich text) | Formatted briefing variant A |
| `BRIEF##B.WRI` | Windows Write (binary rich text) | Formatted briefing variant B |

**File coverage:** All 16 missions have `MSSN##.DAT` files. The `.WRI` files exist for the full 01–16 range (`CONTR01–16.WRI`, `BRIEF01A–16A.WRI`, `BRIEF01B–16B.WRI`). The `.DAT` plaintext versions only exist for missions 01–03 — likely an incomplete conversion or the `.WRI` files are the canonical format and `.DAT` files are drafts/exports.

---

## CONTR##.DAT — Contract Description

### Format

Pure plaintext prose with DOS line endings (`\r\n`). No headers, no structured fields, no delimiters. The entire file is a single block of narrative text describing the job offer.

### Content Pattern

Each contract describes:
- **The client** — who is hiring MERCS, Inc.
- **The situation** — what happened, background context
- **The objective** — what the player's team must accomplish
- **The stakes** — why it matters, consequences of failure

### Example (CONTR01.DAT, abridged)

```
Richarde LeClure, the President and CEO of Armes Developpement
International, a leader in fourth generation Laser Weapons research,
is in need of a professional organization to find and retrieve his
daughter, Lizza Montague LeClure. Miss LeClure was kidnapped on
New Years Eve, December 31, 2000...
```

### Encoding Notes

- DOS line endings (`\r\n`, visible as `^M` in Unix tools)
- Windows-1252 encoding: curly apostrophes appear as `\x92` (`M-^R` in `cat -v`), e.g. "LeClure's" rendered as `LeClure\x92s`
- No trailing delimiter or EOF marker

---

## BRIEF##.DAT / BRIEF##A.DAT / BRIEF##B.DAT — Mission Briefings

### Format

Pure plaintext prose with DOS line endings. Same encoding as contract files.

### Intelligence Tier System

The three briefing variants (base, A, B) correspond to intelligence quality tiers purchased by the player. This is confirmed by the `MSSN##.DAT` intelligence section, which defines three intelligence agencies at different price points:

```
Intelligence:
Information Consultants:  40000  5000    ← cheapest tier
Intelligence, Inc:        70000  7500    ← middle tier
Global Intelligence:     100000 10000    ← most expensive tier
```

The briefing variants differ in **small but tactically significant details** — specifically the quality of intelligence sources cited:

| Variant | Intelligence Source Wording | Map Source Wording |
|---|---|---|
| `BRIEF##.DAT` (base) | "Satellite recon photos" / "intercepted internal communiqué" | "obtained from public records" |
| `BRIEF##A.DAT` (tier A) | Same as base (identical to base in missions 01–02) | "obtained from public records" |
| `BRIEF##B.DAT` (tier B) | "Our operatives" / "An informant" | "obtained from close observation" / "obtained from public records" |

The differences are subtle — a single phrase changed per variant — suggesting the intelligence tier affects the **source reliability** described in the text rather than revealing fundamentally different information. The base and A variants are often identical; B consistently uses wording that implies better/closer intelligence sources.

### Content Pattern

Each briefing describes:
- **Location** — where the target/objective is
- **Tactical situation** — enemy disposition, guard schedules, terrain
- **Insertion plan** — how the team gets to the operational area
- **Weather/conditions** — expected weather, recommended camouflage
- **Time constraints** — mission timing, deadlines

### Example (BRIEF01.DAT, abridged)

```
Lizza Montague LeClure has been located in Columbia, South America
at the hacienda of one Silvera Pina Salvatore... The best time for
your team to enter the operational area should be 10:00am local time...
The weather is expected to be excellent... jungle camo is highly recommended.
```

---

## .WRI Files — Windows Write Format

The `.WRI` files are **binary** Microsoft Windows Write documents (the precursor to WordPad). They contain:

- A binary header (~128 bytes) with format metadata
- The same narrative text as the corresponding `.DAT` file
- Font information (Arial) and formatting data in a trailing binary section
- A partial duplicate of text near the end (appears to be a Write format artifact)

The `.WRI` format is the canonical display format used by the game engine's in-game text viewer. The `.DAT` plaintext files are likely either:
1. Source drafts that were converted to `.WRI` for the game
2. Exports for easier editing that were not maintained for all missions

### Binary Structure (observed)

```
Offset  Content
0x00    Magic bytes: 31 BE 00 00 00 AB (Windows Write signature)
0x08    Varies per file
~0x80   Start of plaintext body (null-padded to alignment)
...     Body text with \r\n line endings
...     Trailing binary: font table, formatting runs
        Font name embedded: "Arial" (null-terminated, padded)
```

### Parser Recommendation

For the reimplementation, parse the `.WRI` files to extract the plaintext body:
1. Skip the 128-byte header
2. Read until encountering a run of null bytes or the font table marker
3. Strip `\r` from line endings
4. Handle Windows-1252 encoding (smart quotes, accented characters like `é`)

Alternatively, if `.DAT` equivalents exist for all missions, prefer those. Currently only missions 01–03 have `.DAT` versions, so a `.WRI` parser is required for missions 04–16.

---

## Relationship: MSSN##.DAT ↔ CONTR##/BRIEF## Files

The `MSSN##.DAT` file is the **master mission definition** containing all structured/mechanical data. The contract and briefing files provide the **narrative layer**. They are linked by their shared mission number (`##`).

### What lives where

| Data | Location |
|---|---|
| Contract narrative (job offer text) | `CONTR##.DAT` / `CONTR##.WRI` |
| Briefing narrative (tactical details) | `BRIEF##.DAT` / `BRIEF##A.DAT` / `BRIEF##B.DAT` (and `.WRI` equivalents) |
| Client name (structured) | `MSSN##.DAT` → `Contract: From:` field |
| Contract terms (structured) | `MSSN##.DAT` → `Terms:` and `Bonus:` fields |
| Payment amounts | `MSSN##.DAT` → `Advance/Bonus/Deadline:` line |
| Negotiation mechanics | `MSSN##.DAT` → `Contract Negotiation:` section |
| Intelligence agency costs | `MSSN##.DAT` → `Intelligence:` section |
| Enemy composition | `MSSN##.DAT` → `Enemy Ratings Chart:` and `Enemy Weapons Chart:` |
| Weather probabilities | `MSSN##.DAT` → `Weather Table:` |
| Travel options/costs | `MSSN##.DAT` → `Travel Table:` |
| Start time | `MSSN##.DAT` → `Start Time:` |
| Animation/sprite files | `MSSN##.DAT` → `Animation Files:` section |
| Prestige rewards/penalties | `MSSN##.DAT` → `Prestige:` section |

### Contract Section in MSSN##.DAT (reference)

The MSSN file embeds a structured contract summary:

```
Contract:
Date:    7 2001                              ← day-of-year and year
From:
Richarde LeClure, President and CEO, ...     ← client name/title

Terms:
Find and return alive ... his daughter ...   ← objective summary
Bonus:
... must be returned in good physical ...    ← bonus condition
Advance/Bonus/Deadline:  324000 535000 20 2001  ← base advance $, bonus $, deadline day-of-year, year
```

This structured data drives the contract UI; the `CONTR##` files provide the flavor text displayed alongside it.

### Contract Negotiation in MSSN##.DAT

```
Contract Negotiation:
Advance:  349000 374000 399000 424000    ← 4 counter-offer tiers for advance payment
Bonus:    560000 585000 610000 635000    ← 4 counter-offer tiers for bonus
Deadline:     22     24     26     28    ← 4 counter-offer tiers for deadline (day-of-year)
Chance:       76     52     28     04    ← % chance client accepts each tier

Counter:  25000 50000 75000 100000  2  4  6  8   ← player's counter-offer increments ($ and days)
Advance:     10    40    10     40 10 30 10 30   ← % chance accepted per increment
Bonus:       10    80    10     70 10 50 10 40   ← % chance accepted per increment
Deadline:    10    80    10     70 10 60 10 50   ← % chance accepted per increment
```

Missions with no negotiation (e.g., government contracts like mission 03) have all zeros in these fields.

---

## Mission Number → Narrative Summary

Based on the files examined:

| Mission | Client | Objective | Location |
|---|---|---|---|
| 01 | Richarde LeClure / Armes Developpement Internationale | Rescue kidnapped daughter Lizza LeClure | Colombia, South America |
| 02 | Symmetry AI, Inc. | Recover briefcase from crashed jet | Dead Zone (Jordan/Iran border) |
| 03 | Egyptian Government | Destroy stolen Stinger missiles | Libya (coast near Sirte) |

---

## Encoding Reference

| Byte (hex) | `cat -v` | Character |
|---|---|---|
| `0x92` | `M-^R` or `M-F` | Right single quote (') — Windows-1252 |
| `0xE9` | `M-i` | Latin small letter e with acute (é) |
| `0x0D 0x0A` | `^M` + newline | DOS line ending (CR+LF) |

---

## Parser Implementation Notes

1. **Priority:** Parse `.WRI` files since they cover all 16 missions. Fall back to `.DAT` for testing/validation.
2. **Intelligence tiers:** Load the correct briefing variant (base/A/B) based on which intelligence agency the player purchased. The MSSN file's intelligence section determines pricing; the briefing suffix determines which text to show.
3. **Text display:** Strip `\r`, convert Windows-1252 to UTF-8, render in the in-game text panel.
4. **The `~` terminator:** MSSN##.DAT files end with a `~` on its own line. Contract and briefing `.DAT` files do not use this terminator.
5. **Null handling in .WRI:** The plaintext body in `.WRI` files may be followed by null padding bytes before the binary formatting section. Stop reading text at the first null byte.
