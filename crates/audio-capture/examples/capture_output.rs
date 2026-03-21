//! Test output (loopback) audio capture.
//!
//! Usage:
//!   cargo run -p audio-capture --example capture_output
//!   cargo run -p audio-capture --example capture_output -- --device "BlackHole 2ch"

use audio_capture::{AudioCapture, AudioConfig, AudioSource};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let device_name = args
        .iter()
        .position(|a| a == "--device")
        .map(|i| args[i + 1].clone());
    let duration_secs: u64 = args
        .iter()
        .position(|a| a == "--duration")
        .map(|i| args[i + 1].parse().unwrap_or(5))
        .unwrap_or(5);

    println!("=== Output Audio Capture Test ===");
    if let Some(ref name) = device_name {
        println!("Device: {name}");
    } else {
        println!("Device: (default output)");
    }
    println!("Duration: {duration_secs}s\n");

    let config = AudioConfig {
        device_name,
        frame_size: 256,
        source: AudioSource::Output,
    };

    let mut capture = AudioCapture::new(config)?;
    let frame_count = Arc::new(AtomicU32::new(0));
    let fc = frame_count.clone();

    match capture.start(move |frame| {
        let count = fc.fetch_add(1, Ordering::Relaxed);
        if count % 100 == 0 {
            let rms: f64 = frame
                .samples
                .iter()
                .map(|&s| (s as f64) * (s as f64))
                .sum::<f64>()
                / frame.samples.len() as f64;
            println!(
                "[{:.2}s] frame #{count}, source={:?}, rms={:.1}",
                frame.timestamp.as_secs_f64(),
                frame.source,
                rms.sqrt()
            );
        }
    }) {
        Ok(()) => println!("Capturing...\n"),
        Err(e) => {
            println!("Failed to start output capture: {e}");
            println!("\nOn macOS < 14.2, ScreenCaptureKit is used for output capture.");
            println!("Grant screen recording permission: System Settings > Privacy & Security > Screen Recording");
            println!("Alternatively, install a loopback device (e.g. BlackHole) and use --device \"BlackHole 2ch\"");
            return Ok(());
        }
    }

    std::thread::sleep(std::time::Duration::from_secs(duration_secs));

    capture.stop();
    println!(
        "\nDone. Total frames: {}",
        frame_count.load(Ordering::Relaxed)
    );
    Ok(())
}
