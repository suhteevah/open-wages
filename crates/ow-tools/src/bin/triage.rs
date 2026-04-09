//! Deep-inspect individual game files: magic bytes, entropy, struct patterns, strings.
//!
//! Usage:
//!     cargo run -p ow-tools --bin triage -- path/to/file.dat
//!     cargo run -p ow-tools --bin triage -- file1.dat file2.obj file3.bmp

use anyhow::Context;
use clap::Parser;
use ow_tools::classify;
use ow_tools::strings::find_strings;
use ow_tools::structs::detect_repeating_struct;
use std::path::PathBuf;
use tracing::info;

#[derive(Parser)]
#[command(name = "triage", about = "Deep-inspect game data files")]
struct Args {
    /// Files to inspect
    #[arg(required = true)]
    files: Vec<PathBuf>,

    /// Maximum struct stride to test
    #[arg(long, default_value = "128")]
    max_stride: usize,

    /// Minimum string length to extract
    #[arg(long, default_value = "4")]
    min_string_len: usize,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    for path in &args.files {
        if !path.is_file() {
            eprintln!("SKIP: Not a file: {}", path.display());
            continue;
        }

        if let Err(e) = triage_file(path, args.max_stride, args.min_string_len) {
            eprintln!("ERROR: {} — {e}", path.display());
        }
    }

    Ok(())
}

fn triage_file(path: &PathBuf, max_stride: usize, min_string_len: usize) -> anyhow::Result<()> {
    info!(path = %path.display(), "Triaging file");

    let data = std::fs::read(path)
        .with_context(|| format!("Reading {}", path.display()))?;
    let size = data.len();

    let header = &data[..data.len().min(512)];
    let header_64 = &data[..data.len().min(64)];

    // Magic detection
    let detected = classify::detect_magic(header);

    // Ratios
    let ar = classify::ascii_ratio(header);

    // Entropy
    let full_entropy = entropy(&data);
    let header_entropy = entropy(header);

    // Strings
    let strings = find_strings(&data, min_string_len);

    // Struct detection
    let struct_pattern = detect_repeating_struct(&data, max_stride);

    // Size divisibility
    let divisors: Vec<usize> = [8, 12, 16, 20, 24, 32, 48, 64, 128, 256]
        .iter()
        .copied()
        .filter(|&s| size % s == 0)
        .collect();

    // Classification
    let classification = if let Some(fmt) = detected {
        format!("Known format: {fmt}")
    } else if ar > 0.85 {
        "TEXT — likely INI/CSV/config, open in text editor".to_string()
    } else if full_entropy > 7.5 {
        "COMPRESSED/ENCRYPTED — high entropy, look for zlib/LZSS headers".to_string()
    } else if let Some(ref sp) = struct_pattern {
        format!(
            "STRUCT ARRAY — {} records x {} bytes (confidence {:.0}%)",
            sp.record_count,
            sp.stride,
            sp.confidence * 100.0
        )
    } else {
        "BINARY — needs manual hex analysis".to_string()
    };

    // Output
    println!("\n{}", "=".repeat(70));
    println!("  {}", path.display());
    println!("  {} bytes | {classification}", format_size(size));
    println!("{}", "=".repeat(70));
    println!("  ASCII ratio:    {:.1}%", ar * 100.0);
    println!("  Entropy (full): {full_entropy:.2} / 8.0");
    println!("  Entropy (hdr):  {header_entropy:.2} / 8.0");

    if let Some(fmt) = detected {
        println!("  Detected:       {fmt}");
    }
    if !divisors.is_empty() {
        println!("  Size divides by: {divisors:?}");
    }
    if let Some(ref sp) = struct_pattern {
        println!(
            "  Struct pattern: {} x {}B (conf={:.0}%)",
            sp.record_count,
            sp.stride,
            sp.confidence * 100.0
        );
    }

    let hex_str = classify::hex_display(header_64);
    let ascii_str = classify::ascii_display(header_64);
    // Show first 48 chars worth
    println!("\n  Header (hex):   {}...", &hex_str[..hex_str.len().min(48)]);
    println!("  Header (ascii): {}...", &ascii_str[..ascii_str.len().min(48)]);

    if !strings.is_empty() {
        println!("\n  Strings ({} total, showing first 20):", strings.len());
        for (off, s) in strings.iter().take(20) {
            let truncated = if s.len() > 80 { &s[..80] } else { s.as_str() };
            println!("    0x{off:06x}: {truncated}");
        }
    }

    println!();
    Ok(())
}

/// Shannon entropy of a byte slice (0.0 = uniform, 8.0 = maximum randomness).
fn entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut freq = [0u64; 256];
    for &b in data {
        freq[b as usize] += 1;
    }

    let len = data.len() as f64;
    freq.iter()
        .filter(|&&count| count > 0)
        .map(|&count| {
            let p = count as f64 / len;
            -p * p.log2()
        })
        .sum()
}

fn format_size(size: usize) -> String {
    if size >= 1_000_000 {
        format!("{:.1}MB", size as f64 / 1_000_000.0)
    } else if size >= 1_000 {
        format!("{:.1}KB", size as f64 / 1_000.0)
    } else {
        format!("{size}B")
    }
}
