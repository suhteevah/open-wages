//! Basic enemy AI decision system.
//!
//! The AI uses a simple priority-based decision tree to pick the best action
//! for an enemy unit each time it gets a turn. This mirrors the original game's
//! MOVES.DAT-driven behavior with an alert escalation system.
//!
//! ## Decision Priorities (highest to lowest)
//!
//! 1. **Shoot** — If an enemy (player merc) is visible and in weapon range,
//!    shoot the nearest one. Combat is always the top priority when engaged.
//!
//! 2. **Advance** — If an enemy is visible but out of range, move toward the
//!    nearest visible enemy to close the distance.
//!
//! 3. **Hunt** — If no enemies are visible, move toward the last known enemy
//!    position or a patrol waypoint. This models the AI "searching" for the
//!    player after losing contact.
//!
//! 4. **Seek Cover** — If the unit is at low HP (below 25% max), try to move
//!    to a tile adjacent to a wall or forest that blocks LOS from enemies.
//!    Self-preservation kicks in for wounded units.
//!
//! ## Alert Escalation (MOVES.DAT system)
//!
//! The mission's `alert_level` determines how aggressively the AI behaves:
//!
//! - **Level 0 (Unaware)**: Enemies patrol or stand guard. They won't actively
//!   seek the player unless they spot them.
//! - **Level 1 (Suspicious)**: A noise or event has alerted the garrison.
//!   Enemies move toward the disturbance but don't shoot on sight.
//! - **Level 2 (Alerted)**: Shots have been fired. Enemies actively search
//!   for the player and will engage on sight.
//! - **Level 3 (Combat)**: Full engagement. All enemies know the player's
//!   position and will aggressively pursue and attack.
//!
//! At alert level 0, the AI only reacts if it personally spots a player merc.
//! At level 3, all enemies coordinate toward the nearest known player position.

use tracing::{debug, trace};

use crate::actions::Action;
use crate::los::has_line_of_sight;
use crate::merc::{MercId, TilePos};
use crate::mission_setup::MissionState;
use crate::pathfinding::TerrainType;

/// Maximum weapon range for AI shooting decisions (matches actions.rs default).
const AI_WEAPON_RANGE: u32 = 15;

/// HP threshold (percentage of max) below which the AI seeks cover.
const LOW_HP_THRESHOLD_PERCENT: u32 = 25;

/// AP cost for shooting (must match actions.rs).
const AI_SHOOT_AP: u32 = 8;

/// Chebyshev distance between two positions.
fn tile_distance(a: TilePos, b: TilePos) -> u32 {
    let dx = (a.x - b.x).unsigned_abs();
    let dy = (a.y - b.y).unsigned_abs();
    dx.max(dy)
}

/// Decide the best action for an enemy unit based on the current tactical situation.
///
/// This is the AI entry point — called once per enemy turn. The decision tree
/// evaluates conditions in priority order and returns the first applicable action.
///
/// # Parameters
/// - `state`: Current mission state (read-only — actions are executed separately).
/// - `unit_id`: The enemy unit making the decision.
///
/// # Returns
/// The chosen `Action`. Falls back to `EndTurn` if nothing useful can be done.
pub fn decide_action(state: &MissionState, unit_id: MercId) -> Action {
    let enemy = match state.enemy_units.iter().find(|e| e.id == unit_id) {
        Some(e) => e,
        None => {
            debug!(unit_id, "AI: unit not found, ending turn");
            return Action::EndTurn;
        }
    };

    let my_pos = match enemy.position {
        Some(p) => p,
        None => {
            debug!(unit_id, "AI: unit has no position, ending turn");
            return Action::EndTurn;
        }
    };

    let my_ap = enemy.current_ap;
    let hp_percent = if enemy.max_hp > 0 {
        (enemy.current_hp * 100) / enemy.max_hp
    } else {
        0
    };

    trace!(
        unit_id,
        ?my_pos,
        ap = my_ap,
        hp = enemy.current_hp,
        hp_percent,
        alert_level = state.alert_level,
        "AI evaluating options"
    );

    // -- Priority 4: Seek cover if low HP --
    // Check this first so wounded units prefer cover over engaging.
    // Only triggers if there are known enemies to hide from.
    if hp_percent <= LOW_HP_THRESHOLD_PERCENT && my_ap >= 2 {
        if let Some(cover_pos) = find_cover_tile(state, my_pos, unit_id) {
            debug!(
                unit_id,
                hp_percent,
                ?cover_pos,
                "AI: low HP, seeking cover"
            );
            return Action::Move(cover_pos);
        }
    }

    // -- Gather intel on visible player mercs --
    let visible_targets: Vec<(MercId, TilePos, u32)> = state
        .player_units
        .iter()
        .filter(|u| u.is_alive())
        .filter_map(|u| {
            let target_pos = u.position?;
            if has_line_of_sight(&state.map, my_pos, target_pos) {
                let dist = tile_distance(my_pos, target_pos);
                Some((u.id, target_pos, dist))
            } else {
                None
            }
        })
        .collect();

    // Sort by distance (nearest first)
    let nearest_visible = visible_targets
        .iter()
        .min_by_key(|(_, _, dist)| *dist);

    // -- Priority 1: Shoot if enemy visible and in range --
    if let Some(&(target_id, _target_pos, dist)) = nearest_visible {
        if dist <= AI_WEAPON_RANGE && my_ap >= AI_SHOOT_AP {
            debug!(
                unit_id,
                target = target_id,
                distance = dist,
                "AI: shooting nearest visible enemy"
            );
            return Action::Shoot(target_id);
        }
    }

    // -- Priority 2: Move toward visible enemy if out of range --
    if let Some(&(_target_id, target_pos, _dist)) = nearest_visible {
        if my_ap >= 2 {
            let move_target = step_toward(my_pos, target_pos, state);
            debug!(
                unit_id,
                ?target_pos,
                ?move_target,
                "AI: advancing toward visible enemy"
            );
            return Action::Move(move_target);
        }
    }

    // -- Alert-level gated behavior --
    // At alert level 0, enemies that don't see a player just idle.
    if state.alert_level == 0 {
        trace!(unit_id, "AI: unaware, no targets visible, ending turn");
        return Action::EndTurn;
    }

    // -- Priority 3: Hunt — move toward nearest known player position --
    // At alert levels >= 1, move toward the nearest player merc even if
    // not currently visible. This simulates the garrison searching.
    if my_ap >= 2 {
        if let Some(nearest_player_pos) = find_nearest_player_position(state, my_pos) {
            let move_target = step_toward(my_pos, nearest_player_pos, state);
            debug!(
                unit_id,
                ?nearest_player_pos,
                ?move_target,
                alert_level = state.alert_level,
                "AI: hunting toward last known player position"
            );
            return Action::Move(move_target);
        }
    }

    // -- Fallback: nothing useful to do --
    debug!(unit_id, "AI: no actions available, ending turn");
    Action::EndTurn
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Find the nearest living player merc's position (even if not visible).
///
/// Used at alert levels >= 1 when the AI knows roughly where the player is
/// but can't see them directly.
fn find_nearest_player_position(state: &MissionState, from: TilePos) -> Option<TilePos> {
    state
        .player_units
        .iter()
        .filter(|u| u.is_alive())
        .filter_map(|u| u.position)
        .min_by_key(|pos| tile_distance(from, *pos))
}

/// Compute one step toward a target position, preferring walkable tiles.
///
/// Returns a tile adjacent to `from` that is closer to `target`. This is a
/// simple greedy step — the full pathfinding happens in the action executor.
/// We pick the neighbour that minimizes Chebyshev distance to the target.
fn step_toward(from: TilePos, target: TilePos, state: &MissionState) -> TilePos {
    let directions = [
        (0, -1),  // N
        (1, 0),   // E
        (0, 1),   // S
        (-1, 0),  // W
        (1, -1),  // NE
        (1, 1),   // SE
        (-1, 1),  // SW
        (-1, -1), // NW
    ];

    let mut best = from;
    let mut best_dist = tile_distance(from, target);

    for (dx, dy) in directions {
        let candidate = TilePos {
            x: from.x + dx,
            y: from.y + dy,
        };
        if state.map.is_walkable(candidate) {
            let dist = tile_distance(candidate, target);
            if dist < best_dist {
                best_dist = dist;
                best = candidate;
            }
        }
    }

    best
}

/// Find a tile adjacent to `from` that provides cover from enemies.
///
/// "Cover" means a tile next to a wall or forest that would block LOS from
/// at least one visible player merc. The AI doesn't do full LOS reversal —
/// it just looks for tiles adjacent to sight-blocking terrain.
fn find_cover_tile(state: &MissionState, from: TilePos, _unit_id: MercId) -> Option<TilePos> {
    let directions = [
        (0, -1),
        (1, 0),
        (0, 1),
        (-1, 0),
        (1, -1),
        (1, 1),
        (-1, 1),
        (-1, -1),
    ];

    // Look for walkable tiles that have at least one adjacent wall/forest
    let mut candidates: Vec<(TilePos, u32)> = Vec::new();

    for (dx, dy) in &directions {
        let tile_pos = TilePos {
            x: from.x + dx,
            y: from.y + dy,
        };

        if !state.map.is_walkable(tile_pos) {
            continue;
        }

        // Count adjacent sight-blocking tiles (more = better cover)
        let cover_score: u32 = directions
            .iter()
            .filter(|&&(ddx, ddy)| {
                let adj = TilePos {
                    x: tile_pos.x + ddx,
                    y: tile_pos.y + ddy,
                };
                state.map.blocks_sight(adj)
                    || state
                        .map
                        .get(adj.x, adj.y)
                        .map(|t| t.terrain == TerrainType::Forest)
                        .unwrap_or(false)
            })
            .count() as u32;

        if cover_score > 0 {
            candidates.push((tile_pos, cover_score));
        }
    }

    // Pick the tile with the highest cover score
    candidates
        .into_iter()
        .max_by_key(|(_, score)| *score)
        .map(|(pos, _)| pos)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merc::{ActiveMerc, MercStatus, TilePos};
    use crate::mission_setup::{EnemyUnit, MissionObjective, MissionState};
    use crate::pathfinding::{TileInfo, TileMap};
    use crate::weather::Weather;
    use crate::game_state::MissionPhase;

    fn test_player(id: MercId, x: i32, y: i32) -> ActiveMerc {
        ActiveMerc {
            id,
            name: format!("Player_{id}"),
            nickname: format!("P{id}"),
            exp: 40,
            str_stat: 50,
            agl: 50,
            wil: 45,
            wsk: 50,
            hhc: 40,
            tch: 30,
            enc: 300,
            base_aps: 20,
            dpr: 100,
            max_hp: 50,
            current_hp: 50,
            current_ap: 20,
            status: MercStatus::OnMission,
            position: Some(TilePos { x, y }),
            inventory: Vec::new(),
            suppressed: false,
            experience_gained: 0,
        }
    }

    fn test_enemy(id: MercId, x: i32, y: i32) -> EnemyUnit {
        EnemyUnit {
            id,
            name: format!("Enemy_{id}"),
            rating: 5,
            enemy_type: 2,
            exp: 20,
            str_stat: 40,
            agl: 40,
            wil: 30,
            wsk: 40,
            hhc: 30,
            tch: 20,
            enc: 200,
            base_aps: 20,
            dpr: 80,
            max_hp: 40,
            current_hp: 40,
            current_ap: 20,
            position: Some(TilePos { x, y }),
            inventory: Vec::new(),
            suppressed: false,
        }
    }

    fn test_state(
        player_pos: (i32, i32),
        enemy_pos: (i32, i32),
        alert_level: u8,
    ) -> MissionState {
        MissionState {
            player_units: vec![test_player(1, player_pos.0, player_pos.1)],
            enemy_units: vec![test_enemy(1001, enemy_pos.0, enemy_pos.1)],
            npcs: Vec::new(),
            map: TileMap::new_uniform(20, 20, TileInfo::open()),
            weather: Weather::Clear,
            turn_number: 1,
            phase: MissionPhase::Combat,
            objectives: vec![MissionObjective::EliminateAll],
            start_hour: 8,
            start_minute: 0,
            alert_level,
        }
    }

    #[test]
    fn ai_shoots_when_in_range() {
        // Enemy at (5, 5), player at (5, 6) — range 1, clear LOS
        let state = test_state((5, 6), (5, 5), 3);
        let action = decide_action(&state, 1001);

        assert!(
            matches!(action, Action::Shoot(1)),
            "AI should shoot visible in-range target, got {action:?}"
        );
    }

    #[test]
    fn ai_moves_toward_visible_enemy_out_of_range() {
        // Enemy at (0, 0), player at (19, 19) — out of range (dist 19 > 15)
        // but visible on open map
        let state = test_state((19, 19), (0, 0), 3);
        let action = decide_action(&state, 1001);

        // Should move toward the player (not shoot, not end turn)
        match action {
            Action::Move(pos) => {
                // The step should be closer to (19,19) than (0,0) is
                let old_dist = tile_distance(TilePos { x: 0, y: 0 }, TilePos { x: 19, y: 19 });
                let new_dist = tile_distance(pos, TilePos { x: 19, y: 19 });
                assert!(
                    new_dist < old_dist,
                    "AI should move closer to target: old_dist={old_dist}, new_dist={new_dist}"
                );
            }
            _ => panic!("Expected Move action, got {action:?}"),
        }
    }

    #[test]
    fn ai_hunts_when_alerted_no_los() {
        // Place a wall between enemy and player so there's no LOS
        let mut state = test_state((10, 10), (5, 10), 3);
        // Wall at (7, 10) and (8, 10) to block LOS
        for x in 6..=9 {
            if let Some(tile) = state.map.get_mut(x, 10) {
                tile.terrain = TerrainType::Wall;
                tile.walkable = false;
            }
        }

        let action = decide_action(&state, 1001);

        // At alert level 3, should still move toward player (hunting)
        match action {
            Action::Move(pos) => {
                // Should be moving toward the player's general direction
                let old_dist = tile_distance(
                    TilePos { x: 5, y: 10 },
                    TilePos { x: 10, y: 10 },
                );
                let new_dist = tile_distance(pos, TilePos { x: 10, y: 10 });
                assert!(
                    new_dist <= old_dist,
                    "AI should move toward player: old={old_dist}, new={new_dist}"
                );
            }
            _ => panic!("Expected Move action for hunting AI, got {action:?}"),
        }
    }

    #[test]
    fn ai_idles_when_unaware_and_no_los() {
        // Alert level 0, wall blocks LOS
        let mut state = test_state((10, 10), (5, 10), 0);
        for x in 6..=9 {
            if let Some(tile) = state.map.get_mut(x, 10) {
                tile.terrain = TerrainType::Wall;
                tile.walkable = false;
            }
        }

        let action = decide_action(&state, 1001);

        // At alert level 0 with no visible targets, should end turn
        assert!(
            matches!(action, Action::EndTurn),
            "Unaware AI with no LOS should idle, got {action:?}"
        );
    }

    #[test]
    fn ai_seeks_cover_when_low_hp() {
        // Enemy at low HP near a wall
        let mut state = test_state((10, 5), (5, 5), 3);

        // Set enemy HP to 25% of max (10/40)
        state.enemy_units[0].current_hp = 10;

        // Put a wall at (4, 5) so (4, 4) or (4, 6) could be cover
        if let Some(tile) = state.map.get_mut(4, 5) {
            tile.terrain = TerrainType::Wall;
            tile.walkable = false;
        }

        let action = decide_action(&state, 1001);

        // At 25% HP with a wall nearby, AI should seek cover (move action)
        assert!(
            matches!(action, Action::Move(_)),
            "Low-HP AI near cover should seek cover, got {action:?}"
        );
    }

    #[test]
    fn ai_ends_turn_with_no_ap() {
        let mut state = test_state((5, 6), (5, 5), 3);
        state.enemy_units[0].current_ap = 0;

        let action = decide_action(&state, 1001);

        // With 0 AP, can't do anything useful
        assert!(
            matches!(action, Action::EndTurn),
            "AI with 0 AP should end turn, got {action:?}"
        );
    }
}
