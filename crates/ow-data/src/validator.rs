//! Validates that original game data files are present and intact.
//!
//! The validator checks for all required files under the `WOW/` subdirectory
//! within the user-supplied data directory. File checks are case-insensitive
//! since the originals are uppercase but users may have lowercase copies.

use std::path::Path;
use tracing::{debug, error, info, warn};

/// Check whether a path exists using case-insensitive matching.
///
/// Walks each component of `relative` under `base`, matching directory entries
/// case-insensitively. Returns `true` if the full path resolves to an existing
/// file or directory.
fn exists_case_insensitive(base: &Path, relative: &str) -> bool {
    let components: Vec<&str> = relative.split('/').collect();
    let mut current = base.to_path_buf();

    for component in &components {
        let target_lower = component.to_ascii_lowercase();
        let mut found = false;

        let entries = match std::fs::read_dir(&current) {
            Ok(e) => e,
            Err(_) => return false,
        };

        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.to_ascii_lowercase() == target_lower {
                    current = entry.path();
                    found = true;
                    break;
                }
            }
        }

        if !found {
            return false;
        }
    }

    current.exists()
}

/// Check whether a path exists as a directory using case-insensitive matching.
fn dir_exists_case_insensitive(base: &Path, relative: &str) -> bool {
    let components: Vec<&str> = relative.split('/').collect();
    let mut current = base.to_path_buf();

    for component in &components {
        let target_lower = component.to_ascii_lowercase();
        let mut found = false;

        let entries = match std::fs::read_dir(&current) {
            Ok(e) => e,
            Err(_) => return false,
        };

        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.to_ascii_lowercase() == target_lower {
                    current = entry.path();
                    found = true;
                    break;
                }
            }
        }

        if !found {
            return false;
        }
    }

    current.is_dir()
}

/// Validate that required original game files exist in the given directory.
///
/// `data_dir` should point to the directory that contains the `WOW/` folder.
/// Returns `Ok(())` if all required files are found, or an error listing every
/// missing file/directory.
pub fn validate_game_data(data_dir: &Path) -> anyhow::Result<()> {
    info!(data_dir = %data_dir.display(), "Validating original game data files");

    let mut required_files: Vec<String> = vec![
        // Core data files
        "WOW/DATA/MERCS.DAT".into(),
        "WOW/DATA/WEAPONS.DAT".into(),
        "WOW/DATA/EQUIP.DAT".into(),
        "WOW/DATA/ENGWOW.DAT".into(),
        "WOW/DATA/TARGET.DAT".into(),
        // UI
        "WOW/BUTTONS/MAIN.BTN".into(),
    ];

    // Mission-indexed files: MSSN01-16, AINODE01-16, MOVES01-16, LOCK01-16
    for i in 1..=16 {
        required_files.push(format!("WOW/DATA/MSSN{i:02}.DAT"));
        required_files.push(format!("WOW/DATA/AINODE{i:02}.DAT"));
        required_files.push(format!("WOW/DATA/MOVES{i:02}.DAT"));
        required_files.push(format!("WOW/DATA/LOCK{i:02}.DAT"));
    }

    let required_dirs: Vec<&str> = vec![
        "WOW/ANIM",
        "WOW/MAPS",
        "WOW/SPR",
        "WOW/WAV",
        "WOW/MIDI",
    ];

    let mut missing: Vec<String> = Vec::new();

    // Check each required file
    for file in &required_files {
        if exists_case_insensitive(data_dir, file) {
            debug!(file = %file, "Found required file");
        } else {
            warn!(file = %file, "Missing required file");
            missing.push(file.clone());
        }
    }

    // Check each required directory
    for dir in &required_dirs {
        if dir_exists_case_insensitive(data_dir, dir) {
            debug!(dir = %dir, "Found required directory");
        } else {
            warn!(dir = %dir, "Missing required directory");
            missing.push(format!("{dir}/ (directory)"));
        }
    }

    if !missing.is_empty() {
        let list = missing
            .iter()
            .map(|f| format!("  - {f}"))
            .collect::<Vec<_>>()
            .join("\n");
        error!(
            data_dir = %data_dir.display(),
            missing_count = missing.len(),
            "Missing game data files:\n{list}"
        );
        anyhow::bail!(
            "Missing {} original game data file(s) in {:?}:\n{}\n\n\
            Copy your original Wages of War game files to this directory.\n\
            The WOW/ folder and its contents must be present.",
            missing.len(),
            data_dir,
            list
        );
    }

    let total = required_files.len() + required_dirs.len();
    info!(
        data_dir = %data_dir.display(),
        files = required_files.len(),
        directories = required_dirs.len(),
        total = total,
        "All game data files validated successfully"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create the full directory/file structure under a temp dir.
    fn create_full_game_data(base: &Path) {
        let data_dir = base.join("WOW/DATA");
        fs::create_dir_all(&data_dir).unwrap();

        // Core files
        for name in &["MERCS.DAT", "WEAPONS.DAT", "EQUIP.DAT", "ENGWOW.DAT", "TARGET.DAT"] {
            fs::write(data_dir.join(name), "test").unwrap();
        }

        // Numbered files
        for i in 1..=16 {
            for prefix in &["MSSN", "AINODE", "MOVES", "LOCK"] {
                fs::write(data_dir.join(format!("{prefix}{i:02}.DAT")), "test").unwrap();
            }
        }

        // Buttons
        let btn_dir = base.join("WOW/BUTTONS");
        fs::create_dir_all(&btn_dir).unwrap();
        fs::write(btn_dir.join("MAIN.BTN"), "test").unwrap();

        // Required directories
        for dir in &["ANIM", "MAPS", "SPR", "WAV", "MIDI"] {
            fs::create_dir_all(base.join("WOW").join(dir)).unwrap();
        }
    }

    #[test]
    fn test_validate_full_structure_passes() {
        let tmp = tempfile::tempdir().unwrap();
        create_full_game_data(tmp.path());
        assert!(validate_game_data(tmp.path()).is_ok());
    }

    #[test]
    fn test_validate_missing_wow_dir_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let result = validate_game_data(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_case_insensitive() {
        let tmp = tempfile::tempdir().unwrap();
        // Create with lowercase names
        let data_dir = tmp.path().join("wow/data");
        fs::create_dir_all(&data_dir).unwrap();

        for name in &["mercs.dat", "weapons.dat", "equip.dat", "engwow.dat", "target.dat"] {
            fs::write(data_dir.join(name), "test").unwrap();
        }

        for i in 1..=16 {
            for prefix in &["mssn", "ainode", "moves", "lock"] {
                fs::write(data_dir.join(format!("{prefix}{i:02}.dat")), "test").unwrap();
            }
        }

        let btn_dir = tmp.path().join("wow/buttons");
        fs::create_dir_all(&btn_dir).unwrap();
        fs::write(btn_dir.join("main.btn"), "test").unwrap();

        for dir in &["anim", "maps", "spr", "wav", "midi"] {
            fs::create_dir_all(tmp.path().join("wow").join(dir)).unwrap();
        }

        assert!(validate_game_data(tmp.path()).is_ok());
    }

    #[test]
    fn test_validate_reports_all_missing() {
        let tmp = tempfile::tempdir().unwrap();
        // Create only the WOW/DATA dir but no files
        fs::create_dir_all(tmp.path().join("WOW/DATA")).unwrap();

        let result = validate_game_data(tmp.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // Should mention multiple missing files
        assert!(err_msg.contains("MERCS.DAT"));
        assert!(err_msg.contains("WEAPONS.DAT"));
        assert!(err_msg.contains("MSSN01.DAT"));
    }
}
