//! Validates that original game data files are present and intact.

use std::path::Path;
use tracing::{info, error};

/// Validate that required original game files exist in the given directory.
/// Returns Ok if all files found, Err with list of missing files otherwise.
pub fn validate_game_data(data_dir: &Path) -> anyhow::Result<()> {
    // TODO: Populate this list during Phase 1 data reconnaissance
    let required: Vec<&str> = vec![
        // "mercs.dat",
        // "weapons.dat",
        // "missions.dat",
    ];

    if required.is_empty() {
        info!("No required files defined yet — skipping validation (Phase 1 incomplete)");
        return Ok(());
    }

    let missing: Vec<&&str> = required.iter().filter(|f| !data_dir.join(f).exists()).collect();

    if !missing.is_empty() {
        let list = missing.iter().map(|f| format!("  - {f}")).collect::<Vec<_>>().join("\n");
        error!(data_dir = %data_dir.display(), "Missing game data files:\n{list}");
        anyhow::bail!(
            "Missing original game data files in {:?}:\n{}\n\n\
            Copy your original Wages of War game files to this directory.",
            data_dir, list
        );
    }

    info!(data_dir = %data_dir.display(), count = required.len(), "All game data files validated");
    Ok(())
}
