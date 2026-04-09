//! Survey all files in a game directory and classify them as text/binary/mixed.
//!
//! Usage:
//!     cargo run -p ow-tools --bin survey -- "C:\Games\WagesOfWar"
//!
//! Outputs:
//!     - Console summary grouped by extension and type
//!     - file_survey.json with full details for every file

use anyhow::Context;
use clap::Parser;
use ow_tools::classify::{self, FileInfo};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tracing::info;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "survey", about = "Survey and classify all files in a game directory")]
struct Args {
    /// Path to the original game directory
    game_dir: PathBuf,

    /// Output JSON path (default: file_survey.json in current directory)
    #[arg(short, long, default_value = "file_survey.json")]
    output: PathBuf,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    anyhow::ensure!(
        args.game_dir.is_dir(),
        "Not a directory: {}",
        args.game_dir.display()
    );

    info!(dir = %args.game_dir.display(), "Surveying game directory");

    // Walk and classify every file
    let mut results: Vec<FileInfo> = Vec::new();
    for entry in WalkDir::new(&args.game_dir).sort_by_file_name() {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        match classify::classify_file(entry.path(), &args.game_dir) {
            Ok(info) => results.push(info),
            Err(e) => {
                eprintln!("  SKIP: {} — {e}", entry.path().display());
            }
        }
    }

    // === Extension summary ===
    let mut by_ext: BTreeMap<String, Vec<&FileInfo>> = BTreeMap::new();
    for r in &results {
        let key = if r.ext.is_empty() {
            "(none)".to_string()
        } else {
            r.ext.clone()
        };
        by_ext.entry(key).or_default().push(r);
    }

    println!("{}", "=".repeat(70));
    println!("  FILE SURVEY — {} files", results.len());
    println!("{}", "=".repeat(70));

    for (ext, files) in &by_ext {
        let total_size: u64 = files.iter().map(|f| f.size).sum();
        let mut types: Vec<String> = files.iter().map(|f| f.file_type.to_string()).collect();
        types.sort();
        types.dedup();
        println!(
            "  {ext:10}  {:3} files  {total_size:>12} bytes  types: {types:?}",
            files.len()
        );
    }

    // === Detailed listing grouped by type ===
    println!("\n{}", "=".repeat(70));
    println!("  DETAILED FILE LIST");
    println!("{}", "=".repeat(70));

    for type_label in &["text", "mixed", "binary", "known", "empty"] {
        let typed: Vec<&FileInfo> = results
            .iter()
            .filter(|r| {
                let s = r.file_type.to_string();
                s == *type_label || s.starts_with(&format!("{type_label}:"))
            })
            .collect();

        if typed.is_empty() {
            continue;
        }

        println!(
            "\n  --- {} ({} files) ---",
            type_label.to_uppercase(),
            typed.len()
        );
        for r in &typed {
            println!(
                "    {:45} {:>12}  ascii={:.0}%  {}",
                r.path,
                format_size(r.size),
                r.ascii_ratio * 100.0,
                r.file_type,
            );
            println!("      hex: {}", r.header_hex);
            println!("      asc: {}", r.header_ascii);
        }
    }

    // === Write JSON ===
    let json = serde_json::to_string_pretty(&results)?;
    std::fs::write(&args.output, &json)
        .with_context(|| format!("Writing {}", args.output.display()))?;
    println!("\nFull survey saved to: {}", args.output.display());

    Ok(())
}

fn format_size(size: u64) -> String {
    if size >= 1_000_000 {
        format!("{:.1}MB", size as f64 / 1_000_000.0)
    } else if size >= 1_000 {
        format!("{:.1}KB", size as f64 / 1_000.0)
    } else {
        format!("{size}B")
    }
}
