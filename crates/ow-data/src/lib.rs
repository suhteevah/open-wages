//! # ow-data — Original Game File Parsers
//!
//! Parsers for Wages of War data files (.dat text configs, sprite sheets,
//! map/terrain binary formats, sound files). All parsers produce strongly-typed
//! Rust structs from the original game's data files.

pub mod dat_parser;
pub mod validator;

// TODO: Phase 1 output will determine which additional modules are needed
// pub mod sprite_loader;
// pub mod map_loader;
// pub mod sound_loader;
// pub mod palette;
