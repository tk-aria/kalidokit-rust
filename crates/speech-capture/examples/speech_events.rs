//! Real-time speech event detection from microphone.
//!
//! ```sh
//! cargo run -p speech-capture --example speech_events
//! ```

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config = speech_capture::SpeechConfig {
        emit_vad_status: false,
        ..Default::default()
    };

    let mut sc = speech_capture::SpeechCapture::new(config)?;
    println!("Listening... Speak into your microphone. Press Ctrl+C to stop.\n");

    sc.start(|event| match event {
        speech_capture::SpeechEvent::VoiceStart { timestamp } => {
            println!("[{:>7.1}s] Voice start", timestamp.as_secs_f64());
        }
        speech_capture::SpeechEvent::VoiceEnd {
            duration, audio, ..
        } => {
            println!(
                "           Voice end  ({:.1}s, {} samples)",
                duration.as_secs_f64(),
                audio.len()
            );
        }
        speech_capture::SpeechEvent::VadStatus {
            probability,
            is_voice,
            ..
        } => {
            if is_voice {
                print!(".");
                use std::io::Write;
                std::io::stdout().flush().ok();
            }
            let _ = probability;
        }
    })?;

    // Park main thread - Ctrl+C to exit.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
