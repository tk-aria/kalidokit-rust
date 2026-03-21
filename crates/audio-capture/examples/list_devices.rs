//! List available audio input and output devices.
//!
//! Usage:
//!   cargo run -p audio-capture --example list_devices

use audio_capture::AudioCapture;

fn main() {
    println!("=== Input Devices ===");
    match AudioCapture::list_input_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("  (none)");
            }
            for (i, name) in devices.iter().enumerate() {
                println!("  [{i}] {name}");
            }
        }
        Err(e) => println!("  Error: {e}"),
    }

    println!("\n=== Output Devices ===");
    match AudioCapture::list_output_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("  (none)");
            }
            for (i, name) in devices.iter().enumerate() {
                println!("  [{i}] {name}");
            }
        }
        Err(e) => println!("  Error: {e}"),
    }
}
