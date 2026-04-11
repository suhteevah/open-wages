//! Mission initialization — converts parsed mission data + player team into
//! a fully playable `MissionState`.
//!
//! The flow:
//! 1. Roll weather from the mission's `WeatherTable`.
//! 2. Generate enemy units from `EnemyRating` entries (filtered by presence chance).
//! 3. Place player mercs at deployment positions.
//! 4. Build the tile map (placeholder — will be loaded from map data later).
//! 5. Return a `MissionState` ready for the first combat round.

use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace};

use ow_data::mission::{EnemyRating, EnemyWeapon, Mission};

use crate::game_state::MissionPhase;
use crate::merc::{ActiveMerc, InventoryItem, MercId, MercStatus, TilePos};
use crate::pathfinding::TileMap;
use crate::weather::{roll_weather_with_rng, Weather};

// ---------------------------------------------------------------------------
// EnemyUnit — generated combatants from EnemyRating data
// ---------------------------------------------------------------------------

/// An enemy combatant generated from the mission's `EnemyRating` chart.
///
/// Structurally similar to `ActiveMerc` but born from mission data rather than
/// the mercenary roster. The `rating` and `enemy_type` fields carry the original
/// classification from the .DAT file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnemyUnit {
    /// Runtime id (unique within the mission, offset from player ids).
    pub id: MercId,
    /// Display name (e.g. "Enemy_3", "Guard_7").
    pub name: String,
    /// Original rating tier from the mission file.
    pub rating: u8,
    /// Unit type (2=enemy, 3=NPC, 4=NPC variant, 7=vehicle/dog).
    pub enemy_type: u8,

    // -- Stats (copied from EnemyRating) --
    pub exp: i32,
    pub str_stat: i32,
    pub agl: i32,
    pub wil: i32,
    pub wsk: i32,
    pub hhc: i32,
    pub tch: i32,
    pub enc: i32,
    pub base_aps: i32,
    pub dpr: i32,

    // -- Mutable combat state --
    pub max_hp: u32,
    pub current_hp: u32,
    pub current_ap: u32,
    pub position: Option<TilePos>,
    pub inventory: Vec<InventoryItem>,
    pub suppressed: bool,
}

impl EnemyUnit {
    /// Generate an `EnemyUnit` from parsed mission data.
    ///
    /// Stats are copied directly from the `EnemyRating` row. Weapon loadout
    /// items are added to the inventory from `EnemyWeapon`.
    pub fn from_rating(id: MercId, rating: &EnemyRating, weapon: &EnemyWeapon) -> Self {
        let max_hp = (rating.str_ as u32).max(1);

        let mut inventory = Vec::new();

        // Add weapons as inventory items. Weapon indices are opaque for now —
        // the full weapon table lookup will come when ow-data parses WEAPON.DAT.
        if weapon.weapon1 >= 0 {
            inventory.push(InventoryItem {
                name: format!("Weapon_{}", weapon.weapon1),
                encumbrance: 50, // placeholder until weapon table is wired
            });
        }
        if weapon.weapon2 >= 0 {
            inventory.push(InventoryItem {
                name: format!("Weapon_{}", weapon.weapon2),
                encumbrance: 30,
            });
        }
        if weapon.weapon3 >= 0 {
            inventory.push(InventoryItem {
                name: format!("Item_{}", weapon.weapon3),
                encumbrance: 10,
            });
        }

        let label = match rating.enemy_type {
            2 => "Enemy",
            3 | 4 => "NPC",
            7 => "Vehicle",
            _ => "Unit",
        };

        debug!(
            id,
            name = format!("{}_{}", label, id),
            rating = rating.rating,
            hp = max_hp,
            aps = rating.aps,
            "Generated enemy unit from rating"
        );

        Self {
            id,
            name: format!("{}_{}", label, id),
            rating: rating.rating,
            enemy_type: rating.enemy_type,
            exp: rating.exp as i32,
            str_stat: rating.str_ as i32,
            agl: rating.agl as i32,
            wil: rating.wil as i32,
            wsk: rating.wsk as i32,
            hhc: rating.hhc as i32,
            tch: rating.tch as i32,
            enc: rating.enc as i32,
            base_aps: rating.aps as i32,
            dpr: rating.dpr as i32,
            max_hp,
            current_hp: max_hp,
            current_ap: rating.aps as u32,
            position: None,
            inventory,
            suppressed: false,
        }
    }

    /// Convert this enemy unit into an `ActiveMerc` for use in the combat system.
    ///
    /// The combat system works with `ActiveMerc` uniformly — this bridges the gap.
    pub fn to_active_merc(&self) -> ActiveMerc {
        ActiveMerc {
            id: self.id,
            name: self.name.clone(),
            nickname: self.name.clone(),
            exp: self.exp,
            str_stat: self.str_stat,
            agl: self.agl,
            wil: self.wil,
            wsk: self.wsk,
            hhc: self.hhc,
            tch: self.tch,
            enc: self.enc,
            base_aps: self.base_aps,
            dpr: self.dpr,
            max_hp: self.max_hp,
            current_hp: self.current_hp,
            current_ap: self.current_ap,
            status: MercStatus::OnMission,
            position: self.position,
            inventory: self.inventory.clone(),
            suppressed: self.suppressed,
            experience_gained: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// MissionObjective
// ---------------------------------------------------------------------------

/// A mission objective the player must complete (or fail).
///
/// The original game supports rescue, retrieval, and elimination objectives.
/// These are derived from the mission's `PrestigeConfig.mission_type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MissionObjective {
    /// Eliminate all hostile combatants.
    EliminateAll,
    /// Rescue a specific NPC (reach their tile and extract).
    Rescue { npc_id: MercId },
    /// Retrieve an item and extract.
    Retrieve { item_name: String },
    /// Reach the extraction zone.
    Extract,
}

// ---------------------------------------------------------------------------
// MissionState — the big one
// ---------------------------------------------------------------------------

/// Complete tactical mission state, ready for turn-by-turn play.
///
/// This is the "save snapshot" for a mission in progress. Everything needed
/// to resume play is here: unit positions, AP pools, weather, objectives, map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionState {
    /// Player-controlled mercenaries on the battlefield.
    pub player_units: Vec<ActiveMerc>,
    /// Enemy combatants generated from the mission's rating chart.
    pub enemy_units: Vec<EnemyUnit>,
    /// Non-player characters (civilians, rescuees, etc.).
    pub npcs: Vec<EnemyUnit>,
    /// The tactical tile map.
    pub map: TileMap,
    /// Current weather condition.
    pub weather: Weather,
    /// Current turn number (1-based, incremented each full round).
    pub turn_number: u32,
    /// Current phase of the mission.
    pub phase: MissionPhase,
    /// Mission objectives to track completion.
    pub objectives: Vec<MissionObjective>,
    /// Mission start hour (24h clock).
    pub start_hour: u8,
    /// Mission start minute.
    pub start_minute: u8,
    /// Alert level of the enemy garrison (0 = unaware, 1 = suspicious,
    /// 2 = alerted, 3 = full combat). Drives AI behavior escalation
    /// per the MOVES.DAT system.
    pub alert_level: u8,
}

// ---------------------------------------------------------------------------
// setup_mission — the main entry point
// ---------------------------------------------------------------------------

/// Initialize a complete mission from parsed data and the player's team.
///
/// # What this does
/// 1. Rolls weather from the mission's probability table.
/// 2. Filters enemy entries by `presence_chance` — each enemy has an independent
///    roll to determine if they actually appear on this playthrough.
/// 3. Separates enemies (type 2) from NPCs (types 3, 4, 7).
/// 4. Places player mercs at deployment positions (currently a line at the
///    south edge of the map; real deployment zones come from map data).
/// 5. Scatters enemies across the map (placeholder — real positions come from
///    map spawn points).
/// 6. Builds objectives from the mission's `prestige.mission_type`.
/// 7. Returns a `MissionState` ready for the deployment phase.
pub fn setup_mission<R: Rng>(
    mission_data: &Mission,
    team: &[ActiveMerc],
    map: TileMap,
    rng: &mut R,
) -> MissionState {
    info!(
        enemy_count = mission_data.enemy_count,
        npc_count = mission_data.npc_count,
        team_size = team.len(),
        "Setting up mission"
    );

    // -- 1. Roll weather --
    let weather = roll_weather_with_rng(&mission_data.weather, rng);
    info!(%weather, "Mission weather rolled");

    // -- 2. Generate enemies, filtering by presence chance --
    // Each EnemyRating row has a `presence_chance` (0-100). We roll for each
    // independently — this means the actual enemy count varies per playthrough,
    // matching the original game's randomized garrison sizes.
    let mut enemy_units: Vec<EnemyUnit> = Vec::new();
    let mut npcs: Vec<EnemyUnit> = Vec::new();

    // IDs for generated units start after the highest player merc id to avoid collisions.
    let max_player_id = team.iter().map(|m| m.id).max().unwrap_or(0);
    let mut next_id = max_player_id + 1000; // generous offset

    for (i, rating) in mission_data.enemy_ratings.iter().enumerate() {
        // Roll presence
        let roll: u8 = rng.gen_range(0..100);
        if roll >= rating.presence_chance {
            trace!(
                index = i,
                roll,
                chance = rating.presence_chance,
                "Enemy did not spawn (presence roll failed)"
            );
            continue;
        }

        let weapon = mission_data
            .enemy_weapons
            .get(i)
            .cloned()
            .unwrap_or(EnemyWeapon {
                weapon1: -1,
                weapon2: -1,
                ammo1: 0,
                ammo2: 0,
                weapon3: -1,
                extra: -1,
            });

        let unit = EnemyUnit::from_rating(next_id, rating, &weapon);
        next_id += 1;

        // Separate enemies from NPCs based on enemy_type
        match rating.enemy_type {
            2 => {
                trace!(id = unit.id, name = %unit.name, "Spawned enemy combatant");
                enemy_units.push(unit);
            }
            3 | 4 | 7 => {
                trace!(id = unit.id, name = %unit.name, "Spawned NPC");
                npcs.push(unit);
            }
            other => {
                debug!(enemy_type = other, "Unknown enemy_type, treating as enemy");
                enemy_units.push(unit);
            }
        }
    }

    info!(
        enemies_spawned = enemy_units.len(),
        npcs_spawned = npcs.len(),
        "Enemy generation complete"
    );

    // -- 3. Place player mercs at deployment positions --
    // Deployment zone: bottom edge of the map, spaced 2 tiles apart.
    // Real deployment zones will come from parsed map data.
    let mut player_units: Vec<ActiveMerc> = team.to_vec();
    let deploy_y = (map.height as i32) - 1;
    for (i, merc) in player_units.iter_mut().enumerate() {
        merc.status = MercStatus::OnMission;
        merc.position = Some(TilePos {
            x: (i as i32) * 2,
            y: deploy_y,
        });
        debug!(
            id = merc.id,
            name = %merc.name,
            pos = ?merc.position,
            "Placed player merc at deployment position"
        );
    }

    // -- 4. Scatter enemies across the map --
    // Placeholder: distribute enemies across the top half of the map.
    // Real positions will come from map spawn point data.
    let enemy_zone_max_y = (map.height / 2) as i32;
    for enemy in enemy_units.iter_mut() {
        let x = rng.gen_range(0..map.width as i32);
        let y = rng.gen_range(0..enemy_zone_max_y.max(1));
        enemy.position = Some(TilePos { x, y });
        trace!(
            id = enemy.id,
            pos = ?enemy.position,
            "Placed enemy unit"
        );
        // Mark the tile as occupied
        if let Some(tile) = map.get(x, y) {
            // Note: we can't mutate through a shared ref — the caller should
            // update occupancy after setup. This is intentional: map occupancy
            // is managed by the action system during play.
            let _ = tile;
        }
    }

    // Place NPCs similarly but in the middle of the map
    for npc in npcs.iter_mut() {
        let x = rng.gen_range(0..map.width as i32);
        let y =
            rng.gen_range(enemy_zone_max_y.max(0)..((map.height as i32).max(enemy_zone_max_y + 1)));
        npc.position = Some(TilePos { x, y });
        trace!(id = npc.id, pos = ?npc.position, "Placed NPC");
    }

    // -- 5. Build objectives from mission type --
    let objectives = match mission_data.prestige.mission_type {
        1 => {
            // Rescue mission: rescue NPCs + extract
            let mut objs: Vec<MissionObjective> = npcs
                .iter()
                .map(|npc| MissionObjective::Rescue { npc_id: npc.id })
                .collect();
            objs.push(MissionObjective::Extract);
            objs
        }
        2 => {
            // Retrieval mission: grab item + extract
            vec![
                MissionObjective::Retrieve {
                    item_name: "objective_item".to_string(),
                },
                MissionObjective::Extract,
            ]
        }
        _ => {
            // Default: eliminate all enemies + extract
            vec![MissionObjective::EliminateAll, MissionObjective::Extract]
        }
    };

    debug!(
        objective_count = objectives.len(),
        mission_type = mission_data.prestige.mission_type,
        "Mission objectives set"
    );

    let state = MissionState {
        player_units,
        enemy_units,
        npcs,
        map,
        weather,
        turn_number: 0,
        phase: MissionPhase::Deployment,
        objectives,
        start_hour: mission_data.start_hour,
        start_minute: mission_data.start_minute,
        alert_level: 0, // enemies start unaware
    };

    info!(
        players = state.player_units.len(),
        enemies = state.enemy_units.len(),
        npcs = state.npcs.len(),
        weather = %state.weather,
        "Mission setup complete"
    );

    state
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinding::TileInfo;
    use ow_data::mission::*;
    use rand::rngs::mock::StepRng;

    /// Build a minimal mission definition for testing.
    fn test_mission(enemy_count: usize) -> Mission {
        let mut ratings = Vec::new();
        let mut weapons = Vec::new();
        for i in 0..enemy_count {
            ratings.push(EnemyRating {
                rating: 5,
                dpr: 100,
                exp: 20,
                str_: 40,
                agl: 50,
                wil: 30,
                wsk: 45,
                hhc: 30,
                tch: 20,
                enc: 200,
                aps: 20,
                presence_chance: 100, // guaranteed spawn
                enemy_type: 2,        // enemy combatant
            });
            weapons.push(EnemyWeapon {
                weapon1: i as i8,
                weapon2: -1,
                ammo1: 3,
                ammo2: 0,
                weapon3: -1,
                extra: -1,
            });
        }

        Mission {
            animation_files: AnimationFiles {
                good_guys: "test.cor".into(),
                bad_guys: "test.cor".into(),
                dogs: None,
                npc1: None,
                npc2: None,
                npc3_vhc1: None,
                npc4_vhc2: None,
            },
            contract: ContractTerms {
                date_day: 1,
                date_year: 1996,
                from: "TestClient".into(),
                terms: "Test mission".into(),
                bonus_text: "No bonus".into(),
                advance: 10000,
                bonus: 5000,
                deadline_day: 30,
                deadline_year: 1996,
            },
            negotiation: Negotiation {
                advance: [12000, 14000, 16000, 18000],
                bonus: [6000, 7000, 8000, 9000],
                deadline: [35, 40, 45, 50],
                chance: [80, 60, 40, 20],
                counter_values: [0; 8],
                counter_advance: [0; 8],
                counter_bonus: [0; 8],
                counter_deadline: [0; 8],
            },
            prestige: PrestigeConfig {
                mission_type: 3, // elimination
                entrance: 0,
                num_maps: 1,
                success1: 10,
                success2: 0,
                wia: -1,
                mia: -2,
                kia: -3,
            },
            intelligence: IntelligenceConfig {
                tiers: [
                    IntelTier {
                        name: "Low".into(),
                        cost: 100,
                        per_item: 10,
                    },
                    IntelTier {
                        name: "Mid".into(),
                        cost: 200,
                        per_item: 20,
                    },
                    IntelTier {
                        name: "High".into(),
                        cost: 500,
                        per_item: 50,
                    },
                ],
                men: enemy_count as u8,
                exp: 3,
                fire_power: 5,
                success: 70,
                casualties: 2,
                scene_type: 0,
                attachments: 0,
            },
            enemy_count: enemy_count as u16,
            npc_count: 0,
            enemy_ratings: ratings,
            enemy_weapons: weapons,
            preloaded_equipment: EquipmentCounts {
                weapons: 0,
                ammo: 0,
                equipment: 0,
            },
            recommended_equipment: EquipmentCounts {
                weapons: 0,
                ammo: 0,
                equipment: 0,
            },
            recommended_item: None,
            start_hour: 8,
            start_minute: 0,
            weather: WeatherTable {
                clear: 80,
                foggy: 5,
                overcast: 10,
                light_rain: 5,
                heavy_rain: 0,
                storm: 0,
            },
            travel: TravelTable {
                cost1: 1000,
                cost2: 2000,
                cost3: 5000,
                days1: 5,
                days2: 3,
                days3: 1,
            },
            special: SpecialConfig {
                turns: 0,
                special_type: 0,
                item: 0,
                damage: 0,
                damage_message: None,
            },
        }
    }

    /// Build a test merc.
    fn test_merc(id: MercId) -> ActiveMerc {
        ActiveMerc {
            id,
            name: format!("Merc_{id}"),
            nickname: format!("M{id}"),
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
            status: MercStatus::Hired,
            position: None,
            inventory: Vec::new(),
            suppressed: false,
            experience_gained: 0,
        }
    }

    #[test]
    fn setup_correct_enemy_count() {
        let mission = test_mission(5);
        let team = vec![test_merc(1), test_merc(2)];
        let map = TileMap::new_uniform(20, 20, TileInfo::open());
        let mut rng = StepRng::new(0, 1); // deterministic

        let state = setup_mission(&mission, &team, map, &mut rng);

        // All 5 enemies have presence_chance=100, so all should spawn.
        assert_eq!(state.enemy_units.len(), 5);
        assert_eq!(state.npcs.len(), 0);
    }

    #[test]
    fn setup_weather_rolled() {
        let mission = test_mission(1);
        let team = vec![test_merc(1)];
        let map = TileMap::new_uniform(20, 20, TileInfo::open());
        // StepRng(0, 0) always returns 0 -> Clear with this table
        let mut rng = StepRng::new(0, 0);

        let state = setup_mission(&mission, &team, map, &mut rng);
        assert_eq!(state.weather, Weather::Clear);
    }

    #[test]
    fn setup_units_placed() {
        let mission = test_mission(3);
        let team = vec![test_merc(1), test_merc(2)];
        let map = TileMap::new_uniform(20, 20, TileInfo::open());
        let mut rng = StepRng::new(0, 1);

        let state = setup_mission(&mission, &team, map, &mut rng);

        // Player mercs should have positions set
        for merc in &state.player_units {
            assert!(merc.position.is_some(), "Player merc should be placed");
            assert_eq!(merc.status, MercStatus::OnMission);
        }

        // Enemies should have positions set
        for enemy in &state.enemy_units {
            assert!(enemy.position.is_some(), "Enemy should be placed");
        }
    }

    #[test]
    fn setup_starts_in_deployment_phase() {
        let mission = test_mission(1);
        let team = vec![test_merc(1)];
        let map = TileMap::new_uniform(10, 10, TileInfo::open());
        let mut rng = StepRng::new(0, 1);

        let state = setup_mission(&mission, &team, map, &mut rng);

        assert_eq!(state.phase, MissionPhase::Deployment);
        assert_eq!(state.turn_number, 0);
        assert_eq!(state.alert_level, 0);
    }

    #[test]
    fn setup_elimination_objectives() {
        let mission = test_mission(2); // mission_type = 3 -> eliminate
        let team = vec![test_merc(1)];
        let map = TileMap::new_uniform(10, 10, TileInfo::open());
        let mut rng = StepRng::new(0, 1);

        let state = setup_mission(&mission, &team, map, &mut rng);

        assert!(state.objectives.len() >= 2);
        assert!(matches!(
            state.objectives[0],
            MissionObjective::EliminateAll
        ));
        assert!(matches!(state.objectives[1], MissionObjective::Extract));
    }

    #[test]
    fn presence_chance_filters_enemies() {
        // Create a mission where some enemies have 0% presence chance
        // (guaranteed to NOT spawn) to verify filtering works.
        let mut mission = test_mission(6);
        // First 3 enemies: guaranteed spawn
        // Last 3 enemies: guaranteed NO spawn
        for (i, rating) in mission.enemy_ratings.iter_mut().enumerate() {
            if i >= 3 {
                rating.presence_chance = 0; // will never pass `roll < 0`
            }
        }

        let team = vec![test_merc(1)];
        let map = TileMap::new_uniform(20, 20, TileInfo::open());
        let mut rng = StepRng::new(0, 1);

        let state = setup_mission(&mission, &team, map, &mut rng);

        // Only the first 3 enemies (presence_chance=100) should spawn.
        // The last 3 (presence_chance=0) should be filtered out.
        assert_eq!(
            state.enemy_units.len(),
            3,
            "Only enemies with presence_chance > 0 should spawn"
        );
    }
}
