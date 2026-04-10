//! Financial system for the campaign economy.
//!
//! # How the economy works in Wages of War
//!
//! The player's cash flow follows this cycle:
//! 1. **Hire mercs** — pay each merc's `fee_hire` (fee1) from MERCS.DAT upfront.
//! 2. **Buy equipment** — purchase weapons, armor, and items from shop inventories.
//! 3. **Accept a contract** — receive an `advance` payment immediately upon signing.
//! 4. **Complete the mission** — earn the `bonus` payment if objectives are met.
//! 5. **Pay per-mission fees** — each surviving merc's `fee_bonus` (fee2) is deducted.
//! 6. **Death insurance** — if a merc dies, their `fee_death` (fee3) is paid out.
//!
//! Funds can go negative (debt). The player must dig out via successful missions
//! or they'll lose access to top-tier mercs and equipment.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Errors related to the financial system.
#[derive(Debug, Error)]
pub enum EconomyError {
    /// Attempted to spend more than the player has (and debt is not allowed
    /// for this particular transaction — e.g. hiring requires positive balance).
    #[error("insufficient funds: need {needed}, have {available}")]
    InsufficientFunds { needed: i64, available: i64 },
}

/// A single financial transaction record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Positive = income, negative = expense.
    pub amount: i64,
    /// Human-readable description of what this transaction was for.
    pub description: String,
    /// The global turn number when this transaction occurred.
    pub turn_number: u32,
}

/// The player's financial ledger — tracks current balance and full history.
///
/// Funds are stored as i64 so the player can go into debt. Some operations
/// (like hiring) enforce a positive-balance check; others (like death insurance
/// payouts) can push the balance negative.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ledger {
    /// Current cash on hand. Can be negative (debt).
    funds: i64,
    /// Complete history of all transactions, oldest first.
    transaction_history: Vec<Transaction>,
}

impl Ledger {
    /// Create a new ledger with the given starting funds.
    pub fn new(starting_funds: i64) -> Self {
        info!(starting_funds, "Initializing ledger");
        Self {
            funds: starting_funds,
            transaction_history: Vec::new(),
        }
    }

    /// Current cash balance (may be negative).
    pub fn balance(&self) -> i64 {
        self.funds
    }

    /// Check if the player can afford a given cost without going negative.
    pub fn can_afford(&self, amount: i64) -> bool {
        self.funds >= amount
    }

    /// Add money to the ledger (contract advance, mission bonus, selling items, etc.).
    pub fn credit(&mut self, amount: i64, description: impl Into<String>, turn_number: u32) {
        let desc = description.into();
        info!(amount, desc = %desc, turn_number, balance_before = self.funds, "Credit");
        self.funds += amount;
        self.transaction_history.push(Transaction {
            amount,
            description: desc,
            turn_number,
        });
        debug!(balance_after = self.funds, "Post-credit balance");
    }

    /// Deduct money from the ledger (hiring, equipment purchases, etc.).
    ///
    /// Returns `Err(EconomyError::InsufficientFunds)` if the player can't afford it.
    /// Use `force_debit` for transactions that are allowed to push the balance negative
    /// (e.g. death insurance payouts).
    pub fn debit(
        &mut self,
        amount: i64,
        description: impl Into<String>,
        turn_number: u32,
    ) -> Result<(), EconomyError> {
        if !self.can_afford(amount) {
            let desc = description.into();
            warn!(
                amount,
                funds = self.funds,
                desc = %desc,
                "Debit rejected: insufficient funds"
            );
            return Err(EconomyError::InsufficientFunds {
                needed: amount,
                available: self.funds,
            });
        }
        let desc = description.into();
        info!(amount, desc = %desc, turn_number, balance_before = self.funds, "Debit");
        self.funds -= amount;
        self.transaction_history.push(Transaction {
            amount: -amount,
            description: desc,
            turn_number,
        });
        debug!(balance_after = self.funds, "Post-debit balance");
        Ok(())
    }

    /// Debit that is allowed to push the balance negative.
    /// Used for mandatory costs like death insurance payouts.
    pub fn force_debit(&mut self, amount: i64, description: impl Into<String>, turn_number: u32) {
        let desc = description.into();
        info!(amount, desc = %desc, turn_number, balance_before = self.funds, "Force debit");
        self.funds -= amount;
        self.transaction_history.push(Transaction {
            amount: -amount,
            description: desc,
            turn_number,
        });
        debug!(balance_after = self.funds, "Post-force-debit balance");
    }

    /// Full transaction history, oldest first.
    pub fn history(&self) -> &[Transaction] {
        &self.transaction_history
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ledger_has_starting_funds() {
        let ledger = Ledger::new(500_000);
        assert_eq!(ledger.balance(), 500_000);
        assert!(ledger.history().is_empty());
    }

    #[test]
    fn credit_increases_balance() {
        let mut ledger = Ledger::new(100_000);
        ledger.credit(50_000, "Contract advance", 1);
        assert_eq!(ledger.balance(), 150_000);
        assert_eq!(ledger.history().len(), 1);
        assert_eq!(ledger.history()[0].amount, 50_000);
    }

    #[test]
    fn debit_decreases_balance() {
        let mut ledger = Ledger::new(100_000);
        ledger.debit(25_000, "Hired merc", 0).unwrap();
        assert_eq!(ledger.balance(), 75_000);
        assert_eq!(ledger.history().len(), 1);
        // Stored as negative in history for debits
        assert_eq!(ledger.history()[0].amount, -25_000);
    }

    #[test]
    fn debit_rejects_insufficient_funds() {
        let mut ledger = Ledger::new(10_000);
        let result = ledger.debit(50_000, "Too expensive", 0);
        assert!(result.is_err());
        // Balance unchanged on failure
        assert_eq!(ledger.balance(), 10_000);
        assert!(ledger.history().is_empty());
    }

    #[test]
    fn can_afford_checks_correctly() {
        let ledger = Ledger::new(100_000);
        assert!(ledger.can_afford(100_000)); // exact match
        assert!(ledger.can_afford(99_999));
        assert!(!ledger.can_afford(100_001));
    }

    #[test]
    fn force_debit_can_go_negative() {
        let mut ledger = Ledger::new(5_000);
        ledger.force_debit(20_000, "Death insurance payout", 3);
        assert_eq!(ledger.balance(), -15_000);
    }

    #[test]
    fn multiple_transactions_accumulate() {
        let mut ledger = Ledger::new(100_000);
        ledger.credit(50_000, "Advance", 0);
        ledger.debit(30_000, "Hired merc A", 0).unwrap();
        ledger.debit(20_000, "Hired merc B", 0).unwrap();
        ledger.credit(80_000, "Mission bonus", 1);
        assert_eq!(ledger.balance(), 180_000);
        assert_eq!(ledger.history().len(), 4);
    }
}
