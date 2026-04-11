//! Mercenary hiring and team management.
//!
//! # Hiring flow in Wages of War
//!
//! 1. Player visits the office "Hire Mercs" screen.
//! 2. The available roster is loaded from `MERCS.DAT` — only mercs with `AVAIL=1`
//!    and who aren't already on the team are shown.
//! 3. Player selects a merc. The three fee tiers from MERCS.DAT are:
//!    - **fee_hire** (fee1): One-time hiring cost, deducted immediately from funds.
//!    - **fee_bonus** (fee2): Per-mission bonus — paid to the merc after each mission.
//!    - **fee_death** (fee3): Death insurance — paid out if the merc is KIA.
//! 4. If the player can afford `fee_hire`, the merc joins the team.
//! 5. Maximum team size per mission is **8 mercs**.
//!
//! Firing a merc returns them to the available pool. No refund on the hiring fee.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, warn};

use ow_data::mercs::Mercenary;

use crate::economy::Ledger;
use crate::merc::ActiveMerc;

/// Maximum number of mercenaries allowed on the player's team.
pub const MAX_TEAM_SIZE: usize = 8;

/// Errors that can occur during hiring operations.
#[derive(Debug, Error)]
pub enum HiringError {
    #[error("mercenary not found: '{name}'")]
    MercNotFound { name: String },

    #[error("mercenary '{name}' is already hired")]
    AlreadyHired { name: String },

    #[error("insufficient funds to hire '{name}': need {cost}, have {available}")]
    InsufficientFunds {
        name: String,
        cost: i64,
        available: i64,
    },

    #[error("team is full ({max} mercs maximum)")]
    TeamFull { max: usize },
}

/// The mercenary hiring pool — wraps the parsed MERCS.DAT roster and tracks
/// which mercs have been hired onto the player's team.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiringPool {
    /// Full roster from MERCS.DAT (never mutated — availability is tracked
    /// by cross-referencing against hired names).
    roster: Vec<Mercenary>,
    /// Names of mercs currently on the player's team.
    hired_names: Vec<String>,
    /// Next runtime ID to assign when creating an ActiveMerc.
    next_id: u32,
}

impl HiringPool {
    /// Create a hiring pool from the parsed MERCS.DAT roster.
    pub fn new(roster: Vec<Mercenary>) -> Self {
        info!(roster_size = roster.len(), "Initialized hiring pool");
        Self {
            roster,
            hired_names: Vec::new(),
            next_id: 1,
        }
    }

    /// List all mercenaries that are available for hire:
    /// - `avail == 1` in the data file
    /// - Not already on the player's team
    pub fn available_mercs(&self) -> Vec<&Mercenary> {
        self.roster
            .iter()
            .filter(|m| m.avail == 1 && !self.hired_names.contains(&m.name))
            .collect()
    }

    /// Hire a mercenary by name.
    ///
    /// Deducts the merc's `fee_hire` from the ledger, creates an [`ActiveMerc`],
    /// and records the merc as hired in the pool.
    pub fn hire_merc(
        &mut self,
        merc_name: &str,
        ledger: &mut Ledger,
        team: &mut Vec<ActiveMerc>,
        turn_number: u32,
    ) -> Result<ActiveMerc, HiringError> {
        // Check team size limit.
        if team.len() >= MAX_TEAM_SIZE {
            warn!(max = MAX_TEAM_SIZE, "Cannot hire: team full");
            return Err(HiringError::TeamFull { max: MAX_TEAM_SIZE });
        }

        // Check if already hired.
        if self.hired_names.iter().any(|n| n == merc_name) {
            warn!(name = merc_name, "Cannot hire: already on team");
            return Err(HiringError::AlreadyHired {
                name: merc_name.to_string(),
            });
        }

        // Find the merc in the roster.
        let merc_data = self
            .roster
            .iter()
            .find(|m| m.name == merc_name && m.avail == 1)
            .ok_or_else(|| HiringError::MercNotFound {
                name: merc_name.to_string(),
            })?
            .clone();

        // Check funds.
        let cost = merc_data.fee_hire as i64;
        if !ledger.can_afford(cost) {
            warn!(
                name = merc_name,
                cost,
                balance = ledger.balance(),
                "Cannot hire: insufficient funds"
            );
            return Err(HiringError::InsufficientFunds {
                name: merc_name.to_string(),
                cost,
                available: ledger.balance(),
            });
        }

        // Deduct hiring fee.
        ledger
            .debit(cost, format!("Hired {}", merc_name), turn_number)
            .map_err(|_| HiringError::InsufficientFunds {
                name: merc_name.to_string(),
                cost,
                available: ledger.balance(),
            })?;

        // Create the active merc.
        let id = self.next_id;
        self.next_id += 1;
        let active = ActiveMerc::from_data(id, &merc_data);

        // Record as hired.
        self.hired_names.push(merc_name.to_string());
        team.push(active.clone());

        info!(
            name = merc_name,
            id,
            cost,
            balance = ledger.balance(),
            "Mercenary hired"
        );

        Ok(active)
    }

    /// Fire a mercenary, removing them from the team and returning them to the pool.
    ///
    /// No refund is given for the hiring fee. The merc becomes available for
    /// re-hiring (their `avail` flag in MERCS.DAT is unchanged).
    pub fn fire_merc(
        &mut self,
        merc_name: &str,
        team: &mut Vec<ActiveMerc>,
    ) -> Result<(), HiringError> {
        // Find and remove from team.
        let pos = team
            .iter()
            .position(|m| m.name == merc_name)
            .ok_or_else(|| HiringError::MercNotFound {
                name: merc_name.to_string(),
            })?;
        let removed = team.remove(pos);

        // Remove from hired tracking.
        self.hired_names.retain(|n| n != merc_name);

        info!(name = merc_name, id = removed.id, "Mercenary fired");
        Ok(())
    }

    /// Get the per-mission bonus fee (fee2) for a merc by name.
    /// Returns None if the merc is not in the roster.
    pub fn mission_fee(&self, merc_name: &str) -> Option<i32> {
        self.roster
            .iter()
            .find(|m| m.name == merc_name)
            .map(|m| m.fee_bonus)
    }

    /// Get the death insurance payout (fee3) for a merc by name.
    /// Returns None if the merc is not in the roster.
    pub fn death_insurance(&self, merc_name: &str) -> Option<i32> {
        self.roster
            .iter()
            .find(|m| m.name == merc_name)
            .map(|m| m.fee_death)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal Mercenary for testing without needing real data files.
    fn test_mercenary(name: &str, fee_hire: i32, avail: i32) -> Mercenary {
        Mercenary {
            name: name.to_string(),
            nickname: name[..1].to_string(),
            age: 30,
            height_feet: 5,
            height_inches: 10,
            weight: 180,
            nation: "USA".to_string(),
            rating: 50,
            dpr: 100,
            psg: 0,
            avail,
            exp: 40,
            str_stat: 50,
            agl: 50,
            wil: 45,
            wsk: 50,
            hhc: 40,
            tch: 30,
            enc: 300,
            aps: 38,
            fee_hire,
            fee_bonus: 5_000,
            fee_death: 50_000,
            mail: 0,
            biography: String::new(),
        }
    }

    #[test]
    fn hire_deducts_fee_and_adds_to_team() {
        let roster = vec![test_mercenary("Duke", 25_000, 1)];
        let mut pool = HiringPool::new(roster);
        let mut ledger = Ledger::new(100_000);
        let mut team = Vec::new();

        let merc = pool.hire_merc("Duke", &mut ledger, &mut team, 0).unwrap();
        assert_eq!(merc.name, "Duke");
        assert_eq!(ledger.balance(), 75_000);
        assert_eq!(team.len(), 1);
    }

    #[test]
    fn hire_insufficient_funds() {
        let roster = vec![test_mercenary("Duke", 25_000, 1)];
        let mut pool = HiringPool::new(roster);
        let mut ledger = Ledger::new(10_000);
        let mut team = Vec::new();

        let result = pool.hire_merc("Duke", &mut ledger, &mut team, 0);
        assert!(matches!(result, Err(HiringError::InsufficientFunds { .. })));
        assert_eq!(ledger.balance(), 10_000); // unchanged
        assert!(team.is_empty());
    }

    #[test]
    fn hire_already_hired() {
        let roster = vec![test_mercenary("Duke", 25_000, 1)];
        let mut pool = HiringPool::new(roster);
        let mut ledger = Ledger::new(100_000);
        let mut team = Vec::new();

        pool.hire_merc("Duke", &mut ledger, &mut team, 0).unwrap();
        let result = pool.hire_merc("Duke", &mut ledger, &mut team, 0);
        assert!(matches!(result, Err(HiringError::AlreadyHired { .. })));
    }

    #[test]
    fn hire_merc_not_found() {
        let roster = vec![test_mercenary("Duke", 25_000, 1)];
        let mut pool = HiringPool::new(roster);
        let mut ledger = Ledger::new(100_000);
        let mut team = Vec::new();

        let result = pool.hire_merc("Ghost", &mut ledger, &mut team, 0);
        assert!(matches!(result, Err(HiringError::MercNotFound { .. })));
    }

    #[test]
    fn hire_unavailable_merc_not_found() {
        // avail=0 mercs should not be hireable
        let roster = vec![test_mercenary("Locked", 10_000, 0)];
        let mut pool = HiringPool::new(roster);
        let mut ledger = Ledger::new(100_000);
        let mut team = Vec::new();

        let result = pool.hire_merc("Locked", &mut ledger, &mut team, 0);
        assert!(matches!(result, Err(HiringError::MercNotFound { .. })));
    }

    #[test]
    fn hire_team_full() {
        let roster: Vec<Mercenary> = (0..10)
            .map(|i| test_mercenary(&format!("Merc{}", i), 1_000, 1))
            .collect();
        let mut pool = HiringPool::new(roster);
        let mut ledger = Ledger::new(1_000_000);
        let mut team = Vec::new();

        // Hire up to the limit.
        for i in 0..MAX_TEAM_SIZE {
            pool.hire_merc(&format!("Merc{}", i), &mut ledger, &mut team, 0)
                .unwrap();
        }
        assert_eq!(team.len(), MAX_TEAM_SIZE);

        // The 9th hire should fail.
        let result = pool.hire_merc("Merc8", &mut ledger, &mut team, 0);
        assert!(matches!(result, Err(HiringError::TeamFull { .. })));
    }

    #[test]
    fn fire_returns_merc_to_pool() {
        let roster = vec![test_mercenary("Duke", 25_000, 1)];
        let mut pool = HiringPool::new(roster);
        let mut ledger = Ledger::new(100_000);
        let mut team = Vec::new();

        pool.hire_merc("Duke", &mut ledger, &mut team, 0).unwrap();
        assert_eq!(team.len(), 1);
        assert!(pool.available_mercs().is_empty());

        pool.fire_merc("Duke", &mut team).unwrap();
        assert!(team.is_empty());
        assert_eq!(pool.available_mercs().len(), 1);
    }

    #[test]
    fn fire_nonexistent_merc_errors() {
        let mut pool = HiringPool::new(Vec::new());
        let mut team = Vec::new();

        let result = pool.fire_merc("Nobody", &mut team);
        assert!(matches!(result, Err(HiringError::MercNotFound { .. })));
    }

    #[test]
    fn available_mercs_filters_correctly() {
        let roster = vec![
            test_mercenary("Alpha", 10_000, 1),
            test_mercenary("Beta", 15_000, 1),
            test_mercenary("Locked", 5_000, 0), // unavailable
        ];
        let mut pool = HiringPool::new(roster);
        let mut ledger = Ledger::new(100_000);
        let mut team = Vec::new();

        // All available mercs (excluding locked).
        assert_eq!(pool.available_mercs().len(), 2);

        // After hiring Alpha, only Beta is available.
        pool.hire_merc("Alpha", &mut ledger, &mut team, 0).unwrap();
        let avail = pool.available_mercs();
        assert_eq!(avail.len(), 1);
        assert_eq!(avail[0].name, "Beta");
    }
}
