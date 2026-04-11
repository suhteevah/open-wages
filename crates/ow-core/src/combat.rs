//! Initiative-based combat system.
//!
//! # Key Design: NOT IGOUGO
//!
//! Wages of War uses an **initiative-based** turn system, NOT "I Go, You Go"
//! (IGOUGO). All units — player mercs, enemies, and neutrals — are sorted
//! into a single initiative queue each round. A fast enemy acts before a slow
//! player merc. This creates tension because you can't safely move all your
//! units before the enemy responds.
//!
//! # How Initiative Works
//!
//! Each unit's initiative score is computed from their stats (experience +
//! willpower). Higher initiative = acts earlier in the round. Ties are broken
//! by unit ID (lower ID acts first, giving a slight edge to earlier-hired mercs).
//!
//! # Suppression
//!
//! When a unit takes incoming fire (even misses), it can become **suppressed**.
//! Suppression halves the unit's initiative for the next round, pushing it
//! later in the turn order. This models the real-world effect of suppressive
//! fire — even if you don't hit anyone, you slow them down and disrupt their plans.

use std::collections::BinaryHeap;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace};

use crate::merc::{ActiveMerc, MercId};

/// Which side a unit fights for.
///
/// Neutral units (civilians, mission-critical NPCs) are in the initiative queue
/// but generally don't attack. They can still be killed — friendly fire on
/// neutrals typically fails the mission or reduces reputation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Faction {
    Player,
    Enemy,
    /// Non-combatant. Present in the initiative queue but typically only moves
    /// (flee behavior). Killing neutrals has consequences.
    Neutral,
}

/// A combat unit — wraps an `ActiveMerc` with faction info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatUnit {
    pub merc: ActiveMerc,
    pub faction: Faction,
}

/// An entry in the initiative priority queue.
///
/// Rust's `BinaryHeap` is a max-heap, so higher values are popped first.
/// This naturally fits our needs: higher initiative = acts first.
///
/// Tie-breaking uses reversed unit_id comparison (`other.unit_id.cmp(&self.unit_id)`)
/// so that the LOWER id wins ties. This gives a consistent, deterministic turn
/// order when two units have equal initiative — earlier-hired mercs (lower IDs)
/// get a slight edge, which matches the original game's behavior.
#[derive(Debug, Clone, Eq, PartialEq)]
struct InitiativeEntry {
    initiative: u32,
    unit_id: MercId,
}

impl Ord for InitiativeEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Primary sort: higher initiative acts first (natural max-heap order).
        // Secondary sort: lower unit_id wins ties (reversed comparison).
        self.initiative
            .cmp(&other.initiative)
            .then_with(|| other.unit_id.cmp(&self.unit_id))
    }
}

impl PartialOrd for InitiativeEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Current phase within a combat round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatPhase {
    /// Waiting for round to begin.
    PreRound,
    /// Units are taking turns in initiative order.
    InProgress,
    /// All units have acted; ready for next round.
    RoundComplete,
}

/// Top-level combat state managing the initiative queue and turn flow.
///
/// The combat loop works as follows:
/// 1. Call `begin_round()` to increment the round counter, recalculate all
///    initiative values, and build the priority queue.
/// 2. Call `next_unit()` repeatedly to pop units from the queue in initiative order.
/// 3. After each unit acts, call `end_turn()` to signal it's done.
/// 4. When `next_unit()` returns `None`, the round is complete — go to step 1.
///
/// Because factions are interleaved in the queue, enemy and player turns
/// alternate freely based on initiative scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatState {
    /// All participating units. This is a flat list, not faction-separated,
    /// because the initiative system treats all factions uniformly.
    pub units: Vec<CombatUnit>,
    /// Current round number (1-based). Incremented at the start of each round.
    pub turn_number: u32,
    /// Id of the unit currently acting, if any. `None` between turns.
    pub current_unit_id: Option<MercId>,
    /// Current phase of the round.
    pub phase: CombatPhase,
    /// The initiative queue. This is a max-heap — highest initiative is popped
    /// first. Skipped during serialization because it's rebuilt fresh each round
    /// from unit stats (which may change due to suppression, injuries, etc.).
    #[serde(skip)]
    queue: BinaryHeap<InitiativeEntry>,
}

impl CombatState {
    /// Create a new combat with the given units. Round starts at 0 (call `begin_round` first).
    pub fn new(units: Vec<CombatUnit>) -> Self {
        info!(unit_count = units.len(), "Initializing combat state");
        Self {
            units,
            turn_number: 0,
            current_unit_id: None,
            phase: CombatPhase::PreRound,
            queue: BinaryHeap::new(),
        }
    }

    /// Begin a new combat round: increment turn counter, recalculate all
    /// initiatives, build the priority queue.
    ///
    /// Initiative is recalculated every round because it can change mid-combat:
    /// suppression halves initiative, injuries reduce stats, and experience
    /// gained in combat can increase it. This means turn order can shift between
    /// rounds — a suppressed unit may act last this round but first next round
    /// once the suppression wears off.
    pub fn begin_round(&mut self) {
        self.turn_number += 1;
        self.phase = CombatPhase::InProgress;
        self.current_unit_id = None;
        self.queue.clear();

        // Rebuild the initiative queue from scratch. We only queue living units
        // and reset their AP to full at the start of each round.
        for unit in &mut self.units {
            if unit.merc.is_alive() {
                unit.merc.reset_ap();
                // initiative() factors in suppression — a suppressed unit's
                // initiative is halved, pushing it later in the turn order.
                let init = unit.merc.initiative();
                trace!(
                    id = unit.merc.id,
                    name = %unit.merc.name,
                    faction = ?unit.faction,
                    initiative = init,
                    "Queued unit"
                );
                self.queue.push(InitiativeEntry {
                    initiative: init,
                    unit_id: unit.merc.id,
                });
            }
        }

        info!(
            round = self.turn_number,
            queued = self.queue.len(),
            "Started combat round"
        );
    }

    /// Pop the next unit from the initiative queue.
    ///
    /// Returns `None` when all units have acted this round (sets phase to `RoundComplete`).
    ///
    /// Units can die mid-round (killed by another unit's action), so we skip
    /// any dead units still in the queue. We also skip units that can no longer
    /// act (0 AP remaining). This is why we check `is_alive()` and `can_act()`
    /// here rather than filtering at queue-build time.
    pub fn next_unit(&mut self) -> Option<MercId> {
        // Skip dead or exhausted units that were still in the queue when killed/drained.
        while let Some(entry) = self.queue.pop() {
            if let Some(unit) = self.find_unit(entry.unit_id) {
                if unit.merc.is_alive() && unit.merc.can_act() {
                    debug!(
                        id = entry.unit_id,
                        initiative = entry.initiative,
                        "Next unit to act"
                    );
                    self.current_unit_id = Some(entry.unit_id);
                    return Some(entry.unit_id);
                }
            }
        }

        debug!(round = self.turn_number, "All units have acted");
        self.phase = CombatPhase::RoundComplete;
        self.current_unit_id = None;
        None
    }

    /// Signal that the current unit has finished its turn.
    ///
    /// Forces the unit's AP to 0 so `can_act()` returns false if the unit
    /// somehow re-enters the queue (defensive measure). In the original game,
    /// ending your turn is irreversible — you can't "undo" and keep acting.
    pub fn end_turn(&mut self) {
        if let Some(id) = self.current_unit_id {
            if let Some(unit) = self.find_unit_mut(id) {
                // Zero out AP to prevent the unit from acting again this round.
                unit.merc.current_ap = 0;
                trace!(id, "Unit ended turn");
            }
        }
        self.current_unit_id = None;
    }

    /// Look up a unit by id (immutable).
    pub fn find_unit(&self, id: MercId) -> Option<&CombatUnit> {
        self.units.iter().find(|u| u.merc.id == id)
    }

    /// Look up a unit by id (mutable).
    pub fn find_unit_mut(&mut self, id: MercId) -> Option<&mut CombatUnit> {
        self.units.iter_mut().find(|u| u.merc.id == id)
    }

    /// Get all living units of a given faction.
    pub fn living_units(&self, faction: Faction) -> Vec<&CombatUnit> {
        self.units
            .iter()
            .filter(|u| u.faction == faction && u.merc.is_alive())
            .collect()
    }

    /// Check if combat is over (one side eliminated).
    ///
    /// Combat ends when either all player mercs or all enemies are dead.
    /// Neutral units don't count — a battle can end with civilians still alive.
    /// The caller checks which side survived to determine victory vs defeat.
    pub fn is_combat_over(&self) -> bool {
        let players_alive = self.living_units(Faction::Player).len();
        let enemies_alive = self.living_units(Faction::Enemy).len();
        // Either side being wiped out ends combat. Both being zero (mutual
        // annihilation) also ends it — the game treats this as a loss.
        players_alive == 0 || enemies_alive == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merc::{MercStatus, TilePos};

    fn make_unit(id: MercId, faction: Faction, exp: i32, wil: i32) -> CombatUnit {
        CombatUnit {
            merc: ActiveMerc {
                id,
                name: format!("Unit_{id}"),
                nickname: format!("U{id}"),
                exp,
                str_stat: 50,
                agl: 50,
                wil,
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
                position: Some(TilePos { x: 0, y: 0 }),
                inventory: Vec::new(),
                suppressed: false,
                experience_gained: 0,
            },
            faction,
        }
    }

    #[test]
    fn initiative_order_highest_first() {
        let units = vec![
            make_unit(1, Faction::Player, 20, 20), // init 40
            make_unit(2, Faction::Enemy, 50, 40),  // init 90
            make_unit(3, Faction::Player, 30, 30), // init 60
        ];

        let mut combat = CombatState::new(units);
        combat.begin_round();

        assert_eq!(combat.next_unit(), Some(2)); // 90
        combat.end_turn();
        assert_eq!(combat.next_unit(), Some(3)); // 60
        combat.end_turn();
        assert_eq!(combat.next_unit(), Some(1)); // 40
        combat.end_turn();
        assert_eq!(combat.next_unit(), None);
        assert_eq!(combat.phase, CombatPhase::RoundComplete);
    }

    #[test]
    fn dead_units_skipped() {
        let units = vec![
            make_unit(1, Faction::Player, 50, 50), // init 100
            make_unit(2, Faction::Enemy, 40, 40),  // init 80
        ];

        let mut combat = CombatState::new(units);

        // Kill unit 1 before the round
        combat.units[0].merc.current_hp = 0;
        combat.units[0].merc.status = MercStatus::KIA;

        combat.begin_round();
        assert_eq!(combat.next_unit(), Some(2));
        combat.end_turn();
        assert_eq!(combat.next_unit(), None);
    }

    #[test]
    fn mixed_factions_interleaved() {
        let units = vec![
            make_unit(1, Faction::Player, 30, 30), // 60
            make_unit(2, Faction::Enemy, 50, 50),  // 100
            make_unit(3, Faction::Player, 40, 40), // 80
            make_unit(4, Faction::Enemy, 35, 35),  // 70
        ];

        let mut combat = CombatState::new(units);
        combat.begin_round();

        // Should interleave: Enemy(100), Player(80), Enemy(70), Player(60)
        let order: Vec<MercId> = std::iter::from_fn(|| {
            let id = combat.next_unit()?;
            combat.end_turn();
            Some(id)
        })
        .collect();

        assert_eq!(order, vec![2, 3, 4, 1]);
    }

    #[test]
    fn combat_over_detection() {
        let units = vec![
            make_unit(1, Faction::Player, 30, 30),
            make_unit(2, Faction::Enemy, 30, 30),
        ];

        let mut combat = CombatState::new(units);
        assert!(!combat.is_combat_over());

        combat.units[1].merc.current_hp = 0;
        combat.units[1].merc.status = MercStatus::KIA;
        assert!(combat.is_combat_over());
    }

    #[test]
    fn multiple_rounds() {
        let units = vec![
            make_unit(1, Faction::Player, 30, 30),
            make_unit(2, Faction::Enemy, 40, 40),
        ];

        let mut combat = CombatState::new(units);

        combat.begin_round();
        assert_eq!(combat.turn_number, 1);
        while combat.next_unit().is_some() {
            combat.end_turn();
        }

        combat.begin_round();
        assert_eq!(combat.turn_number, 2);
        assert_eq!(combat.phase, CombatPhase::InProgress);
        assert_eq!(combat.next_unit(), Some(2)); // higher init goes first again
    }
}
