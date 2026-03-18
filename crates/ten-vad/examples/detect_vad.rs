//! Read a 16 kHz mono WAV file, run VAD frame-by-frame, and print results.
//!
//! ```sh
//! cargo run -p ten-vad --example detect_vad -- input.wav
//! ```

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: detect_vad <input.wav>");
        std::process::exit(1);
    });

    let mut reader = hound::WavReader::open(&path)?;
    let spec = reader.spec();
    assert_eq!(
        spec.sample_rate, 16000,
        "Expected 16 kHz, got {}",
        spec.sample_rate
    );
    assert_eq!(
        spec.channels, 1,
        "Expected mono, got {} channels",
        spec.channels
    );

    let hop = ten_vad::HopSize::Samples256;
    let mut vad = ten_vad::TenVad::new(hop, 0.5)?;
    println!("TEN VAD {}", ten_vad::TenVad::version());
    println!(
        "File: {path}  ({} Hz, {} ch)",
        spec.sample_rate, spec.channels
    );

    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    let frame_size = hop.as_usize();
    let mut voice_frames = 0u32;
    let mut total_frames = 0u32;

    for (i, chunk) in samples.chunks_exact(frame_size).enumerate() {
        let r = vad.process(chunk)?;
        total_frames += 1;
        if r.is_voice {
            voice_frames += 1;
        }
        let time_ms = i * frame_size * 1000 / 16000;
        if r.is_voice {
            println!("[{time_ms:>6} ms] VOICE  prob={:.3}", r.probability);
        }
    }

    println!(
        "\nSummary: {voice_frames}/{total_frames} frames contain voice ({:.1}%)",
        voice_frames as f64 / total_frames.max(1) as f64 * 100.0
    );
    Ok(())
}
