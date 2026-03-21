//! Real-time microphone → VAD + ETD demo.
//!
//! Usage:
//!   cargo run -p speech-capture --features end-of-turn --example etd_speech
//!
//! Requires: smart_turn_v3.onnx in assets/models/

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config = speech_capture::SpeechConfig {
        emit_vad_status: false,
        #[cfg(feature = "end-of-turn")]
        etd: Some(etd::EtdConfig::default()),
        ..Default::default()
    };

    let mut sc = speech_capture::SpeechCapture::new(config)?;
    sc.start(|event| match event {
        speech_capture::SpeechEvent::VoiceStart { timestamp } => {
            println!("[{:>7.1}s] Voice start", timestamp.as_secs_f64());
        }
        speech_capture::SpeechEvent::VoiceEnd {
            duration,
            audio,
            transcript,
            end_of_turn,
            turn_probability,
            ..
        } => {
            let eot = end_of_turn.map_or("N/A".into(), |v| format!("{v}"));
            let prob = turn_probability.map_or("N/A".into(), |v| format!("{v:.3}"));
            println!(
                "           Voice end  ({:.1}s, {} samples)",
                duration.as_secs_f64(),
                audio.len()
            );
            if let Some(text) = transcript {
                println!("           Transcript: {text}");
            }
            println!("           ETD: end_of_turn={eot} probability={prob}");
        }
        _ => {}
    })?;

    println!("Listening... Press Ctrl+C to stop.");
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
