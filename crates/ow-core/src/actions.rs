//! Player and AI action system.
//!
//! Every action a unit can take on its turn is represented by the [`Action`] enum.
//! Actions are validated, executed, and resolved through [`execute_action`], which
//! mutates the `MissionState` and returns an [`ActionResult`] describing what happened.
//!
//! ## Action Point System
//!
//! Every action costs AP (Action Points). A unit starts each round with AP based on
//! its `base_aps` stat (halved if suppressed). When AP runs out, the unit's turn ends.
//!
//! Typical costs:
//! - **Move**: 2+ AP per tile (depends on encumbrance and terrain)
//! - **Shoot**: 8 AP for aimed shot, 4 AP for snap shot
//! - **Reload**: 4 AP
//! - **Crouch/Stand**: 2 AP
//! - **Use Item**: varies (typically 4 AP)
//! - **Overwatch**: all remaining AP reserved for reaction fire
//! - **End Turn**: 0 AP (just passes)

use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace};

use crate::damage::{check_suppression, resolve_attack, AttackResult};
use crate::los::has_line_of_sight;
use crate::merc::{MercId, MercStatus, TilePos};
use crate::mission_setup::MissionState;
use crate::pathfinding::find_path;

// ---------------------------------------------------------------------------
// Action enum — what a unit can do
// ---------------------------------------------------------------------------

/// An action a unit can attempt on its turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Move to a target tile. Pathfinding determines the route and AP cost.
    Move(TilePos),
    /// Shoot at a target unit. Requires LOS, range, and ammo.
    Shoot(MercId),
    /// Reload the currently equipped weapon.
    Reload,
    /// Use an inventory item (medkit, grenade, etc.) by name.
    UseItem(String),
    /// End the unit's turn immediately, forfeiting remaining AP.
    EndTurn,
    /// Toggle crouch stance — reduces profile (harder to hit) but costs AP.
    Crouch,
    /// Enter overwatch: reserve all remaining AP for reaction shots against
    /// enemies that move through the unit's field of fire. The unit does
    /// nothing now but may interrupt enemy movement later.
    OverWatch,
}

// ---------------------------------------------------------------------------
// ActionEffect — granular outcomes
// ---------------------------------------------------------------------------

/// A discrete effect produced by executing an action.
///
/// A single action can produce multiple effects (e.g., a shot can deal damage
/// AND suppress the target). The UI layer uses these to play animations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionEffect {
    /// Unit moved from one tile to another.
    Moved { from: TilePos, to: TilePos },
    /// Damage was dealt to a target.
    DamageDealt {
        target: MercId,
        amount: u32,
        penetrated: bool,
    },
    /// Target was suppressed by incoming fire (even on a miss).
    Suppressed { target: MercId },
    /// A weapon was reloaded.
    Reloaded,
    /// An inventory item was consumed.
    ItemUsed { item: String },
    /// The unit's turn was ended (voluntarily or by AP exhaustion).
    TurnEnded,
    /// Unit entered overwatch stance.
    EnteredOverwatch,
    /// Unit toggled crouch stance.
    Crouched,
    /// Target was killed.
    TargetKilled { target: MercId },
}

// ---------------------------------------------------------------------------
// ActionResult
// ---------------------------------------------------------------------------

/// The outcome of executing an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// Whether the action completed successfully.
    pub success: bool,
    /// Total AP spent on this action.
    pub ap_cost: u32,
    /// All effects that occurred as a result.
    pub effects: Vec<ActionEffect>,
}

// ---------------------------------------------------------------------------
// ActionError
// ---------------------------------------------------------------------------

/// Reasons an action cannot be executed.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ActionError {
    #[error("unit {0} not found in mission state")]
    UnitNotFound(MercId),

    #[error("unit {0} has no position on the map")]
    NoPosition(MercId),

    #[error("not enough AP: need {needed}, have {have}")]
    NotEnoughAp { needed: u32, have: u32 },

    #[error("no path to target tile {0:?}")]
    NoPath(TilePos),

    #[error("target {0} not found")]
    TargetNotFound(MercId),

    #[error("no line of sight to target {0}")]
    NoLineOfSight(MercId),

    #[error("target {0} is out of weapon range")]
    OutOfRange(MercId),

    #[error("no weapon equipped")]
    NoWeapon,

    #[error("no ammo remaining")]
    NoAmmo,

    #[error("item '{0}' not found in inventory")]
    ItemNotFound(String),

    #[error("unit {0} cannot act (dead, MIA, or no AP)")]
    CannotAct(MercId),
}

// ---------------------------------------------------------------------------
// AP cost constants
// ---------------------------------------------------------------------------

/// AP cost for a aimed shot.
const SHOOT_AP_COST: u32 = 8;

/// AP cost for reloading a weapon.
const RELOAD_AP_COST: u32 = 4;

/// AP cost for using an inventory item.
const USE_ITEM_AP_COST: u32 = 4;

/// AP cost for toggling crouch stance.
const CROUCH_AP_COST: u32 = 2;

/// Maximum effective weapon range in tiles (placeholder — real value comes
/// from WEAPON.DAT per weapon type).
const DEFAULT_WEAPON_RANGE: u32 = 15;

/// Default weapon damage class (placeholder).
const DEFAULT_WEAPON_DAMAGE: u32 = 8;

/// Default weapon penetration rating (placeholder).
const DEFAULT_WEAPON_PEN: u32 = 5;

/// Default armor protection rating (placeholder — will come from equipment).
const DEFAULT_ARMOR: u32 = 3;

// ---------------------------------------------------------------------------
// Helper: find a unit across player and enemy lists
// ---------------------------------------------------------------------------

/// Chebyshev distance between two tiles (used for range checks).
fn tile_distance(a: TilePos, b: TilePos) -> u32 {
    let dx = (a.x - b.x).unsigned_abs();
    let dy = (a.y - b.y).unsigned_abs();
    dx.max(dy) // Chebyshev distance — appropriate for 8-directional grid
}

// ---------------------------------------------------------------------------
// execute_action — the big dispatcher
// ---------------------------------------------------------------------------

/// Execute an action for a unit, mutating the mission state.
///
/// Validates preconditions (AP, LOS, range, ammo), performs the action, and
/// returns an `ActionResult` with all effects. On failure, returns an
/// `ActionError` explaining why the action was rejected.
///
/// # Parameters
/// - `state`: The current mission state (mutated in place).
/// - `unit_id`: The acting unit's id.
/// - `action`: The action to perform.
/// - `hit_table`: The parsed TARGET.DAT hit probability table (needed for shooting).
/// - `rng`: Random number generator for attack rolls.
pub fn execute_action<R: rand::Rng>(
    state: &mut MissionState,
    unit_id: MercId,
    action: Action,
    hit_table: &ow_data::target::HitTable,
    rng: &mut R,
) -> Result<ActionResult, ActionError> {
    debug!(unit_id, action = ?action, "Executing action");

    // Look up the acting unit's stats from whichever list it belongs to.
    let (unit_pos, unit_ap, unit_wsk) = {
        if let Some(u) = state.player_units.iter().find(|u| u.id == unit_id) {
            if !u.can_act() {
                return Err(ActionError::CannotAct(unit_id));
            }
            (
                u.position.ok_or(ActionError::NoPosition(unit_id))?,
                u.current_ap,
                u.wsk as u32,
            )
        } else if let Some(e) = state.enemy_units.iter().find(|e| e.id == unit_id) {
            let merc = e.to_active_merc();
            if !merc.can_act() {
                return Err(ActionError::CannotAct(unit_id));
            }
            (
                e.position.ok_or(ActionError::NoPosition(unit_id))?,
                e.current_ap,
                e.wsk as u32,
            )
        } else {
            return Err(ActionError::UnitNotFound(unit_id));
        }
    };

    match action {
        Action::Move(target) => execute_move(state, unit_id, unit_pos, unit_ap, target),
        Action::Shoot(target_id) => {
            execute_shoot(state, unit_id, unit_pos, unit_ap, unit_wsk, target_id, hit_table, rng)
        }
        Action::Reload => execute_reload(state, unit_id, unit_ap),
        Action::UseItem(ref item_name) => {
            execute_use_item(state, unit_id, unit_ap, item_name.clone())
        }
        Action::EndTurn => execute_end_turn(state, unit_id),
        Action::Crouch => execute_crouch(state, unit_id, unit_ap),
        Action::OverWatch => execute_overwatch(state, unit_id, unit_ap),
    }
}

// ---------------------------------------------------------------------------
// Individual action executors
// ---------------------------------------------------------------------------

/// Move action: pathfind to target, deduct AP per tile, update position.
fn execute_move(
    state: &mut MissionState,
    unit_id: MercId,
    from: TilePos,
    current_ap: u32,
    target: TilePos,
) -> Result<ActionResult, ActionError> {
    // Find path using the pathfinding module
    let (path, ap_cost) = find_path(&state.map, from, target, current_ap)
        .ok_or(ActionError::NoPath(target))?;

    if ap_cost > current_ap {
        return Err(ActionError::NotEnoughAp {
            needed: ap_cost,
            have: current_ap,
        });
    }

    // Update unit position and deduct AP
    let final_pos = *path.last().unwrap_or(&from);

    if let Some(u) = state.player_units.iter_mut().find(|u| u.id == unit_id) {
        u.position = Some(final_pos);
        u.current_ap = u.current_ap.saturating_sub(ap_cost);
        debug!(
            id = unit_id,
            from = ?from,
            to = ?final_pos,
            ap_cost,
            ap_remaining = u.current_ap,
            "Player unit moved"
        );
    } else if let Some(e) = state.enemy_units.iter_mut().find(|e| e.id == unit_id) {
        e.position = Some(final_pos);
        e.current_ap = e.current_ap.saturating_sub(ap_cost);
        debug!(
            id = unit_id,
            from = ?from,
            to = ?final_pos,
            ap_cost,
            ap_remaining = e.current_ap,
            "Enemy unit moved"
        );
    }

    Ok(ActionResult {
        success: true,
        ap_cost,
        effects: vec![ActionEffect::Moved { from, to: final_pos }],
    })
}

/// Shoot action: check LOS, range, resolve attack, apply damage/suppression.
#[allow(clippy::too_many_arguments)]
fn execute_shoot<R: rand::Rng>(
    state: &mut MissionState,
    unit_id: MercId,
    unit_pos: TilePos,
    current_ap: u32,
    unit_wsk: u32,
    target_id: MercId,
    hit_table: &ow_data::target::HitTable,
    rng: &mut R,
) -> Result<ActionResult, ActionError> {
    // Check AP
    if current_ap < SHOOT_AP_COST {
        return Err(ActionError::NotEnoughAp {
            needed: SHOOT_AP_COST,
            have: current_ap,
        });
    }

    // Find target position
    let target_pos = {
        if let Some(t) = state.player_units.iter().find(|u| u.id == target_id) {
            t.position.ok_or(ActionError::NoPosition(target_id))?
        } else if let Some(t) = state.enemy_units.iter().find(|e| e.id == target_id) {
            t.position.ok_or(ActionError::NoPosition(target_id))?
        } else {
            return Err(ActionError::TargetNotFound(target_id));
        }
    };

    // Check LOS
    if !has_line_of_sight(&state.map, unit_pos, target_pos) {
        return Err(ActionError::NoLineOfSight(target_id));
    }

    // Check range
    let range = tile_distance(unit_pos, target_pos);
    if range > DEFAULT_WEAPON_RANGE {
        return Err(ActionError::OutOfRange(target_id));
    }

    // Deduct AP from the shooter
    if let Some(u) = state.player_units.iter_mut().find(|u| u.id == unit_id) {
        u.current_ap = u.current_ap.saturating_sub(SHOOT_AP_COST);
    } else if let Some(e) = state.enemy_units.iter_mut().find(|e| e.id == unit_id) {
        e.current_ap = e.current_ap.saturating_sub(SHOOT_AP_COST);
    }

    // Resolve the attack
    let weather_mod = state.weather.accuracy_modifier();
    let roll = rng.gen_range(0..100);

    let attack_result = resolve_attack(
        unit_wsk,
        DEFAULT_WEAPON_DAMAGE,
        DEFAULT_WEAPON_PEN,
        DEFAULT_ARMOR,
        range,
        weather_mod,
        hit_table,
        roll,
    );

    let mut effects = Vec::new();

    match attack_result {
        AttackResult::Hit { damage, penetrated } => {
            info!(
                attacker = unit_id,
                target = target_id,
                damage,
                penetrated,
                range,
                "Attack hit"
            );

            effects.push(ActionEffect::DamageDealt {
                target: target_id,
                amount: damage,
                penetrated,
            });

            // Apply damage to the target
            let target_killed = apply_damage(state, target_id, damage);
            if target_killed {
                effects.push(ActionEffect::TargetKilled { target: target_id });
            }

            // Raise alert level on combat
            if state.alert_level < 3 {
                state.alert_level = 3;
                debug!("Alert level raised to 3 (combat)");
            }
        }
        AttackResult::Miss => {
            debug!(
                attacker = unit_id,
                target = target_id,
                range,
                roll,
                "Attack missed"
            );

            // Check suppression on near misses
            let target_wil = get_unit_wil(state, target_id).unwrap_or(50);
            if check_suppression(target_wil, DEFAULT_WEAPON_DAMAGE, range) {
                apply_suppression(state, target_id);
                effects.push(ActionEffect::Suppressed { target: target_id });
            }

            // Raise alert on shots fired (even misses)
            if state.alert_level < 2 {
                state.alert_level = 2;
                debug!("Alert level raised to 2 (shots fired)");
            }
        }
        AttackResult::Suppressed => {
            // Suppression-only result (from damage.rs)
            apply_suppression(state, target_id);
            effects.push(ActionEffect::Suppressed { target: target_id });
        }
    }

    Ok(ActionResult {
        success: matches!(attack_result, AttackResult::Hit { .. }),
        ap_cost: SHOOT_AP_COST,
        effects,
    })
}

/// Reload action: restore ammo (placeholder until WEAPON.DAT integration).
fn execute_reload(
    state: &mut MissionState,
    unit_id: MercId,
    current_ap: u32,
) -> Result<ActionResult, ActionError> {
    if current_ap < RELOAD_AP_COST {
        return Err(ActionError::NotEnoughAp {
            needed: RELOAD_AP_COST,
            have: current_ap,
        });
    }

    // Deduct AP
    deduct_ap(state, unit_id, RELOAD_AP_COST);
    debug!(unit_id, "Weapon reloaded");

    Ok(ActionResult {
        success: true,
        ap_cost: RELOAD_AP_COST,
        effects: vec![ActionEffect::Reloaded],
    })
}

/// Use item action: consume an inventory item.
fn execute_use_item(
    state: &mut MissionState,
    unit_id: MercId,
    current_ap: u32,
    item_name: String,
) -> Result<ActionResult, ActionError> {
    if current_ap < USE_ITEM_AP_COST {
        return Err(ActionError::NotEnoughAp {
            needed: USE_ITEM_AP_COST,
            have: current_ap,
        });
    }

    // Check the item exists in inventory
    let has_item = if let Some(u) = state.player_units.iter().find(|u| u.id == unit_id) {
        u.inventory.iter().any(|i| i.name == item_name)
    } else if let Some(e) = state.enemy_units.iter().find(|e| e.id == unit_id) {
        e.inventory.iter().any(|i| i.name == item_name)
    } else {
        false
    };

    if !has_item {
        return Err(ActionError::ItemNotFound(item_name));
    }

    // Remove item from inventory and deduct AP
    if let Some(u) = state.player_units.iter_mut().find(|u| u.id == unit_id) {
        u.inventory.retain(|i| i.name != item_name);
        u.current_ap = u.current_ap.saturating_sub(USE_ITEM_AP_COST);
    } else if let Some(e) = state.enemy_units.iter_mut().find(|e| e.id == unit_id) {
        e.inventory.retain(|i| i.name != item_name);
        e.current_ap = e.current_ap.saturating_sub(USE_ITEM_AP_COST);
    }

    debug!(unit_id, item = %item_name, "Item used");

    Ok(ActionResult {
        success: true,
        ap_cost: USE_ITEM_AP_COST,
        effects: vec![ActionEffect::ItemUsed { item: item_name }],
    })
}

/// End turn action: set AP to 0.
fn execute_end_turn(
    state: &mut MissionState,
    unit_id: MercId,
) -> Result<ActionResult, ActionError> {
    deduct_ap_all(state, unit_id);
    debug!(unit_id, "Turn ended voluntarily");

    Ok(ActionResult {
        success: true,
        ap_cost: 0,
        effects: vec![ActionEffect::TurnEnded],
    })
}

/// Crouch action: toggle crouch stance, costs AP.
fn execute_crouch(
    state: &mut MissionState,
    unit_id: MercId,
    current_ap: u32,
) -> Result<ActionResult, ActionError> {
    if current_ap < CROUCH_AP_COST {
        return Err(ActionError::NotEnoughAp {
            needed: CROUCH_AP_COST,
            have: current_ap,
        });
    }

    deduct_ap(state, unit_id, CROUCH_AP_COST);
    debug!(unit_id, "Toggled crouch stance");

    Ok(ActionResult {
        success: true,
        ap_cost: CROUCH_AP_COST,
        effects: vec![ActionEffect::Crouched],
    })
}

/// Overwatch action: reserve all remaining AP for reaction fire.
fn execute_overwatch(
    _state: &mut MissionState,
    unit_id: MercId,
    current_ap: u32,
) -> Result<ActionResult, ActionError> {
    // Overwatch requires at least enough AP for one shot
    if current_ap < SHOOT_AP_COST {
        return Err(ActionError::NotEnoughAp {
            needed: SHOOT_AP_COST,
            have: current_ap,
        });
    }

    // Don't deduct AP — it's reserved for reaction fire.
    // The combat system checks overwatch status when enemies move.
    debug!(unit_id, reserved_ap = current_ap, "Entered overwatch");

    Ok(ActionResult {
        success: true,
        ap_cost: 0, // AP is reserved, not spent
        effects: vec![ActionEffect::EnteredOverwatch],
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Apply damage to a target unit (player or enemy). Returns true if killed.
fn apply_damage(state: &mut MissionState, target_id: MercId, damage: u32) -> bool {
    if let Some(u) = state.player_units.iter_mut().find(|u| u.id == target_id) {
        u.current_hp = u.current_hp.saturating_sub(damage);
        if u.current_hp == 0 {
            u.status = MercStatus::KIA;
            info!(id = target_id, name = %u.name, "Player merc killed");
            return true;
        }
    } else if let Some(e) = state.enemy_units.iter_mut().find(|e| e.id == target_id) {
        e.current_hp = e.current_hp.saturating_sub(damage);
        if e.current_hp == 0 {
            info!(id = target_id, name = %e.name, "Enemy unit killed");
            return true;
        }
    }
    false
}

/// Apply suppression to a target unit.
fn apply_suppression(state: &mut MissionState, target_id: MercId) {
    if let Some(u) = state.player_units.iter_mut().find(|u| u.id == target_id) {
        u.suppressed = true;
        debug!(id = target_id, name = %u.name, "Player merc suppressed");
    } else if let Some(e) = state.enemy_units.iter_mut().find(|e| e.id == target_id) {
        e.suppressed = true;
        debug!(id = target_id, name = %e.name, "Enemy unit suppressed");
    }
}

/// Get a unit's willpower stat.
fn get_unit_wil(state: &MissionState, id: MercId) -> Option<u32> {
    state
        .player_units
        .iter()
        .find(|u| u.id == id)
        .map(|u| u.wil.max(0) as u32)
        .or_else(|| {
            state
                .enemy_units
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.wil.max(0) as u32)
        })
}

/// Deduct a specific amount of AP from a unit.
fn deduct_ap(state: &mut MissionState, unit_id: MercId, amount: u32) {
    if let Some(u) = state.player_units.iter_mut().find(|u| u.id == unit_id) {
        u.current_ap = u.current_ap.saturating_sub(amount);
    } else if let Some(e) = state.enemy_units.iter_mut().find(|e| e.id == unit_id) {
        e.current_ap = e.current_ap.saturating_sub(amount);
    }
}

/// Set a unit's AP to zero (end of turn).
fn deduct_ap_all(state: &mut MissionState, unit_id: MercId) {
    if let Some(u) = state.player_units.iter_mut().find(|u| u.id == unit_id) {
        u.current_ap = 0;
    } else if let Some(e) = state.enemy_units.iter_mut().find(|e| e.id == unit_id) {
        e.current_ap = 0;
    }
}

// ---------------------------------------------------------------------------
// available_actions — what can this unit do right now?
// ---------------------------------------------------------------------------

/// Compute all actions currently available to a unit based on its AP, position,
/// weapon, and the tactical situation.
///
/// Used by the UI to show valid action buttons and by the AI to pick from.
pub fn available_actions(state: &MissionState, unit_id: MercId) -> Vec<Action> {
    let mut actions = Vec::new();

    // Find the unit's stats
    let (pos, ap) = if let Some(u) = state.player_units.iter().find(|u| u.id == unit_id) {
        if !u.can_act() {
            return actions;
        }
        (
            match u.position {
                Some(p) => p,
                None => return actions,
            },
            u.current_ap,
        )
    } else if let Some(e) = state.enemy_units.iter().find(|e| e.id == unit_id) {
        let merc = e.to_active_merc();
        if !merc.can_act() {
            return actions;
        }
        (
            match e.position {
                Some(p) => p,
                None => return actions,
            },
            e.current_ap,
        )
    } else {
        return actions;
    };

    let is_player = state.player_units.iter().any(|u| u.id == unit_id);

    // EndTurn is always available
    actions.push(Action::EndTurn);

    // Crouch if enough AP
    if ap >= CROUCH_AP_COST {
        actions.push(Action::Crouch);
    }

    // Reload if enough AP
    if ap >= RELOAD_AP_COST {
        actions.push(Action::Reload);
    }

    // Overwatch if enough AP for at least one shot
    if ap >= SHOOT_AP_COST {
        actions.push(Action::OverWatch);
    }

    // Movement: check cardinal neighbours as representative move targets.
    // The UI would expand this to show the full reachable overlay.
    if ap >= 2 {
        // Minimum possible move cost is 2 AP
        let directions = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        for (dx, dy) in directions {
            let target = TilePos {
                x: pos.x + dx,
                y: pos.y + dy,
            };
            if state.map.is_walkable(target) {
                actions.push(Action::Move(target));
            }
        }
    }

    // Shooting: check each visible enemy (or player, if AI)
    if ap >= SHOOT_AP_COST {
        let targets: Vec<(MercId, TilePos)> = if is_player {
            // Player shoots enemies
            state
                .enemy_units
                .iter()
                .filter(|e| e.current_hp > 0)
                .filter_map(|e| e.position.map(|p| (e.id, p)))
                .collect()
        } else {
            // Enemy shoots players
            state
                .player_units
                .iter()
                .filter(|u| u.is_alive())
                .filter_map(|u| u.position.map(|p| (u.id, p)))
                .collect()
        };

        for (target_id, target_pos) in targets {
            let range = tile_distance(pos, target_pos);
            if range <= DEFAULT_WEAPON_RANGE && has_line_of_sight(&state.map, pos, target_pos) {
                actions.push(Action::Shoot(target_id));
            }
        }
    }

    trace!(
        unit_id,
        count = actions.len(),
        "Computed available actions"
    );

    actions
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merc::{ActiveMerc, MercStatus, TilePos};
    use crate::mission_setup::{EnemyUnit, MissionState, MissionObjective};
    use crate::pathfinding::{TileInfo, TileMap};
    use crate::weather::Weather;
    use crate::game_state::MissionPhase;
    use ow_data::target::HitTable;
    use rand::rngs::mock::StepRng;

    fn test_hit_table() -> HitTable {
        let rows = vec![
            vec![98, 98, 98, 98, 98], // range 0: point blank
            vec![70, 80, 85, 90, 95], // range 1
            vec![40, 55, 65, 75, 85], // range 2
            vec![20, 35, 50, 60, 70], // range 3
            vec![5, 15, 30, 45, 55],  // range 4
        ];
        let json = serde_json::json!({ "rows": rows, "aux_sections": [] });
        serde_json::from_value(json).expect("HitTable deserialization")
    }

    fn test_player(id: MercId, x: i32, y: i32) -> ActiveMerc {
        ActiveMerc {
            id,
            name: format!("Player_{id}"),
            nickname: format!("P{id}"),
            exp: 40,
            str_stat: 50,
            agl: 50,
            wil: 45,
            wsk: 50, // WSK 50 -> col 10, capped at table width
            hhc: 40,
            tch: 30,
            enc: 300,
            base_aps: 30,
            dpr: 100,
            max_hp: 50,
            current_hp: 50,
            current_ap: 30,
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

    fn test_mission_state() -> MissionState {
        MissionState {
            player_units: vec![test_player(1, 5, 9)],
            enemy_units: vec![test_enemy(1001, 5, 2)],
            npcs: Vec::new(),
            map: TileMap::new_uniform(10, 10, TileInfo::open()),
            weather: Weather::Clear,
            turn_number: 1,
            phase: MissionPhase::Combat,
            objectives: vec![MissionObjective::EliminateAll],
            start_hour: 8,
            start_minute: 0,
            alert_level: 0,
        }
    }

    #[test]
    fn move_deducts_ap_and_updates_position() {
        let mut state = test_mission_state();
        let hit_table = test_hit_table();
        let mut rng = StepRng::new(0, 1);

        let target = TilePos { x: 6, y: 9 };
        let result = execute_action(&mut state, 1, Action::Move(target), &hit_table, &mut rng);

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        assert!(result.ap_cost > 0);

        // Position should be updated
        let player = state.player_units.iter().find(|u| u.id == 1).unwrap();
        assert_eq!(player.position, Some(target));

        // AP should be reduced
        assert!(player.current_ap < 30);
    }

    #[test]
    fn move_fails_with_no_path() {
        let mut state = test_mission_state();
        let hit_table = test_hit_table();
        let mut rng = StepRng::new(0, 1);

        // Target is out of bounds
        let target = TilePos { x: 100, y: 100 };
        let result = execute_action(&mut state, 1, Action::Move(target), &hit_table, &mut rng);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ActionError::NoPath(_)));
    }

    #[test]
    fn shoot_hit_deals_damage() {
        let mut state = test_mission_state();
        // Place units close together for guaranteed hit
        state.player_units[0].position = Some(TilePos { x: 5, y: 5 });
        state.enemy_units[0].position = Some(TilePos { x: 5, y: 6 }); // range 1

        let hit_table = test_hit_table();
        // StepRng(0, 0) -> roll = 0, which is always < any hit chance at range 1
        let mut rng = StepRng::new(0, 0);

        let result = execute_action(
            &mut state,
            1,
            Action::Shoot(1001),
            &hit_table,
            &mut rng,
        );

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        assert_eq!(result.ap_cost, SHOOT_AP_COST);

        // Enemy should have taken damage
        let enemy = state.enemy_units.iter().find(|e| e.id == 1001).unwrap();
        assert!(enemy.current_hp < 40, "Enemy should have taken damage");

        // Shooter AP should be reduced
        let player = state.player_units.iter().find(|u| u.id == 1).unwrap();
        assert_eq!(player.current_ap, 30 - SHOOT_AP_COST);
    }

    #[test]
    fn shoot_miss_may_suppress() {
        let mut state = test_mission_state();
        // Place units at range 4 with low WSK for likely miss
        state.player_units[0].position = Some(TilePos { x: 0, y: 0 });
        state.player_units[0].wsk = 0; // very low WSK
        state.enemy_units[0].position = Some(TilePos { x: 4, y: 0 }); // range 4
        state.enemy_units[0].wil = 1; // very low willpower -> easy to suppress

        let hit_table = test_hit_table();
        // High roll value to force a miss (WSK 0, range 4 -> 5% chance)
        // StepRng that produces a high value
        let mut rng = StepRng::new(99, 0); // gen_range(0..100) with seed 99

        let result = execute_action(
            &mut state,
            1,
            Action::Shoot(1001),
            &hit_table,
            &mut rng,
        );

        // The action itself succeeds (it was executed), but the shot misses
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.ap_cost, SHOOT_AP_COST);
        // We can't guarantee suppression with mock RNG, but AP should be spent
    }

    #[test]
    fn shoot_no_los_fails() {
        let mut state = test_mission_state();
        // Put a wall between the units
        state.player_units[0].position = Some(TilePos { x: 0, y: 5 });
        state.enemy_units[0].position = Some(TilePos { x: 4, y: 5 });
        // Wall at (2, 5)
        if let Some(tile) = state.map.get_mut(2, 5) {
            tile.terrain = crate::pathfinding::TerrainType::Wall;
            tile.walkable = false;
        }

        let hit_table = test_hit_table();
        let mut rng = StepRng::new(0, 1);

        let result = execute_action(
            &mut state,
            1,
            Action::Shoot(1001),
            &hit_table,
            &mut rng,
        );

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ActionError::NoLineOfSight(_)));
    }

    #[test]
    fn end_turn_sets_ap_to_zero() {
        let mut state = test_mission_state();
        let hit_table = test_hit_table();
        let mut rng = StepRng::new(0, 1);

        let result = execute_action(&mut state, 1, Action::EndTurn, &hit_table, &mut rng);

        assert!(result.is_ok());
        let player = state.player_units.iter().find(|u| u.id == 1).unwrap();
        assert_eq!(player.current_ap, 0);
    }

    #[test]
    fn not_enough_ap_for_shoot() {
        let mut state = test_mission_state();
        state.player_units[0].current_ap = 3; // less than SHOOT_AP_COST (8)

        let hit_table = test_hit_table();
        let mut rng = StepRng::new(0, 1);

        let result = execute_action(
            &mut state,
            1,
            Action::Shoot(1001),
            &hit_table,
            &mut rng,
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ActionError::NotEnoughAp { needed: 8, have: 3 }
        ));
    }

    #[test]
    fn available_actions_includes_end_turn() {
        let state = test_mission_state();
        let actions = available_actions(&state, 1);
        assert!(actions.contains(&Action::EndTurn));
    }

    #[test]
    fn available_actions_includes_shoot_when_in_range() {
        let mut state = test_mission_state();
        // Place enemy in LOS and range
        state.player_units[0].position = Some(TilePos { x: 5, y: 5 });
        state.enemy_units[0].position = Some(TilePos { x: 5, y: 6 });

        let actions = available_actions(&state, 1);
        assert!(actions.contains(&Action::Shoot(1001)));
    }
}
