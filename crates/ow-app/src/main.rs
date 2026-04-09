//! Open Wages — main entry point.

use clap::Parser;
use std::path::PathBuf;
use tracing::{info, error};

#[derive(Parser, Debug)]
#[command(name = "open-wages", about = "Open-source Wages of War engine")]
struct Args {
    /// Path to original game data directory
    #[arg(long, default_value = "./data")]
    data_dir: PathBuf,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,ow_data=debug,ow_core=debug".parse().unwrap()),
        )
        .init();

    let args = Args::parse();
    info!(data_dir = %args.data_dir.display(), "Open Wages starting");

    // Validate game data
    match ow_data::validator::validate_game_data(&args.data_dir) {
        Ok(()) => info!("Game data validated"),
        Err(e) => {
            error!("Game data validation failed: {e}");
            eprintln!("\n{e}\n");
            std::process::exit(1);
        }
    }

    info!("Engine initialized — Phase 1 (data recon) not yet complete. Exiting.");
    Ok(())
}
