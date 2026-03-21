//! End-to-end inference tests with known audio samples.
//!
//! Prerequisites:
//!   - smart_turn_v3.onnx in assets/models/
//!   - Test WAV files in tests/fixtures/:
//!     - complete_utterance.wav   (a complete sentence, expect prediction=true)
//!     - incomplete_utterance.wav (a cut-off sentence, expect prediction=false)
//!
//! Run: cargo test -p etd --test e2e_inference -- --ignored

use etd::{EndOfTurnDetector, EtdConfig};
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn test_config() -> EtdConfig {
    EtdConfig {
        model_path: workspace_root().join("assets/models/smart_turn_v3.onnx"),
        ..EtdConfig::default()
    }
}

fn load_wav_i16(path: &std::path::Path) -> Vec<i16> {
    let mut reader = hound::WavReader::open(path).expect("failed to open WAV");
    reader.samples::<i16>().map(|s| s.unwrap()).collect()
}

#[test]
#[ignore]
fn test_complete_utterance() {
    let wav_path = fixtures_dir().join("complete_utterance.wav");
    if !wav_path.exists() {
        eprintln!("Skipping: tests/fixtures/complete_utterance.wav not found");
        return;
    }

    let mut detector = EndOfTurnDetector::new(test_config()).expect("failed to load model");
    let audio = load_wav_i16(&wav_path);
    let result = detector.predict_i16(&audio).expect("inference failed");

    println!(
        "complete_utterance: prediction={} probability={:.4}",
        result.prediction, result.probability
    );
    assert!(
        result.prediction,
        "Expected complete turn, got probability={:.4}",
        result.probability
    );
}

#[test]
#[ignore]
fn test_incomplete_utterance() {
    let wav_path = fixtures_dir().join("incomplete_utterance.wav");
    if !wav_path.exists() {
        eprintln!("Skipping: tests/fixtures/incomplete_utterance.wav not found");
        return;
    }

    let mut detector = EndOfTurnDetector::new(test_config()).expect("failed to load model");
    let audio = load_wav_i16(&wav_path);
    let result = detector.predict_i16(&audio).expect("inference failed");

    println!(
        "incomplete_utterance: prediction={} probability={:.4}",
        result.prediction, result.probability
    );
    assert!(
        !result.prediction,
        "Expected incomplete turn, got probability={:.4}",
        result.probability
    );
}

#[test]
#[ignore]
fn test_inference_consistency() {
    let mut detector = EndOfTurnDetector::new(test_config()).expect("failed to load model");

    // Run the same input twice → should produce identical results.
    let samples = vec![0.0f32; 32000]; // 2s silence
    let r1 = detector.predict(&samples).expect("inference failed");
    let r2 = detector.predict(&samples).expect("inference failed");

    assert_eq!(
        r1.probability, r2.probability,
        "inference not deterministic: {:.6} vs {:.6}",
        r1.probability, r2.probability
    );
}
