//! Real-time voice activity detection from microphone input.
//!
//! Captures audio from the default input device, resamples to 16 kHz mono
//! if needed, and runs TEN VAD frame-by-frame.
//!
//! ```sh
//! cargo run -p ten-vad --example realtime_vad
//! ```

use std::sync::mpsc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .expect("No input device available");
    println!("Input device: {}", device.name()?);

    let config = device.default_input_config()?;
    println!(
        "Input config: {} Hz, {} ch, {:?}",
        config.sample_rate(),
        config.channels(),
        config.sample_format()
    );

    let sample_rate = config.sample_rate();
    let channels = config.channels() as usize;

    // Channel to send audio samples from the cpal callback to the VAD thread.
    let (tx, rx) = mpsc::channel::<Vec<f32>>();

    // Build input stream — captures f32 samples.
    let err_fn = |err| eprintln!("Stream error: {err}");
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let _ = tx.send(data.to_vec());
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let floats: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                let _ = tx.send(floats);
            },
            err_fn,
            None,
        )?,
        other => {
            eprintln!("Unsupported sample format: {other:?}");
            return Ok(());
        }
    };

    stream.play()?;
    println!("Capturing... Press Ctrl+C to stop.\n");

    // VAD processing thread.
    let hop = vad::HopSize::Samples256;
    let mut vad_instance = vad::TenVad::new(hop, 0.5)?;
    println!("TEN VAD {}", vad::TenVad::version());

    let frame_size = hop.as_usize(); // 256 samples at 16 kHz
    let mut mono_buffer: Vec<f32> = Vec::new(); // accumulated mono 16 kHz samples
    let ratio = sample_rate as f64 / 16000.0; // downsample ratio

    // Simple frame counter for display throttling.
    let mut frame_count: u64 = 0;
    let mut last_state = false;

    loop {
        // Receive audio chunks from the capture callback.
        let chunk = match rx.recv() {
            Ok(c) => c,
            Err(_) => break, // stream closed
        };

        // Convert to mono by averaging channels.
        let mono: Vec<f32> = if channels == 1 {
            chunk
        } else {
            chunk
                .chunks_exact(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect()
        };

        // Simple downsample to 16 kHz (nearest-neighbor).
        // For production use, a proper resampler (e.g. rubato) would be better.
        if (ratio - 1.0).abs() < 0.01 {
            // Already 16 kHz
            mono_buffer.extend_from_slice(&mono);
        } else {
            // Downsample: pick every `ratio`-th sample
            let mut pos = 0.0f64;
            while (pos as usize) < mono.len() {
                mono_buffer.push(mono[pos as usize]);
                pos += ratio;
            }
        }

        // Process complete frames.
        while mono_buffer.len() >= frame_size {
            let frame_f32: Vec<f32> = mono_buffer.drain(..frame_size).collect();

            // Convert f32 [-1.0, 1.0] to i16.
            let frame_i16: Vec<i16> = frame_f32
                .iter()
                .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
                .collect();

            match vad_instance.process(&frame_i16) {
                Ok(result) => {
                    frame_count += 1;
                    let time_ms = frame_count * frame_size as u64 * 1000 / 16000;

                    // Print state transitions + periodic status.
                    if result.is_voice != last_state {
                        if result.is_voice {
                            println!("[{time_ms:>7} ms] 🎤 VOICE START  prob={:.3}", result.probability);
                        } else {
                            println!("[{time_ms:>7} ms] 🔇 VOICE END    prob={:.3}", result.probability);
                        }
                        last_state = result.is_voice;
                    } else if result.is_voice && frame_count % 30 == 0 {
                        // Print every ~0.5s while speaking
                        print!("\r[{time_ms:>7} ms] speaking... prob={:.3}  ", result.probability);
                        use std::io::Write;
                        std::io::stdout().flush().ok();
                    }
                }
                Err(e) => eprintln!("VAD error: {e}"),
            }
        }
    }

    Ok(())
}
