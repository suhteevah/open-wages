# FORMAT_AI_NODES.md -- AI Pathfinding, Movement Orders & Store Inventory

Covers four per-mission file families: `AINODEnn.DAT`, `NODESnn.DAT`, `MOVESnn.DAT`, `LOCKnn.DAT`.

All files are plaintext, Windows line endings (CR+LF), tab-delimited where applicable.

---

## 1. AINODEnn.DAT / NODESnn.DAT -- AI Pathfinding Node Graph

### Purpose

These files define a **waypoint graph** for AI pathfinding on a mission map. Each node represents a navigable position (tile) and lists which other nodes it connects to in the four cardinal directions. The AI uses this graph for high-level route planning rather than per-tile A*.

### AINODE vs NODES: Two Graph Variants

`AINODEnn.DAT` and `NODESnn.DAT` share the **exact same format** but can contain **different data** for the same mission. Observed differences between AINODE01 and NODES01:

- Different tile IDs at certain node indices (e.g., node 54: tile 5918 vs 5198)
- Different grid types at certain nodes (e.g., node 77: grid 1 vs 4)
- Different neighbor connections (some edges present in one but not the other)

**Working hypothesis:** One graph is for foot-mobile AI (infantry), the other for vehicle AI, OR one is the "planning" graph and the other is the "execution" graph with runtime modifications. For some missions (e.g., mission 2) the two files are byte-identical.

### File Structure

```
# AI NODE LIST -- MISSION #<N>
                                        <- blank line
<NodeCount> ; Total # Of Nodes In The File
                                        <- blank line
;Tile	Grid	North	East	South	West	Inside
;-----------------------------------------------------
                                        <- blank line
<NodeData lines, 0-indexed>
```

### Header

| Line | Content |
|------|---------|
| 1 | `# AI NODE LIST -- MISSION #<N>` -- comment identifying the mission |
| 2 | blank |
| 3 | `<integer> ; Total # Of Nodes In The File` -- total node count (0-indexed, so valid indices are 0..N-1) |
| 4 | blank |
| 5 | `;Tile	Grid	North	East	South	West	Inside` -- column header (comment) |
| 6 | `;-----------------------------------------------------` -- separator (comment) |
| 7 | blank |

### Comment Format

- Lines beginning with `;` are comments.
- Every 10th node is followed by a comment line: `; 10`, `; 20`, `; 30`, etc. These serve as visual index markers for the human editor and should be skipped by parsers.
- The header contains two comment lines (column names and separator).

### Node Data (one line per node)

Fields are **tab-separated**. Implicit 0-based index determined by line position (first data line = node 0).

| Field | Type | Description |
|-------|------|-------------|
| Tile | u16 | Tile ID on the map grid. This is a **linear index** into the map's tile array. |
| Grid | u8 (1-4) | Grid quadrant/sub-position within the tile. Values observed: 1, 2, 3, 4. Likely encodes which corner or sub-cell of the tile the waypoint occupies. |
| North | i16 | Index of the neighbor node to the north, or **-1** if no connection. |
| East | i16 | Index of the neighbor node to the east, or **-1** if no connection. |
| South | i16 | Index of the neighbor node to the south, or **-1** if no connection. |
| West | i16 | Index of the neighbor node to the west, or **-1** if no connection. |
| Inside | u8 | Location flag. See below. |

### Inside Flag Values

| Value | Meaning |
|-------|---------|
| 0 | Outdoor / open area |
| 1 | Inside a building or enclosed space |
| 4 | Special location (observed at map-edge nodes, vehicle spawn points, and key strategic positions -- possibly extraction zones, reinforcement entry points, or objectives) |

### Graph Properties

- **Bidirectional connections are NOT guaranteed.** A node's north neighbor may not list the original node as its south neighbor (though most do). The parser must handle asymmetric edges.
- **Duplicate tile IDs occur.** Multiple nodes can reference the same tile with different Grid values (e.g., nodes 72 and 73 in mission 1 both reference tile 3253 but with Grid=4 and Grid=1 respectively). This represents multiple waypoints at the same tile but at different sub-positions.
- **Node count matches exactly.** The declared count in the header equals the number of data lines. Both AINODE and NODES files for the same mission always have the same node count.

### Example

```
 5288	4	-1	8	-1	-1	1
```
Node 0: at tile 5288, grid quadrant 4, no north neighbor, east neighbor is node 8, no south/west neighbors, inside a building.

---

## 2. MOVESnn.DAT -- AI Movement Orders / Behavior Scripts

### Purpose

Defines the initial placement and behavioral scripts for all AI-controlled entities (enemies, NPCs, vehicles) on a given mission map. Each entity has an A/B variant (likely for difficulty scaling or random selection) and a 6-level alert escalation system.

### File Structure

```
Enemies: <count>
NPCs:	<count>
Vehicles: <count>
                                        <- blank line
<Entity blocks...>
                                        <- blank line
Vehicle 1: <TileID> <Grid>
Vehicle 2: <TileID> <Grid>
...
```

### Header

| Field | Type | Description |
|-------|------|-------------|
| Enemies | u8 | Number of enemy unit slots (each has A/B variants, so 10 enemies = 20 blocks) |
| NPCs | u8 | Number of NPC unit slots (also A/B) |
| Vehicles | u8 | Number of vehicles (listed at end of file) |

### Entity Block Format

Each enemy/NPC has **two blocks** (A and B variant):

```
Enemy <N>A:
NPC Type: <type>
Attached To: <id>
Setup: <TileID> <Grid>
Level 1: <threshold>	<Action> <TileID> <Grid> <Action> <TileID> <Grid> ... (10 slots)
Level 2: <threshold>	<Action> <TileID> <Grid> ... (10 slots)
Level 3: <threshold>	<Action> <TileID> <Grid> ... (10 slots)
Level 4: <threshold>	<Action> <TileID> <Grid> ... (10 slots)
Level 5: <threshold>	<Action> <TileID> <Grid> ... (10 slots)
Level 6: <threshold>	<Action> <TileID> <Grid> ... (10 slots)
```

### Entity Header Fields

| Field | Type | Description |
|-------|------|-------------|
| NPC Type | u8 | Unit type identifier. 0 = standard infantry, 2 = vehicle crew / special unit. |
| Attached To | u8 | Vehicle attachment. 0 = independent, N = attached to Vehicle N (crew member). |
| Setup | TileID Grid | Initial spawn position: tile ID and grid quadrant. |

### Alert Level System

Each entity has **6 alert levels**, evaluated in order. Each level has:

| Field | Type | Description |
|-------|------|-------------|
| Threshold | u8 (0-100) | Activation threshold (likely a percentage). 0 = level disabled. 100 = always triggers. Appears to represent an alert/awareness score at which this level's orders activate. |
| Waypoint Slots | 10x | Up to 10 waypoint commands, each as `<Action> <TileID> <Grid>` |

### Action Codes

| Code | Meaning | Notes |
|------|---------|-------|
| N | No action / empty slot | Always paired with `0 0`. Used to pad unused slots. |
| M | Move | Move to the specified tile+grid position. The primary patrol/advance command. |
| I | Investigate | Move cautiously to position (search mode). Often used at alert level 3 and 5, frequently targeting indoor nodes (Inside=1). |
| C | Call for help / Cover | Triggers reinforcement behavior or moves to a cover position. Always paired with `0 0` (no specific destination -- uses nearest cover or calls allies). |
| E | Escape / Evacuate | Flee to extraction point. Typically appears at alert level 6 (highest alert). Often targets map-edge nodes (Inside=4). |
| S | Stand ground / Snipe | Hold position and engage. Paired with `0 0` (stay at current location). Appears at level 6 as a last-stand behavior. |
| V | Vehicle mount | Mount/use a vehicle. Paired with `0 0`. Observed on crew units. |
| W | Wait / Watch | Overwatch or sentry behavior. Paired with `0 0`. |

### Alert Level Interpretation

Based on observed patterns across missions:

| Level | Typical Role | Notes |
|-------|-------------|-------|
| 1 | Patrol route | Standard patrol waypoints when unalerted. High thresholds (75-80). |
| 2 | First response | Initial reaction to contact. Often a single move or hold. |
| 3 | Investigate | Search behavior, often with I-actions and building entry. |
| 4 | Reinforce | Move to support allies. C-actions common. |
| 5 | Escalation | Aggressive advance or retreat to key positions. |
| 6 | Final response | Escape (E), last stand (S), or full assault. Highest thresholds (60-100). |

### Vehicle Entries

Listed at the end of the file:

```
Vehicle 1: <TileID> <Grid>
Vehicle 2: <TileID> <Grid>
```

Simple spawn position. Vehicle behavior is driven by their attached crew members' MOVES entries.

### Parsing Notes

- A/B variants (e.g., Enemy 1A vs Enemy 1B) likely represent alternate configurations. The game probably selects one at mission start for variety.
- Empty entities (all thresholds = 0, all actions = N) are placeholder slots.
- The `0 0` tile+grid pair is the null/sentinel value meaning "no destination" or "use context-dependent target."
- Tab alignment is inconsistent -- parser should split on whitespace, not fixed tabs.

---

## 3. LOCKnn.DAT -- Equipment Store / Locker Inventory

### Purpose

Despite the name suggesting "locked doors," these files define the **equipment store inventory** (the "locker" from which the player purchases weapons, ammo, and equipment between missions). Each mission has its own LOCK file tracking per-mission stock availability.

### File Structure

Items are listed in **pairs**: item name on odd lines, item attributes on even lines. Terminated by `~` sentinel lines.

```
<ItemName>
STOCK: <count>    PRICE: <price>    STATUS:<status>   TYPE:<type>
<ItemName>
STOCK: <count>    PRICE: <price>    STATUS:<status>   TYPE:<type>
...
Empty
STOCK: 0    PRICE: 0       STATUS:EMPTY     TYPE:EMPTY
~
~
```

### Item Record

| Line | Content |
|------|---------|
| Odd | Item display name (free-form string, may contain spaces, punctuation, periods) |
| Even | Attribute line with key:value pairs |

### Attribute Fields

| Field | Type | Description |
|-------|------|-------------|
| STOCK | u16 | Current quantity in stock. 0 = none available. |
| PRICE | u32 | Cost per unit in game currency. |
| STATUS | enum | Availability status (see below). |
| TYPE | enum | Item category (see below). |

### STATUS Values

| Value | Meaning |
|-------|---------|
| STOCKED | Available for purchase (STOCK > 0) |
| OUTOFSTOCK | Temporarily unavailable (STOCK = 0, may restock later) |
| DISCONTINUED | Permanently removed from inventory |
| UNAVAILABLE | Not available at this point in the campaign |
| EMPTY | Sentinel/terminator entry |

### TYPE Values

| Value | Meaning |
|-------|---------|
| WEAPON | Primary weapon (firearms that use AMMO) |
| AMMO | Ammunition for the preceding WEAPON entry |
| WEAPON2 | Secondary weapon / ordnance (grenades, rockets, melee -- items that don't use separate ammo) |
| EQUIPMENT | Non-weapon gear (armor, tools, medical kits, etc.) |
| EMPTY | Sentinel/terminator entry |

### Item Ordering Convention

- **WEAPON + AMMO pairs:** Each firearm is immediately followed by its ammo entry. The ammo entry's name is the caliber designation (e.g., `9_x_19mm`).
- **WEAPON2 entries:** Standalone (grenades, rockets, melee weapons). Not followed by ammo.
- **EQUIPMENT entries:** Grouped at the end before the terminator.
- **Terminator:** An `Empty` item with `STATUS:EMPTY TYPE:EMPTY` followed by two `~` lines.

### Per-Mission Stock Variation

LOCK01.DAT (mission 1) has all items at STOCK=0 with OUTOFSTOCK status -- this is the initial game state where the store is empty. Later missions (e.g., LOCK05.DAT) have items with positive STOCK values and STOCKED status, representing store progression as the campaign advances. Prices and the item catalog remain the same; only STOCK and STATUS change.

### Parsing Notes

- Field spacing is inconsistent (variable whitespace between key:value pairs). Parse on `STOCK:`, `PRICE:`, `STATUS:`, `TYPE:` tokens.
- Ammo names use underscores for spaces in caliber designations (e.g., `9_x_19mm`).
- Some item names have trailing spaces -- trim whitespace.
- The file ends with two `~` lines followed by blank lines.

---

## Cross-File Relationships

The three AI-related files (AINODE, NODES, MOVES) form an integrated system:

1. **AINODE/NODES** define the **topology** -- which waypoints exist and how they connect.
2. **MOVES** defines the **behavior** -- which entities use which waypoints and in what order.
3. MOVES files reference tile IDs and grid values that correspond to nodes in the AINODE/NODES graphs. The AI system looks up the nearest graph node to plan paths between MOVES waypoints.

The LOCK files are independent of the AI system and track the player-facing economy.

### File Naming

All files use the pattern `<TYPE><NN>.DAT` where NN is the zero-padded mission number (01-16).
