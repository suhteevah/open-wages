//! # ow-core — Game Rules Engine
//!
//! All game logic with zero rendering dependencies. Combat resolution,
//! economy, AI, pathfinding, line of sight, suppression, weather.

pub mod combat;
pub mod damage;
pub mod game_state;
pub mod merc;
pub mod weather;

pub mod los;
pub mod pathfinding;

pub mod contract;
pub mod economy;
pub mod hiring;
pub mod inventory;

pub mod actions;
pub mod ai;
pub mod config;
pub mod mission_setup;
pub mod ruleset;
pub mod save;
