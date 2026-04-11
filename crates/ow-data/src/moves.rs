//! Parser for `MOVESnn.DAT` — AI movement orders / behavior scripts.
//!
//! # What MOVES Files Do
//!
//! Each mission has a `MOVESnn.DAT` file (e.g., `MOVES01.DAT` for mission 1)
//! that scripts all AI-controlled entities on the map. This is NOT real-time
//! pathfinding — it's a pre-authored waypoint system. Each enemy/NPC has a
//! set of waypoints they follow, and their behavior changes based on an
//! **alert escalation** system (see below).
//!
//! # A/B Variants
//!
//! Every entity has two behavior blocks: an **A variant** and a **B variant**.
//! The game randomly selects one at mission start, giving replay variety.
//! For example, "Enemy 1A" might patrol the north corridor while "Enemy 1B"
//! patrols the south. Only one variant is active per playthrough.
//!
//! # Alert Escalation (6 Levels)
//!
//! Each variant has 6 alert levels with activation thresholds (0-100):
//! - **Level 1**: Default patrol behavior (threshold ~50-75 = likely to activate)
//! - **Level 2-4**: Intermediate escalation (investigate gunfire, take cover)
//! - **Level 5**: High alert (aggressive search patterns)
//! - **Level 6**: Maximum alert / retreat behavior (threshold ~60-100)
//!
//! A threshold of 0 means that level is disabled. The game's alert system
//! raises the global alert value as combat occurs, triggering higher levels.
//!
//! # Action Codes (8 Types)
//!
//! Each alert level has up to 10 waypoint commands, each a triplet of
//! `Action TileID Grid`:
//! - **M** (Move): Walk to the target tile
//! - **I** (Investigate): Move to tile cautiously, weapon ready
//! - **C** (Cover): Take cover at tile (crouch/prone)
//! - **E** (Escape): Flee to tile (used in level 6 for retreat)
//! - **S** (Stand): Hold position at tile
//! - **V** (Vehicle): Board/operate attached vehicle
//! - **W** (Wait): Pause at current position for a duration
//! - **N** (None): No-op / empty slot (tile_id and grid are 0)
//!
//! # Vehicle Spawns vs Crew Attachment
//!
//! Vehicles are spawned separately at the end of the file with their own
//! tile positions. Crew members reference vehicles via `attached_to` — e.g.,
//! `Attached To: 1` means this entity crews Vehicle 1. The `npc_type` field
//! distinguishes infantry (0) from vehicle crew (2).

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, trace, warn};

/// Errors that can occur while parsing MOVES files.
#[derive(Debug, Error)]
pub enum MovesError {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    #[error("line {line}: missing header field '{field}'")]
    MissingHeader { line: usize, field: &'static str },

    #[error("line {line}: failed to parse header value for '{field}': {detail}")]
    InvalidHeader {
        line: usize,
        field: &'static str,
        detail: String,
    },

    #[error("line {line}: expected entity header (e.g. 'Enemy 1A:'), got: {detail}")]
    ExpectedEntityHeader { line: usize, detail: String },

    #[error("line {line}: expected 'Level {level}:' line")]
    ExpectedLevel { line: usize, level: u8 },

    #[error("line {line}: invalid action code '{code}'")]
    InvalidAction { line: usize, code: char },

    #[error("line {line}: failed to parse integer in level data: {detail}")]
    InvalidLevelData { line: usize, detail: String },

    #[error("line {line}: unexpected end of file")]
    UnexpectedEof { line: usize },

    #[error("line {line}: missing field '{field}' in entity header")]
    MissingEntityField { line: usize, field: &'static str },
}

/// A single waypoint command in an alert level.
///
/// Commands are always stored as triplets (action, tile_id, grid). Empty slots
/// use `N 0 0` as padding — the original files always have exactly 10 command
/// slots per level, even if most are no-ops.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MoveCommand {
    /// Action code: M(ove), I(nvestigate), C(over), E(scape), S(tand), V(ehicle), W(ait), N(one).
    pub action: char,
    /// Target tile ID. These are indices into the mission's tile map, not pixel
    /// coordinates. A value of 0 means "no destination" (used with N/W actions).
    pub tile_id: u32,
    /// Grid quadrant within the target tile (1-4). The original engine subdivides
    /// each isometric tile into quadrants for finer positioning. 0 = no quadrant.
    pub grid: u8,
}

/// One alert level's activation threshold and command sequence.
///
/// The threshold determines when this level activates. The game maintains a
/// global alert value that increases as the player is detected, fires weapons,
/// or triggers alarms. When the alert value crosses a level's threshold, the
/// AI switches to that level's command sequence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlertLevel {
    /// Activation threshold (0-100). 0 = disabled (this level is never used),
    /// 100 = always triggers immediately. Typical patrol levels use 50-75.
    pub threshold: u32,
    /// Up to 10 waypoint commands. The AI executes these sequentially — once
    /// all non-N commands are exhausted, the entity holds its last position.
    /// N/0/0 slots are included as-is to preserve the original file's structure.
    pub commands: Vec<MoveCommand>,
}

/// An A or B variant behavior block for a single entity.
///
/// The game picks either A or B for each entity at mission start. Both variants
/// share the same entity "slot" (e.g., Enemy 1) but can have completely
/// different spawn positions and patrol routes, adding replay variety.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntityBehavior {
    /// Entity label, e.g. "Enemy 1A", "NPC 2B". The trailing letter is the variant.
    pub label: String,
    /// Variant: 'A' or 'B'. Extracted from the last character of the label.
    pub variant: char,
    /// NPC type identifier. 0 = standard infantry (walks, uses cover, carries
    /// small arms). 2 = vehicle crew or special unit (operates attached vehicle,
    /// different AI behavior when dismounted).
    pub npc_type: u8,
    /// Vehicle attachment. 0 = independent foot soldier. N = crews Vehicle N
    /// (the vehicle must exist in the vehicles list at the end of the file).
    /// When attached, the entity's movement is controlled by the vehicle until
    /// it's destroyed, at which point the crew dismounts and reverts to infantry AI.
    pub attached_to: u8,
    /// Initial spawn tile ID — where this entity appears at mission start.
    pub setup_tile: u32,
    /// Initial spawn grid quadrant within the tile (1-4).
    pub setup_grid: u8,
    /// The 6 alert levels, indexed 0-5 (representing Level 1 through Level 6).
    pub alert_levels: Vec<AlertLevel>,
}

/// A vehicle spawn entry at the end of the file.
///
/// Vehicles are listed after all entity behavior blocks. They only define a
/// spawn position — the vehicle's behavior is driven by its attached crew
/// members. If all crew are killed, the vehicle becomes inert.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VehicleSpawn {
    /// Vehicle number (1-based). Crew members reference this via `attached_to`.
    pub index: u32,
    /// Spawn tile ID — where the vehicle appears on the map at mission start.
    pub tile_id: u32,
    /// Spawn grid quadrant within the tile.
    pub grid: u8,
}

/// The complete parsed MOVES file for a mission.
///
/// The file structure is: header (3 count lines) -> entity blocks -> vehicle entries.
/// Entity blocks appear in order: Enemy 1A, Enemy 1B, Enemy 2A, Enemy 2B, ...,
/// then NPC 1A, NPC 1B, etc. The total number of behavior blocks is always
/// `(enemy_count + npc_count) * 2` because every entity has both A and B variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveScript {
    /// Number of enemy unit slots. Each slot produces 2 behavior blocks (A + B).
    pub enemy_count: u32,
    /// Number of NPC unit slots. NPCs are non-hostile AI entities (civilians,
    /// allies, mission-critical characters). Each also has A/B variants.
    pub npc_count: u32,
    /// Number of vehicles on this mission map.
    pub vehicle_count: u32,
    /// All entity behavior blocks (enemies A/B, then NPCs A/B). Length is
    /// always `(enemy_count + npc_count) * 2`.
    pub behaviors: Vec<EntityBehavior>,
    /// Vehicle spawn positions, listed at the end of the file.
    pub vehicles: Vec<VehicleSpawn>,
}

/// Valid action codes for move commands.
/// M=Move, I=Investigate, C=Cover, E=Escape, S=Stand, V=Vehicle, W=Wait, N=None.
/// Any other character in the data file is a parsing error — the original game
/// only uses these 8 codes.
const VALID_ACTIONS: &[char] = &['M', 'I', 'C', 'E', 'S', 'V', 'W', 'N'];

/// Parse a `MOVESnn.DAT` file into a [`MoveScript`].
pub fn parse_moves(path: &Path) -> Result<MoveScript, MovesError> {
    info!("parsing MOVES script from {}", path.display());

    let contents = std::fs::read_to_string(path).map_err(|e| MovesError::Io {
        path: path.display().to_string(),
        source: e,
    })?;

    let lines: Vec<&str> = contents.lines().collect();
    let mut cursor = 0;

    // Helper to get a trimmed line or return EOF error.
    let get_line = |idx: usize| -> Result<&str, MovesError> {
        lines
            .get(idx)
            .map(|l| l.trim_end_matches('\r'))
            .ok_or(MovesError::UnexpectedEof { line: idx + 1 })
    };

    // --- Parse header ---
    // The first 3 lines declare counts: "Enemies: N", "NPCs: N", "Vehicles: N".
    // These determine how many entity blocks and vehicle entries to expect below.
    let enemy_count = parse_header_line(get_line(cursor)?, cursor + 1, "Enemies")?;
    cursor += 1;
    let npc_count = parse_header_line(get_line(cursor)?, cursor + 1, "NPCs")?;
    cursor += 1;
    let vehicle_count = parse_header_line(get_line(cursor)?, cursor + 1, "Vehicles")?;
    cursor += 1;

    info!("header: {enemy_count} enemies, {npc_count} NPCs, {vehicle_count} vehicles");

    // Skip blank lines after header.
    while cursor < lines.len() {
        let line = get_line(cursor)?.trim();
        if !line.is_empty() {
            break;
        }
        cursor += 1;
    }

    // --- Parse entity blocks ---
    // Every entity slot produces 2 blocks (A and B variants), so the total
    // number of behavior blocks is always double the entity count.
    let total_entities = (enemy_count + npc_count) * 2;
    let mut behaviors = Vec::with_capacity(total_entities as usize);

    for _ in 0..total_entities {
        // Skip blank lines between entities.
        while cursor < lines.len() {
            let line = get_line(cursor)?.trim();
            if !line.is_empty() {
                break;
            }
            cursor += 1;
        }
        if cursor >= lines.len() {
            break;
        }

        let (behavior, new_cursor) = parse_entity_block(&lines, cursor)?;
        debug!(
            "parsed entity '{}': npc_type={} attached_to={} setup=({},{})",
            behavior.label,
            behavior.npc_type,
            behavior.attached_to,
            behavior.setup_tile,
            behavior.setup_grid
        );
        behaviors.push(behavior);
        cursor = new_cursor;
    }

    // --- Parse vehicle entries ---
    // Vehicle lines appear at the end of the file, after all entity blocks.
    // Format: "Vehicle N: <tile_id> <grid>" — simple spawn positions.
    // The vehicle's behavior comes from its attached crew, not from its own data.
    let mut vehicles = Vec::with_capacity(vehicle_count as usize);
    while cursor < lines.len() {
        let line = get_line(cursor)?.trim();
        cursor += 1;

        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("Vehicle ") {
            // Format: "Vehicle 1: 2971 3" — index, tile ID, grid quadrant.
            // We use unwrap_or(0) for parse failures because some modded data
            // files have malformed vehicle lines that the original game ignores.
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() == 2 {
                let idx: u32 = parts[0].trim().parse().unwrap_or(0);
                let vals: Vec<&str> = parts[1].split_whitespace().collect();
                if vals.len() >= 2 {
                    let tile_id: u32 = vals[0].parse().unwrap_or(0);
                    let grid: u8 = vals[1].parse().unwrap_or(0);
                    debug!("vehicle {idx}: tile={tile_id} grid={grid}");
                    vehicles.push(VehicleSpawn {
                        index: idx,
                        tile_id,
                        grid,
                    });
                }
            }
        }
    }

    info!(
        "successfully parsed {} behaviors and {} vehicles from {}",
        behaviors.len(),
        vehicles.len(),
        path.display()
    );

    Ok(MoveScript {
        enemy_count,
        npc_count,
        vehicle_count,
        behaviors,
        vehicles,
    })
}

/// Parse a header line like "Enemies: 10" or "NPCs:\t1".
///
/// The original data files use inconsistent whitespace — sometimes tabs,
/// sometimes spaces after the colon. We normalize by trimming after the colon.
fn parse_header_line(line: &str, line_num: usize, field: &'static str) -> Result<u32, MovesError> {
    let line = line.trim();
    // Match field name case-insensitively at the start.
    let lower = line.to_lowercase();
    let prefix = format!("{}:", field.to_lowercase());
    if !lower.starts_with(&prefix) {
        return Err(MovesError::MissingHeader {
            line: line_num,
            field,
        });
    }
    let val_str = line[prefix.len()..].trim();
    val_str.parse().map_err(|_| MovesError::InvalidHeader {
        line: line_num,
        field,
        detail: format!("'{val_str}' is not a valid integer"),
    })
}

/// Parse a single entity block (header + 6 levels), returning the behavior and the new cursor.
///
/// Each entity block is structured as:
/// - Line 1: Entity header (e.g., "Enemy 1A:")
/// - Line 2: "NPC Type: <n>"
/// - Line 3: "Attached To: <n>"
/// - Line 4: "Setup: <tile_id> <grid>"
/// - Lines 5-10: "Level 1: ..." through "Level 6: ..." (one per alert level)
fn parse_entity_block(lines: &[&str], start: usize) -> Result<(EntityBehavior, usize), MovesError> {
    let mut cursor = start;

    let get = |idx: usize| -> Result<&str, MovesError> {
        lines
            .get(idx)
            .map(|l| l.trim_end_matches('\r'))
            .ok_or(MovesError::UnexpectedEof { line: idx + 1 })
    };

    // Entity header line, e.g. "Enemy 1A:" or "NPC 1B:".
    // The variant letter (A or B) is always the last character before the colon.
    let header_line = get(cursor)?.trim();
    let label = header_line.trim_end_matches(':').trim().to_string();
    let variant = label.chars().last().unwrap_or('A');
    trace!("line {}: entity header '{}'", cursor + 1, label);
    cursor += 1;

    // "NPC Type: <n>"
    let npc_type_line = get(cursor)?.trim();
    let npc_type = extract_trailing_int(npc_type_line, cursor + 1, "NPC Type")? as u8;
    cursor += 1;

    // "Attached To: <n>"
    let attached_line = get(cursor)?.trim();
    let attached_to = extract_trailing_int(attached_line, cursor + 1, "Attached To")? as u8;
    cursor += 1;

    // "Setup: <TileID> <Grid>"
    let setup_line = get(cursor)?.trim();
    let setup_rest = setup_line
        .split(':')
        .nth(1)
        .ok_or(MovesError::MissingEntityField {
            line: cursor + 1,
            field: "Setup",
        })?
        .trim();
    let setup_parts: Vec<&str> = setup_rest.split_whitespace().collect();
    let setup_tile: u32 = setup_parts.first().unwrap_or(&"0").parse().unwrap_or(0);
    let setup_grid: u8 = setup_parts.get(1).unwrap_or(&"0").parse().unwrap_or(0);
    trace!(
        "line {}: setup tile={} grid={}",
        cursor + 1,
        setup_tile,
        setup_grid
    );
    cursor += 1;

    // Parse all 6 alert levels. Every entity always has exactly 6 levels in the
    // file, even if most are disabled (threshold=0, all N/0/0 commands). This
    // fixed structure makes parsing predictable.
    let mut alert_levels = Vec::with_capacity(6);
    for level_num in 1..=6u8 {
        let level_line = get(cursor)?.trim_end_matches('\r');
        let alert_level = parse_level_line(level_line, cursor + 1, level_num)?;
        trace!(
            "line {}: level {} threshold={} commands={}",
            cursor + 1,
            level_num,
            alert_level.threshold,
            alert_level.commands.len()
        );
        alert_levels.push(alert_level);
        cursor += 1;
    }

    Ok((
        EntityBehavior {
            label,
            variant,
            npc_type,
            attached_to,
            setup_tile,
            setup_grid,
            alert_levels,
        },
        cursor,
    ))
}

/// Extract a trailing integer from a "Key: value" line.
fn extract_trailing_int(
    line: &str,
    line_num: usize,
    field: &'static str,
) -> Result<u32, MovesError> {
    let val_str = line
        .split(':')
        .nth(1)
        .ok_or(MovesError::MissingEntityField {
            line: line_num,
            field,
        })?
        .trim();
    val_str.parse().map_err(|_| MovesError::InvalidLevelData {
        line: line_num,
        detail: format!("'{val_str}' is not a valid integer for {field}"),
    })
}

/// Parse a level line like "Level 1: 75\tM 2417 3 M 3545 3 ... N 0 0".
///
/// Format: `Level <N>: <threshold>\t<action> <tile> <grid> <action> <tile> <grid> ...`
/// The tab after the threshold is how the original game separates the threshold
/// from the command sequence. There are always 10 command triplets per level.
fn parse_level_line(
    line: &str,
    line_num: usize,
    expected_level: u8,
) -> Result<AlertLevel, MovesError> {
    // Split off "Level N:" prefix. We rejoin with ":" in case the line
    // contains colons elsewhere (unlikely but defensive).
    let after_colon = line.split(':').skip(1).collect::<Vec<&str>>().join(":");
    let after_colon = after_colon.trim();

    // The first token is the threshold.
    let tokens: Vec<&str> = after_colon.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(MovesError::ExpectedLevel {
            line: line_num,
            level: expected_level,
        });
    }

    let threshold: u32 = tokens[0]
        .parse()
        .map_err(|_| MovesError::InvalidLevelData {
            line: line_num,
            detail: format!("'{}' is not a valid threshold", tokens[0]),
        })?;

    // Remaining tokens are triplets: Action TileID Grid. We consume them
    // 3 at a time. If there's a partial triplet at the end, we stop — this
    // handles files that were truncated or hand-edited.
    let mut commands = Vec::new();
    let mut i = 1;
    while i + 2 <= tokens.len() {
        let action_str = tokens[i];
        if action_str.len() != 1 {
            // Not a valid action token; stop parsing commands.
            warn!(
                "line {line_num}: unexpected token '{}' at position {i}, stopping command parse",
                action_str
            );
            break;
        }
        let action = action_str.chars().next().unwrap();

        if !VALID_ACTIONS.contains(&action) {
            return Err(MovesError::InvalidAction {
                line: line_num,
                code: action,
            });
        }

        let tile_id: u32 = tokens[i + 1]
            .parse()
            .map_err(|_| MovesError::InvalidLevelData {
                line: line_num,
                detail: format!("'{}' is not a valid tile ID", tokens[i + 1]),
            })?;

        let grid: u8 = tokens[i + 2]
            .parse()
            .map_err(|_| MovesError::InvalidLevelData {
                line: line_num,
                detail: format!("'{}' is not a valid grid value", tokens[i + 2]),
            })?;

        commands.push(MoveCommand {
            action,
            tile_id,
            grid,
        });
        i += 3;
    }

    Ok(AlertLevel {
        threshold,
        commands,
    })
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

    #[test]
    fn parse_minimal_moves() {
        let data = "\
Enemies: 1\r\n\
NPCs:\t0\r\n\
Vehicles: 0\r\n\
\r\n\
Enemy 1A:\r\n\
NPC Type: 0\r\n\
Attached To: 0\r\n\
Setup: 5000 2\r\n\
Level 1: 75\tM 100 1 M 200 2 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 2: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 3: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 4: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 5: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 6: 60\tE 9000 4 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
\r\n\
Enemy 1B:\r\n\
NPC Type: 0\r\n\
Attached To: 0\r\n\
Setup: 6000 3\r\n\
Level 1: 43\tM 300 1 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 2: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 3: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 4: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 5: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 6: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
";
        let f = write_temp_file(data);
        let script = parse_moves(f.path()).unwrap();

        assert_eq!(script.enemy_count, 1);
        assert_eq!(script.npc_count, 0);
        assert_eq!(script.vehicle_count, 0);
        assert_eq!(script.behaviors.len(), 2);

        let a = &script.behaviors[0];
        assert_eq!(a.label, "Enemy 1A");
        assert_eq!(a.variant, 'A');
        assert_eq!(a.npc_type, 0);
        assert_eq!(a.setup_tile, 5000);
        assert_eq!(a.setup_grid, 2);

        // Level 1 should have 10 commands (2 M + 8 N).
        assert_eq!(a.alert_levels[0].threshold, 75);
        assert_eq!(a.alert_levels[0].commands.len(), 10);
        assert_eq!(a.alert_levels[0].commands[0].action, 'M');
        assert_eq!(a.alert_levels[0].commands[0].tile_id, 100);
        assert_eq!(a.alert_levels[0].commands[1].action, 'M');
        assert_eq!(a.alert_levels[0].commands[1].tile_id, 200);
        assert_eq!(a.alert_levels[0].commands[2].action, 'N');

        // Level 6 should have escape.
        assert_eq!(a.alert_levels[5].threshold, 60);
        assert_eq!(a.alert_levels[5].commands[0].action, 'E');
        assert_eq!(a.alert_levels[5].commands[0].tile_id, 9000);
    }

    #[test]
    fn parse_with_vehicles() {
        let data = "\
Enemies: 1\r\n\
NPCs:\t0\r\n\
Vehicles: 2\r\n\
\r\n\
Enemy 1A:\r\n\
NPC Type: 2\r\n\
Attached To: 1\r\n\
Setup: 1000 1\r\n\
Level 1: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 2: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 3: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 4: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 5: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 6: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
\r\n\
Enemy 1B:\r\n\
NPC Type: 2\r\n\
Attached To: 1\r\n\
Setup: 2000 3\r\n\
Level 1: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 2: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 3: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 4: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 5: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 6: 100\tV 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
\r\n\
Vehicle 1: 2971 3\r\n\
Vehicle 2: 5987 4\r\n\
";
        let f = write_temp_file(data);
        let script = parse_moves(f.path()).unwrap();

        assert_eq!(script.vehicle_count, 2);
        assert_eq!(script.vehicles.len(), 2);
        assert_eq!(script.vehicles[0].index, 1);
        assert_eq!(script.vehicles[0].tile_id, 2971);
        assert_eq!(script.vehicles[0].grid, 3);
        assert_eq!(script.vehicles[1].index, 2);
        assert_eq!(script.vehicles[1].tile_id, 5987);

        assert_eq!(script.behaviors[0].npc_type, 2);
        assert_eq!(script.behaviors[0].attached_to, 1);

        // Level 6 V action.
        assert_eq!(script.behaviors[1].alert_levels[5].commands[0].action, 'V');
    }

    #[test]
    fn all_action_codes_accepted() {
        // Build a level line with every valid action code.
        let data = "\
Enemies: 1\r\n\
NPCs:\t0\r\n\
Vehicles: 0\r\n\
\r\n\
Enemy 1A:\r\n\
NPC Type: 0\r\n\
Attached To: 0\r\n\
Setup: 100 1\r\n\
Level 1: 50\tM 1 1 I 2 2 C 0 0 E 3 3 S 0 0 V 0 0 W 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 2: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 3: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 4: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 5: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 6: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
\r\n\
Enemy 1B:\r\n\
NPC Type: 0\r\n\
Attached To: 0\r\n\
Setup: 100 1\r\n\
Level 1: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 2: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 3: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 4: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 5: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 6: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
";
        let f = write_temp_file(data);
        let script = parse_moves(f.path()).unwrap();

        let cmds = &script.behaviors[0].alert_levels[0].commands;
        assert_eq!(cmds.len(), 10);
        assert_eq!(cmds[0].action, 'M');
        assert_eq!(cmds[1].action, 'I');
        assert_eq!(cmds[2].action, 'C');
        assert_eq!(cmds[3].action, 'E');
        assert_eq!(cmds[4].action, 'S');
        assert_eq!(cmds[5].action, 'V');
        assert_eq!(cmds[6].action, 'W');
        assert_eq!(cmds[7].action, 'N');
    }

    #[test]
    fn invalid_action_rejected() {
        let data = "\
Enemies: 1\r\n\
NPCs:\t0\r\n\
Vehicles: 0\r\n\
\r\n\
Enemy 1A:\r\n\
NPC Type: 0\r\n\
Attached To: 0\r\n\
Setup: 100 1\r\n\
Level 1: 50\tX 1 1 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 2: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 3: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 4: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 5: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 6: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
\r\n\
Enemy 1B:\r\n\
NPC Type: 0\r\n\
Attached To: 0\r\n\
Setup: 100 1\r\n\
Level 1: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 2: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 3: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 4: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 5: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
Level 6: 0\tN 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0 N 0 0\r\n\
";
        let f = write_temp_file(data);
        let err = parse_moves(f.path()).unwrap_err();
        assert!(matches!(err, MovesError::InvalidAction { code: 'X', .. }));
    }
}
