//! Cross-platform Opus/OGG/WAV/MP3 → STT CLI using Whisper.
//!
//! Build:
//!   cargo build -p stt-cli --features metal --release   # macOS (Metal)
//!   cargo build -p stt-cli --features cuda --release    # Linux (CUDA)
//!   cargo build -p stt-cli --release                    # CPU only

use std::path::PathBuf;

use clap::Parser;

mod decode;
mod transcribe;

#[derive(Parser)]
#[command(name = "stt-cli", about = "Audio file → Speech-to-Text via Whisper")]
struct Args {
    /// Audio file path (Opus/OGG/WAV/MP3)
    input: PathBuf,

    /// Whisper model path (ggml format)
    #[arg(short, long, default_value = "models/ggml-large-v3-turbo.bin")]
    model: String,

    /// Language code (e.g., ja, en, auto)
    #[arg(short, long, default_value = "ja")]
    lang: String,

    /// Chunk duration in seconds for splitting long audio
    #[arg(short, long, default_value = "30")]
    chunk_secs: u32,

    /// Output format: text, json, srt
    #[arg(short, long, default_value = "text")]
    format: String,
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    log::info!("Decoding: {}", args.input.display());
    let samples = match decode::decode_to_i16_16khz(&args.input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error decoding audio: {e}");
            std::process::exit(1);
        }
    };
    let duration_secs = samples.len() as f64 / 16000.0;
    log::info!("Decoded: {:.1}s, {} samples (16kHz mono)", duration_secs, samples.len());

    let config = transcribe::WhisperConfig {
        model_path: args.model,
        language: if args.lang == "auto" { None } else { Some(args.lang) },
    };

    let results = match transcribe::transcribe_chunks(&samples, &config, args.chunk_secs) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    match args.format.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&results).unwrap_or_default();
            println!("{json}");
        }
        "srt" => {
            for (i, seg) in results.iter().enumerate() {
                println!("{}", i + 1);
                println!("{} --> {}", format_srt_time(seg.start_secs), format_srt_time(seg.end_secs));
                println!("{}", seg.text);
                println!();
            }
        }
        _ => {
            for seg in &results {
                println!("[{:.1}s - {:.1}s] {}", seg.start_secs, seg.end_secs, seg.text);
            }
        }
    }
}

fn format_srt_time(secs: f64) -> String {
    let h = (secs / 3600.0) as u32;
    let m = ((secs % 3600.0) / 60.0) as u32;
    let s = (secs % 60.0) as u32;
    let ms = ((secs % 1.0) * 1000.0) as u32;
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}
