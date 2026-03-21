//! Mel spectrogram accuracy test: compare Rust output against Python reference.
//!
//! Prerequisites:
//!   1. Place a 16kHz mono WAV at tests/fixtures/test_audio.wav
//!   2. Run: python scripts/generate_mel_reference.py tests/fixtures/test_audio.wav tests/fixtures/
//!   3. Run: cargo test -p etd --test mel_accuracy -- --ignored

use etd::mel::{log_mel_spectrogram, MelConfig};
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Load a .npy file containing a 2D f32 array (row-major).
/// Minimal .npy parser for f32 arrays only.
fn load_npy_f32(path: &std::path::Path) -> Vec<f32> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).expect("failed to open .npy file");
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).expect("failed to read .npy");

    // .npy format: 6-byte magic + 2-byte version + 2-byte header_len + header + data
    assert_eq!(&buf[..6], b"\x93NUMPY", "not a valid .npy file");
    let header_len = u16::from_le_bytes([buf[8], buf[9]]) as usize;
    let data_start = 10 + header_len;

    // Parse float32 data
    let data = &buf[data_start..];
    assert_eq!(
        data.len() % 4,
        0,
        "data length not aligned to f32 (4 bytes)"
    );
    data.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[test]
#[ignore]
fn test_mel_matches_python() {
    let fixtures = fixtures_dir();
    let wav_path = fixtures.join("test_audio.wav");
    let npy_path = fixtures.join("mel_reference.npy");

    if !wav_path.exists() || !npy_path.exists() {
        eprintln!(
            "Skipping mel accuracy test: fixtures not found.\n\
             Run scripts/generate_mel_reference.py first."
        );
        return;
    }

    // Load WAV
    let mut reader = hound::WavReader::open(&wav_path).expect("failed to open WAV");
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();

    // Compute Rust mel spectrogram
    let config = MelConfig::default();
    let rust_mel = log_mel_spectrogram(&samples, &config);

    // Load Python reference
    let python_mel = load_npy_f32(&npy_path);

    assert_eq!(
        rust_mel.len(),
        python_mel.len(),
        "mel length mismatch: rust={} vs python={}",
        rust_mel.len(),
        python_mel.len()
    );

    // Compare element-wise
    let tolerance = 1e-3;
    let mut max_diff: f32 = 0.0;
    let mut diff_count = 0usize;

    for (i, (r, p)) in rust_mel.iter().zip(python_mel.iter()).enumerate() {
        let diff = (r - p).abs();
        if diff > tolerance {
            diff_count += 1;
            if diff_count <= 10 {
                eprintln!("mel[{}] rust={:.6} python={:.6} diff={:.6}", i, r, p, diff);
            }
        }
        max_diff = max_diff.max(diff);
    }

    assert_eq!(
        diff_count, 0,
        "{} elements exceed tolerance {}: max_diff={:.6}",
        diff_count, tolerance, max_diff
    );
}
