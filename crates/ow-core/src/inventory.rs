//! Equipment inventory management for mercenaries.
//!
//! # Encumbrance system
//!
//! Every merc has an `ENC` stat from MERCS.DAT — this is their carry capacity
//! in abstract weight units. Every weapon and equipment item has an `ENC` value
//! too (from WEAPONS.DAT / EQUIP.DAT). The sum of all equipped items' ENC
//! must not exceed the merc's capacity.
//!
//! Overloaded mercs suffer movement penalties (higher AP cost per tile) and
//! accuracy debuffs. The movement cost scaling is handled in `ActiveMerc::movement_cost_per_tile`.
//!
//! # Equipment slots
//!
//! Each merc has fixed slots:
//! - **PrimaryWeapon**: Main combat weapon (rifle, SMG, etc.)
//! - **SecondaryWeapon**: Sidearm or backup weapon
//! - **Armor**: Body armor (provides PEN resistance)
//! - **Item1..Item4**: Utility slots for grenades, medkits, ammo, etc.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Equipment slot on a mercenary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipmentSlot {
    /// Main weapon (rifle, shotgun, SMG, etc.)
    PrimaryWeapon,
    /// Sidearm or backup weapon.
    SecondaryWeapon,
    /// Body armor.
    Armor,
    /// General-purpose item slot 1 (grenade, medkit, etc.)
    Item1,
    /// General-purpose item slot 2.
    Item2,
    /// General-purpose item slot 3.
    Item3,
    /// General-purpose item slot 4.
    Item4,
}

/// Errors that can occur during inventory operations.
#[derive(Debug, Error)]
pub enum InventoryError {
    #[error("slot {slot:?} is already occupied by '{current_item}'")]
    SlotOccupied {
        slot: EquipmentSlot,
        current_item: String,
    },

    #[error(
        "equipping '{item}' (enc {item_enc}) would exceed capacity: \
         current load {current} + {item_enc} > capacity {capacity}"
    )]
    WouldExceedCapacity {
        item: String,
        item_enc: u32,
        current: u32,
        capacity: u32,
    },
}

/// An item equipped in a slot, tracking its name and encumbrance contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquippedItem {
    /// Item name (matches WEAPONS.DAT or EQUIP.DAT name field).
    pub name: String,
    /// Encumbrance cost of this item.
    pub encumbrance: u32,
}

/// A mercenary's personal equipment loadout.
///
/// Maps each occupied slot to the item in it. Empty slots have no entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MercInventory {
    /// Items currently equipped in each slot.
    slots: HashMap<EquipmentSlot, EquippedItem>,
}

impl MercInventory {
    /// Create a new empty inventory.
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
        }
    }

    /// Equip an item into a specific slot.
    ///
    /// Fails if the slot is already occupied (unequip first) or if equipping
    /// the item would exceed the merc's encumbrance capacity.
    pub fn equip_item(
        &mut self,
        slot: EquipmentSlot,
        item_name: impl Into<String>,
        item_enc: u32,
        merc_enc_cap: u32,
    ) -> Result<(), InventoryError> {
        let item_name = item_name.into();

        // Check if slot is already taken.
        if let Some(existing) = self.slots.get(&slot) {
            warn!(
                slot = ?slot,
                current = %existing.name,
                attempted = %item_name,
                "Slot already occupied"
            );
            return Err(InventoryError::SlotOccupied {
                slot,
                current_item: existing.name.clone(),
            });
        }

        // Check encumbrance.
        let current_load = self.total_encumbrance();
        if current_load + item_enc > merc_enc_cap {
            warn!(
                item = %item_name,
                item_enc,
                current_load,
                capacity = merc_enc_cap,
                "Would exceed encumbrance capacity"
            );
            return Err(InventoryError::WouldExceedCapacity {
                item: item_name,
                item_enc,
                current: current_load,
                capacity: merc_enc_cap,
            });
        }

        info!(
            slot = ?slot,
            item = %item_name,
            enc = item_enc,
            load_after = current_load + item_enc,
            capacity = merc_enc_cap,
            "Equipped item"
        );

        self.slots.insert(
            slot,
            EquippedItem {
                name: item_name,
                encumbrance: item_enc,
            },
        );

        Ok(())
    }

    /// Remove an item from a slot, returning its name if the slot was occupied.
    pub fn unequip_item(&mut self, slot: EquipmentSlot) -> Option<String> {
        let removed = self.slots.remove(&slot);
        if let Some(ref item) = removed {
            info!(slot = ?slot, item = %item.name, "Unequipped item");
        } else {
            debug!(slot = ?slot, "Unequip: slot was already empty");
        }
        removed.map(|i| i.name)
    }

    /// Total encumbrance of all currently equipped items.
    pub fn total_encumbrance(&self) -> u32 {
        self.slots.values().map(|i| i.encumbrance).sum()
    }

    /// Check whether the merc is overloaded (total encumbrance exceeds capacity).
    pub fn is_overloaded(&self, merc_enc_cap: u32) -> bool {
        self.total_encumbrance() > merc_enc_cap
    }

    /// Get a reference to the item in a given slot, if any.
    pub fn get_slot(&self, slot: EquipmentSlot) -> Option<&EquippedItem> {
        self.slots.get(&slot)
    }

    /// Iterate over all equipped items.
    pub fn equipped_items(&self) -> impl Iterator<Item = (&EquipmentSlot, &EquippedItem)> {
        self.slots.iter()
    }
}

impl Default for MercInventory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MERC_CAPACITY: u32 = 300;

    #[test]
    fn empty_inventory() {
        let inv = MercInventory::new();
        assert_eq!(inv.total_encumbrance(), 0);
        assert!(!inv.is_overloaded(MERC_CAPACITY));
    }

    #[test]
    fn equip_and_check_encumbrance() {
        let mut inv = MercInventory::new();
        inv.equip_item(EquipmentSlot::PrimaryWeapon, "M16", 80, MERC_CAPACITY)
            .unwrap();
        inv.equip_item(EquipmentSlot::Armor, "Kevlar Vest", 50, MERC_CAPACITY)
            .unwrap();
        assert_eq!(inv.total_encumbrance(), 130);
        assert!(!inv.is_overloaded(MERC_CAPACITY));
    }

    #[test]
    fn equip_exceeds_capacity() {
        let mut inv = MercInventory::new();
        inv.equip_item(EquipmentSlot::PrimaryWeapon, "Heavy MG", 200, MERC_CAPACITY)
            .unwrap();
        let result = inv.equip_item(EquipmentSlot::SecondaryWeapon, "RPG", 150, MERC_CAPACITY);
        assert!(matches!(
            result,
            Err(InventoryError::WouldExceedCapacity { .. })
        ));
        // Only the first item should be equipped.
        assert_eq!(inv.total_encumbrance(), 200);
    }

    #[test]
    fn equip_slot_occupied() {
        let mut inv = MercInventory::new();
        inv.equip_item(EquipmentSlot::PrimaryWeapon, "M16", 80, MERC_CAPACITY)
            .unwrap();
        let result = inv.equip_item(EquipmentSlot::PrimaryWeapon, "AK-47", 85, MERC_CAPACITY);
        assert!(matches!(result, Err(InventoryError::SlotOccupied { .. })));
    }

    #[test]
    fn unequip_returns_item_name() {
        let mut inv = MercInventory::new();
        inv.equip_item(EquipmentSlot::Item1, "Medkit", 10, MERC_CAPACITY)
            .unwrap();
        let removed = inv.unequip_item(EquipmentSlot::Item1);
        assert_eq!(removed.as_deref(), Some("Medkit"));
        assert_eq!(inv.total_encumbrance(), 0);
    }

    #[test]
    fn unequip_empty_slot_returns_none() {
        let mut inv = MercInventory::new();
        assert!(inv.unequip_item(EquipmentSlot::Armor).is_none());
    }

    #[test]
    fn is_overloaded_detects_excess() {
        let mut inv = MercInventory::new();
        // Equip right at capacity.
        inv.equip_item(EquipmentSlot::PrimaryWeapon, "Heavy MG", 300, MERC_CAPACITY)
            .unwrap();
        assert!(!inv.is_overloaded(MERC_CAPACITY)); // exactly at limit

        // Manually test overloaded scenario: capacity 100 with 300 load.
        assert!(inv.is_overloaded(100));
    }

    #[test]
    fn swap_weapon_flow() {
        let mut inv = MercInventory::new();
        inv.equip_item(EquipmentSlot::PrimaryWeapon, "M16", 80, MERC_CAPACITY)
            .unwrap();
        // Must unequip before re-equipping.
        let old = inv.unequip_item(EquipmentSlot::PrimaryWeapon);
        assert_eq!(old.as_deref(), Some("M16"));
        inv.equip_item(EquipmentSlot::PrimaryWeapon, "AK-47", 85, MERC_CAPACITY)
            .unwrap();
        assert_eq!(
            inv.get_slot(EquipmentSlot::PrimaryWeapon).unwrap().name,
            "AK-47"
        );
    }
}
