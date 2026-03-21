//! WAV ファイルから End-of-Turn Detection を実行するデモ。
//!
//! Usage:
//!   cargo run -p etd --example etd_demo -- path/to/audio.wav
//!
//! Requires: smart_turn_v3.onnx in assets/models/

use etd::{EndOfTurnDetector, EtdConfig};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).expect("Usage: etd_demo <wav_file>");

    let mut reader = hound::WavReader::open(&path)?;
    let spec = reader.spec();

    if spec.sample_rate != 16000 {
        eprintln!(
            "Warning: expected 16kHz sample rate, got {}Hz. Results may be inaccurate.",
            spec.sample_rate
        );
    }
    if spec.channels != 1 {
        eprintln!(
            "Warning: expected mono audio, got {} channels. Using first channel only.",
            spec.channels
        );
    }

    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    let duration_secs = samples.len() as f32 / spec.sample_rate as f32;

    println!(
        "Audio: {} samples ({:.2}s, {}Hz, {} ch)",
        samples.len(),
        duration_secs,
        spec.sample_rate,
        spec.channels
    );

    let mut detector = EndOfTurnDetector::new(EtdConfig::default())?;
    let result = detector.predict_i16(&samples)?;

    println!(
        "Prediction: {}",
        if result.prediction {
            "COMPLETE"
        } else {
            "INCOMPLETE"
        }
    );
    println!("Probability: {:.4}", result.probability);

    Ok(())
}
