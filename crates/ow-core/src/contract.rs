//! Contract negotiation system.
//!
//! # How contract negotiation works in Wages of War
//!
//! 1. A mission's `ContractTerms` from MSSN*.DAT defines the initial offer:
//!    client name, objective text, advance payment, bonus, and deadline.
//!
//! 2. The player can **accept** the initial offer, or make up to **4 counter-offers**.
//!    Each counter-offer targets one aspect:
//!    - **Advance**: Request more money upfront.
//!    - **Bonus**: Request a higher success bonus.
//!    - **Deadline**: Request more time to complete the mission.
//!
//! 3. The `Negotiation` data from the mission file defines:
//!    - Four escalating counter-offer amounts for each aspect (advance, bonus, deadline).
//!    - Four acceptance probabilities (percent, descending). Each successive counter
//!      has a lower chance of being accepted. Typical values: 80%, 60%, 40%, 20%.
//!
//! 4. On each counter-offer round:
//!    - A random roll determines if the client accepts the player's demand.
//!    - If accepted, the contract terms are updated with the better values.
//!    - If rejected, the client may counter back with their own modified terms
//!      (using the `counter_values` / `counter_advance` / etc. from Negotiation data).
//!
//! 5. After 4 rounds, the player must accept or walk away.
//!
//! 6. When accepted, the `advance` is immediately credited to the player's ledger.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

use ow_data::mission::Negotiation;

use crate::economy::Ledger;

/// Which aspect of the contract the player is pushing on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NegotiationAspect {
    /// Request a higher advance payment.
    Advance,
    /// Request a higher mission completion bonus.
    Bonus,
    /// Request a longer deadline (more days to complete).
    Deadline,
}

/// Errors that can occur during contract negotiation.
#[derive(Debug, Error)]
pub enum ContractError {
    #[error("no counter-offers remaining (max {max} rounds)")]
    NoCountersRemaining { max: u32 },

    #[error("contract already accepted")]
    AlreadyAccepted,

    #[error("no active contract offer to accept")]
    NoActiveOffer,

    #[error("counter-offer rejected by client")]
    CounterRejected,
}

/// The current state of a contract offer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractOffer {
    /// Client / organization name.
    pub from: String,
    /// Mission objective description.
    pub terms: String,
    /// Cash advance paid on acceptance.
    pub advance: i64,
    /// Bonus paid on mission success.
    pub bonus: i64,
    /// Number of days allowed to complete the mission.
    pub deadline_days: u32,
}

/// Tracks the state of an ongoing contract negotiation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationState {
    /// The current (possibly modified) contract offer.
    pub current_offer: ContractOffer,
    /// How many counter-offer rounds have been used (0-3 = can still counter, 4 = done).
    pub counter_round: u32,
    /// Maximum number of counter-offer rounds allowed.
    pub max_rounds: u32,
    /// Whether the contract has been accepted.
    pub accepted: bool,
}

impl NegotiationState {
    /// Create a new negotiation from initial contract terms.
    pub fn new(from: String, terms: String, advance: i64, bonus: i64, deadline_days: u32) -> Self {
        info!(
            from = %from,
            advance,
            bonus,
            deadline_days,
            "Starting contract negotiation"
        );
        Self {
            current_offer: ContractOffer {
                from,
                terms,
                advance,
                bonus,
                deadline_days,
            },
            counter_round: 0,
            max_rounds: 4,
            accepted: false,
        }
    }

    /// Attempt a counter-offer on the given aspect.
    ///
    /// Uses the negotiation data from MSSN*.DAT to determine the new values
    /// and a random roll to decide acceptance. The `roll` parameter (0-99)
    /// should come from the RNG; it's explicit here for testability.
    ///
    /// Returns the updated offer on success, or `ContractError::CounterRejected`
    /// if the client refuses.
    pub fn counter_offer(
        &mut self,
        negotiation: &Negotiation,
        aspect: NegotiationAspect,
        roll: u8,
    ) -> Result<&ContractOffer, ContractError> {
        if self.accepted {
            return Err(ContractError::AlreadyAccepted);
        }
        if self.counter_round >= self.max_rounds {
            return Err(ContractError::NoCountersRemaining {
                max: self.max_rounds,
            });
        }

        let round = self.counter_round as usize;
        let chance = negotiation.chance[round];

        debug!(
            round = self.counter_round,
            aspect = ?aspect,
            chance,
            roll,
            "Processing counter-offer"
        );

        // The roll must be strictly less than the chance percentage to succeed.
        if roll >= chance {
            self.counter_round += 1;
            warn!(
                round = self.counter_round - 1,
                roll, chance, "Counter-offer rejected"
            );
            return Err(ContractError::CounterRejected);
        }

        // Counter accepted — update the relevant aspect with the escalated value.
        match aspect {
            NegotiationAspect::Advance => {
                let new_advance = negotiation.advance[round] as i64;
                info!(
                    old = self.current_offer.advance,
                    new = new_advance,
                    "Advance counter-offer accepted"
                );
                self.current_offer.advance = new_advance;
            }
            NegotiationAspect::Bonus => {
                let new_bonus = negotiation.bonus[round] as i64;
                info!(
                    old = self.current_offer.bonus,
                    new = new_bonus,
                    "Bonus counter-offer accepted"
                );
                self.current_offer.bonus = new_bonus;
            }
            NegotiationAspect::Deadline => {
                let new_deadline = negotiation.deadline[round] as u32;
                info!(
                    old = self.current_offer.deadline_days,
                    new = new_deadline,
                    "Deadline counter-offer accepted"
                );
                self.current_offer.deadline_days = new_deadline;
            }
        }

        self.counter_round += 1;
        Ok(&self.current_offer)
    }

    /// Accept the current contract offer. Credits the advance to the player's ledger.
    pub fn accept_contract(
        &mut self,
        ledger: &mut Ledger,
        turn_number: u32,
    ) -> Result<(), ContractError> {
        if self.accepted {
            return Err(ContractError::AlreadyAccepted);
        }

        let advance = self.current_offer.advance;
        info!(
            from = %self.current_offer.from,
            advance,
            bonus = self.current_offer.bonus,
            deadline = self.current_offer.deadline_days,
            "Contract accepted"
        );

        ledger.credit(
            advance,
            format!("Contract advance from {}", self.current_offer.from),
            turn_number,
        );
        self.accepted = true;

        Ok(())
    }

    /// True if there are counter-offer rounds remaining.
    pub fn can_counter(&self) -> bool {
        !self.accepted && self.counter_round < self.max_rounds
    }

    /// Number of counter-offer rounds remaining.
    pub fn rounds_remaining(&self) -> u32 {
        self.max_rounds.saturating_sub(self.counter_round)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_negotiation() -> Negotiation {
        Negotiation {
            advance: [60_000, 70_000, 80_000, 90_000],
            bonus: [120_000, 140_000, 160_000, 180_000],
            deadline: [45, 50, 55, 60],
            chance: [80, 60, 40, 20],
            counter_values: [0; 8],
            counter_advance: [0; 8],
            counter_bonus: [0; 8],
            counter_deadline: [0; 8],
        }
    }

    #[test]
    fn accept_initial_offer_credits_advance() {
        let mut state = NegotiationState::new(
            "General Mbeki".into(),
            "Rescue hostages".into(),
            50_000,
            100_000,
            30,
        );
        let mut ledger = Ledger::new(100_000);

        state.accept_contract(&mut ledger, 0).unwrap();
        assert!(state.accepted);
        assert_eq!(ledger.balance(), 150_000);
    }

    #[test]
    fn accept_twice_errors() {
        let mut state = NegotiationState::new("Client".into(), "Job".into(), 10_000, 20_000, 30);
        let mut ledger = Ledger::new(100_000);

        state.accept_contract(&mut ledger, 0).unwrap();
        let result = state.accept_contract(&mut ledger, 0);
        assert!(matches!(result, Err(ContractError::AlreadyAccepted)));
    }

    #[test]
    fn successful_counter_offer_updates_advance() {
        let mut state = NegotiationState::new("Client".into(), "Job".into(), 50_000, 100_000, 30);
        let neg = test_negotiation();

        // Roll 0 < chance 80 -> accepted
        let offer = state
            .counter_offer(&neg, NegotiationAspect::Advance, 0)
            .unwrap();
        assert_eq!(offer.advance, 60_000);
        assert_eq!(state.counter_round, 1);
    }

    #[test]
    fn successful_counter_offer_updates_bonus() {
        let mut state = NegotiationState::new("Client".into(), "Job".into(), 50_000, 100_000, 30);
        let neg = test_negotiation();

        let offer = state
            .counter_offer(&neg, NegotiationAspect::Bonus, 10)
            .unwrap();
        assert_eq!(offer.bonus, 120_000);
    }

    #[test]
    fn successful_counter_offer_updates_deadline() {
        let mut state = NegotiationState::new("Client".into(), "Job".into(), 50_000, 100_000, 30);
        let neg = test_negotiation();

        let offer = state
            .counter_offer(&neg, NegotiationAspect::Deadline, 50)
            .unwrap();
        assert_eq!(offer.deadline_days, 45);
    }

    #[test]
    fn rejected_counter_offer_still_consumes_round() {
        let mut state = NegotiationState::new("Client".into(), "Job".into(), 50_000, 100_000, 30);
        let neg = test_negotiation();

        // Roll 99 >= chance 80 -> rejected
        let result = state.counter_offer(&neg, NegotiationAspect::Advance, 99);
        assert!(matches!(result, Err(ContractError::CounterRejected)));
        assert_eq!(state.counter_round, 1);
        // Offer unchanged after rejection.
        assert_eq!(state.current_offer.advance, 50_000);
    }

    #[test]
    fn max_counter_offers_exhausted() {
        let mut state = NegotiationState::new("Client".into(), "Job".into(), 50_000, 100_000, 30);
        let neg = test_negotiation();

        // Use up all 4 rounds (all successful with roll=0).
        for _ in 0..4 {
            let _ = state.counter_offer(&neg, NegotiationAspect::Advance, 0);
        }
        assert_eq!(state.counter_round, 4);
        assert!(!state.can_counter());

        let result = state.counter_offer(&neg, NegotiationAspect::Advance, 0);
        assert!(matches!(
            result,
            Err(ContractError::NoCountersRemaining { .. })
        ));
    }

    #[test]
    fn rounds_remaining_decreases() {
        let mut state = NegotiationState::new("Client".into(), "Job".into(), 50_000, 100_000, 30);
        let neg = test_negotiation();

        assert_eq!(state.rounds_remaining(), 4);
        state
            .counter_offer(&neg, NegotiationAspect::Advance, 0)
            .unwrap();
        assert_eq!(state.rounds_remaining(), 3);
    }

    #[test]
    fn counter_then_accept_uses_updated_terms() {
        let mut state = NegotiationState::new("Client".into(), "Job".into(), 50_000, 100_000, 30);
        let neg = test_negotiation();
        let mut ledger = Ledger::new(0);

        // Push advance up.
        state
            .counter_offer(&neg, NegotiationAspect::Advance, 0)
            .unwrap();
        assert_eq!(state.current_offer.advance, 60_000);

        // Accept with the improved advance.
        state.accept_contract(&mut ledger, 0).unwrap();
        assert_eq!(ledger.balance(), 60_000);
    }
}
