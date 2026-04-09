# FORMAT: *.COR (Animation Sequence Definitions)

## Overview

`.COR` files are plaintext, line-oriented data files that define animation sequences for entities in *Wages of War*. Each `.COR` file is paired with a matching `.DAT` file (binary sprite sheet) and a `.ADD` file (additional sprite data). The `.COR` acts as an index into its companion `.DAT`, describing what animations exist, how many frames each has, and how they map to actions, weapons, directions, and postures.

Uses Windows-style line endings (CR/LF).

## File Structure

```
<dat_filename>                            # e.g. "JUNGSLD.dat"
<add_filename>                            # e.g. "JUNGSLD.add"
<unknown_field>                           # integer, usually 1 (possibly sprite scale or layer count)
[NrAnimations-action-weapon-direction-nrframes]   # field-name legend (literal text)
<total_animation_count>                   # e.g. 1120
[<seq_number>. <human_readable_label>     # comment line describing the animation
<f1>,<f2>,<f3>,<f4>,<f5>,<f6>,<f7>,<f8>,<f9>    # data line (9 comma-separated integers)
...                                       # repeat for each animation
[END]                                     # terminator
```

### Header

| Line | Content | Example |
|------|---------|---------|
| 1 | DAT filename (companion binary sprite data) | `JUNGSLD.dat` |
| 2 | ADD filename (companion additional data) | `JUNGSLD.add` |
| 3 | Unknown integer (always observed as `1` or `4`) | `1` |
| 4 | Field legend (literal bracket-delimited text) | `[NrAnimations-action-weapon-direction-nrframes]` |
| 5 | Total animation count | `1120` |

### Animation Entries

Each animation is two lines:

1. **Comment line:** `[<1-based_index>. <label>` -- human-readable description. The closing bracket is omitted. Labels encode weapon type, action, and direction (e.g., `rifle walk SW`, `dog attack NE`, `boat move NW`).

2. **Data line:** Nine comma-separated integers. No spaces around commas.

### Terminator

The file ends with `[END]` on its own line, optionally followed by a blank line.

## Data Line Fields

```
f1, f2, f3, f4, f5, f6, f7, f8, f9
```

| Position | Field | Type | Description |
|----------|-------|------|-------------|
| 1 | `mirror_flag` | Integer | `1` = normal sprite. `2` = horizontally mirrored from another direction (typically W mirrors E, SW mirrors SE, etc.). Avoids storing duplicate sprite data. |
| 2 | `frame_offset` | Integer | Offset or base index into the sprite sheet for this animation's frames. For soldier entities, this often correlates with posture transitions (see below). Values observed: 0, 1, 7, 8, 15, 16. |
| 3 | `action_id` | Integer | Identifies the action/animation type. Meaning is entity-specific. See action tables below. |
| 4 | `weapon_id` | Integer | For soldiers: weapon class (0-6). For animals/vehicles: a context-specific parameter (direction count, variant, etc.). |
| 5 | `direction` | Integer | Facing direction, 0-7 for 8-direction entities. See direction table. |
| 6 | `nr_frames` | Integer | Number of animation frames in this sequence. `0` = single static frame or placeholder. |
| 7 | `sound_id` | Integer | Sound effect ID to play during this animation. `0` = no sound. Examples: `10` (footsteps), `107` (door kick), `92`/`93` (animal sounds), `114` (smoke). |
| 8 | `unknown_1` | Integer | Always `0` in all observed files. Reserved/unused. |
| 9 | `unknown_2` | Integer | Usually `1`. Occasionally `0` (seen in a few uzi entries) or other values (e.g., `3` for multi-part office animations, `2` for two-phase animations, `5` for rolodex, `15` for fax, `45` for phone call). May indicate playback speed, loop count, or sub-animation grouping. |

## Direction Encoding

Eight isometric directions, numbered clockwise from South:

| Value | Direction | Compass |
|-------|-----------|---------|
| 0 | S | South (toward camera, bottom of screen) |
| 1 | SW | Southwest |
| 2 | W | West |
| 3 | NW | Northwest |
| 4 | N | North (away from camera, top of screen) |
| 5 | NE | Northeast |
| 6 | E | East |
| 7 | SE | Southeast |

Not all entities use all 8 directions. `BOAT01` defines only NW (1) and NE (5). Non-directional entities (e.g., MISC effects) always use direction `0`.

## Mirror Flag (f1)

When `f1 = 2`, the engine mirrors the sprite from the opposite direction rather than storing separate artwork. Typically:
- W (dir=2) mirrors E (dir=6)
- Carry Wounded W/E also use mirror flag `2`

This halves the sprite data needed for symmetric left/right views.

## Weapon IDs (Soldier Entities)

Soldier `.COR` files (e.g., `JUNGSLD.COR`) define animations for multiple weapon classes. The weapon ID in field 4 determines which weapon's sprite set to use:

| ID | Weapon Class | Example Entries |
|----|-------------|-----------------|
| 0 | Rifle | `rifle walk S`, `rifle Fire Weapon S` |
| 1 | Crossbow | `crossbow walk S` |
| 2 | Pistol | `pistol walk S` |
| 3 | Shotgun | `shotgun walk S` |
| 4 | Heavy / BigMacGun | `BigMacGun walk S` |
| 5 | SMG / Uzi | `uzi walk S` |
| 6 | No Weapon / Melee | `no-weapon walk S`, `Knife Slash S`, `throw S` |

Weapon ID `60` is used for the special `Carry Wounded` action (entries 1113-1120 in JUNGSLD).

## Action IDs (Soldier Entities)

Selected action IDs observed in `JUNGSLD.COR`. These are entity-specific -- different .COR files may use different numbering:

| ID | Action | Notes |
|----|--------|-------|
| 0 | Walk | 8 frames typical, with footstep sound |
| 1 | Run | 8 frames, faster footstep sound |
| 2 | Stand to Kneel | Posture transition |
| 3 | Stand to Prone | Posture transition |
| 4 | Kneel to Stand | Posture transition |
| 5 | Kneel to Prone | Posture transition |
| 6 | Prone to Stand | Posture transition |
| 7 | Prone to Kneel | Posture transition |
| 11 | Throw (grenade) | |
| 23 | Kick Door Open | With door-kick sound (107) |
| 25 | Die (animal) | Used in GUARDDOG |
| 26 | Crawl | Prone movement |
| 29-30 | Weapon-specific actions | Vary per weapon class |
| 31-36 | Death animations | Forward #1/#2, backward, etc. |
| 38/40 | Posture death transitions | Kneel/prone death |
| 41-44 | Melee attacks | Knife slash, knife stab, punch |
| 45 | Rest Sequence (standing) | Idle animation |
| 46 | Rest Sequence (kneeling) | Idle animation |
| 50 | Ready Weapon | Raise weapon to fire |
| 51 | Fire Weapon | Muzzle flash frame |
| 52 | Unready Weapon | Lower weapon |
| 53-56 | Kneel/Prone fire variants | Ready/Fire/Unready while kneeling or prone |
| 58 | Kneel Death | |
| 59 | Prone Death | |
| 61 | Attack (animal) | Used in GUARDDOG |
| 99 | Destruction | Used for vehicle/object destruction |

## Frame Offset / Posture Field (f2)

The second field appears to encode posture state or sprite-sheet region:

| Value | Context | Meaning |
|-------|---------|---------|
| 0 | Most actions | Default / standing posture |
| 1 | Fire weapon sequences | Weapon-raised sprite set |
| 7 | Prone death (one entry) | Possibly error or special case |
| 8 | Posture transitions, kneel/prone death | Transition sprite set |
| 15 | Death animations, blast/bomb reactions | Death/hit-reaction sprite set |
| 16 | Animal attack | Attack sprite set |

## Entity Types

### Soldiers (JUNGSLD.COR, etc.)

- 1120 animations typical
- Full weapon x action x direction matrix
- 7 weapon classes x ~20 actions x 8 directions = ~1120 entries
- Weapon ID in field 4, direction in field 5

### Animals (GUARDDOG.COR)

- 32 animations: walk (8 dirs) + run (8) + attack (8) + die (8)
- Field 4 = `8` (possibly indicating 8-direction entity)
- Direction in field 5

### Vehicles (BOAT01.COR)

- 6 animations: idle/move/destroy x 2 directions
- Minimal direction coverage (NW + NE only, mirror handles the rest)
- Field 4 = `6` for most entries

### Misc Effects (MISC.COR)

- 204 entries for UI elements, projectiles, explosions, maps, weather, etc.
- No directional component (direction always 0)
- Many placeholder/bogus entries (reserved slots)
- Includes: dialog boxes, bullet animations, grenade trajectories, explosions, map thumbnails, smoke, muzzle flash, missiles

### Office Sprites (OFFCSPR.COR)

- 17 entries for office/base screen animations
- Field 9 varies (1-45), suggesting timing or loop parameters
- Includes: coffee steam, package delivery, fax machine, file cabinet, fan, rolodex, phone, adding machine, magazines, pizza delivery, training, casualty

## Relationship to .DAT Files

Each `.COR` references a `.DAT` file on its first line. The `.DAT` file is **binary** -- it contains the actual sprite pixel data (RLE-compressed or raw indexed-color bitmaps). The `.COR` serves as the frame index/directory for the `.DAT`.

The `.ADD` file (line 2 of the header) contains additional sprite overlays or supplementary data.

The animation engine uses the `.COR` to look up:
1. Which animation to play (by action + weapon + direction)
2. How many frames it has
3. Where in the `.DAT` sprite sheet to find those frames
4. Whether to mirror the sprite horizontally
5. What sound effect to trigger

## Example: GUARDDOG.COR (Complete)

```
GUARDDOG.dat
GUARDDOG.add
1
[NrAnimations-action-weapon-direction-nrframes]
32
[1. dog walk S
1,0,0,8,0,16,0,0,1
[2. dog walk SW
1,0,0,8,1,16,0,0,1
[3. dog walk W
2,0,0,8,2,16,0,0,1          <-- mirror=2, mirrors E direction
[4. dog walk NW
1,0,0,8,3,16,0,0,1
...
[17. dog attack S
1,16,61,8,0,16,92,0,1       <-- frame_offset=16, action=61, sound=92
...
[25. dog die S
1,15,25,8,0,15,95,0,1       <-- frame_offset=15, action=25, sound=95
...
[END]
```

## Example: BOAT01.COR (Complete)

```
BOAT01.dat
BOAT01.add
4                             <-- differs from the usual "1"
[NrAnimations-action-weapon-direction-nrframes]
6
[1. boat idle NW
1,0,0,6,1,0,0,0,1            <-- 0 frames (static image)
[2. boat move NW
1,0,1,6,1,1,0,0,1            <-- 1 frame of movement
[3. boat dest NW
1,0,2,6,1,0,0,0,1            <-- destruction, static
[4. boat idle NE
1,0,0,6,5,0,0,0,1
[5. boat move NE
1,0,1,6,5,1,0,0,1
[6. boat dest NE
1,0,99,0,0,0,0,0,1           <-- action=99 (destruction), fields zeroed
[END]
```

## Parser Notes

- All integer fields. No floating point.
- Comment lines start with `[` but have no closing bracket (except `[END]` and the field legend).
- Strip `\r` (CR) before parsing -- files use Windows line endings.
- The total animation count on line 5 must match the number of data lines (not comment lines).
- Empty or whitespace-only trailing lines may appear after `[END]`.
- Field values are never negative in observed data.
- The field legend line `[NrAnimations-action-weapon-direction-nrframes]` is always present and always identical across all files. It hints at the key fields but does not enumerate all nine.
