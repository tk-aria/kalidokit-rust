//! Real-time speech event detection from microphone.
//!
//! ```sh
//! # Normal output
//! cargo run -p speech-capture --example speech_events
//!
//! # Verbose: JSON log to stdout
//! cargo run -p speech-capture --example speech_events -- --verbose
//!
//! # Verbose: JSON log to file
//! cargo run -p speech-capture --example speech_events -- --verbose --output events.jsonl
//! ```

use std::sync::{Arc, Mutex};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
    let output_path = args
        .windows(2)
        .find(|w| w[0] == "--output" || w[0] == "-o")
        .map(|w| w[1].clone());

    let config = speech_capture::SpeechConfig {
        emit_vad_status: false,
        ..Default::default()
    };

    let mut sc = speech_capture::SpeechCapture::new(config)?;

    if verbose {
        println!("Listening (verbose JSON mode)... Press Ctrl+C to stop.\n");
    } else {
        println!("Listening... Speak into your microphone. Press Ctrl+C to stop.");
        println!("  Use --verbose for JSON output, --output <file> to write to file.\n");
    }

    // JSON logger (shared across callback via Arc<Mutex>)
    let json_logger: Option<Arc<Mutex<Box<dyn std::io::Write + Send>>>> = if verbose {
        let writer: Box<dyn std::io::Write + Send> = match &output_path {
            Some(path) => {
                eprintln!("Writing JSON events to: {path}");
                Box::new(std::io::BufWriter::new(std::fs::File::create(path)?))
            }
            None => Box::new(std::io::stdout()),
        };
        Some(Arc::new(Mutex::new(writer)))
    } else {
        None
    };

    let logger_clone = json_logger.clone();
    sc.start(move |event| {
        // JSON verbose logging
        if let Some(logger) = &logger_clone {
            let record = speech_capture::json_log::SpeechRecord::from_event(&event);
            if let Ok(json) = serde_json::to_string(&record) {
                if let Ok(mut w) = logger.lock() {
                    let _ = writeln!(w, "{json}");
                    let _ = w.flush();
                }
            }
            return; // JSON mode: don't print human-readable output
        }

        // Human-readable output
        match event {
            speech_capture::SpeechEvent::VoiceStart { timestamp } => {
                println!("[{:>7.1}s] Voice start", timestamp.as_secs_f64());
            }
            speech_capture::SpeechEvent::TranscriptInterim { timestamp, text } => {
                println!("[{:>7.1}s] (interim) {text}", timestamp.as_secs_f64());
            }
            speech_capture::SpeechEvent::VoiceEnd {
                duration,
                audio,
                transcript,
                ..
            } => {
                println!(
                    "           Voice end  ({:.1}s, {} samples)",
                    duration.as_secs_f64(),
                    audio.len()
                );
                if let Some(text) = transcript {
                    println!("           Transcript: {text}");
                }
            }
            speech_capture::SpeechEvent::VadStatus { .. } => {}
        }
    })?;

    use std::io::Write;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
