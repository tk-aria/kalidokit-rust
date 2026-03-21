//! Download a Whisper GGML model from HuggingFace.
//!
//! Usage:
//!   cargo run -p speech-capture --example download_model -- <model> [output_dir]
//!
//! Models:
//!   medium         — 1.4 GB, best speed/quality for Japanese
//!   large-v3       — 2.9 GB, highest accuracy
//!   large-v3-turbo — 1.5 GB, recommended balance of speed + quality
//!
//! Examples:
//!   cargo run -p speech-capture --example download_model -- large-v3-turbo
//!   cargo run -p speech-capture --example download_model -- medium ./my-models

use std::io::Write;
use std::path::PathBuf;

const MODELS: &[(&str, &str, &str)] = &[
    (
        "medium",
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
        "ggml-medium.bin",
    ),
    (
        "large-v3",
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
        "ggml-large-v3.bin",
    ),
    (
        "large-v3-turbo",
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
        "ggml-large-v3-turbo.bin",
    ),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args[1] == "--help" || args[1] == "-h" {
        eprintln!("Usage: download_model <model> [output_dir]");
        eprintln!();
        eprintln!("Available models:");
        for (name, _, filename) in MODELS {
            eprintln!("  {name:<20} → {filename}");
        }
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  download_model large-v3-turbo");
        eprintln!("  download_model medium ./my-models");
        std::process::exit(1);
    }

    let model_name = &args[1];
    let output_dir = args.get(2).map(|s| s.as_str()).unwrap_or("models");

    let (_, url, filename) = MODELS
        .iter()
        .find(|(name, _, _)| *name == model_name)
        .unwrap_or_else(|| {
            eprintln!("Unknown model: {model_name}");
            eprintln!("Available: {}", MODELS.iter().map(|(n, _, _)| *n).collect::<Vec<_>>().join(", "));
            std::process::exit(1);
        });

    let out_path = PathBuf::from(output_dir).join(filename);
    std::fs::create_dir_all(output_dir)?;

    if out_path.exists() {
        let size = std::fs::metadata(&out_path)?.len();
        println!("Already exists: {} ({:.1} GB)", out_path.display(), size as f64 / 1e9);
        println!("Delete it first to re-download.");
        return Ok(());
    }

    println!("Downloading {model_name}...");
    println!("  URL: {url}");
    println!("  To:  {}", out_path.display());
    println!();

    // Use curl for download with progress bar
    let status = std::process::Command::new("curl")
        .args(["-L", "-o", out_path.to_str().unwrap(), url])
        .status()?;

    if !status.success() {
        eprintln!("Download failed (exit {})", status);
        // Clean up partial file
        let _ = std::fs::remove_file(&out_path);
        std::process::exit(1);
    }

    let size = std::fs::metadata(&out_path)?.len();
    println!();
    println!("Done! {} ({:.1} GB)", out_path.display(), size as f64 / 1e9);
    println!();
    println!("Usage:");
    println!("  cargo run -p speech-capture --features stt --example streaming_stt -- {}", out_path.display());

    Ok(())
}
