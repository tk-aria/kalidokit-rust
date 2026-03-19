//! Streaming STT: emit interim transcription results during speech,
//! then final result at VoiceEnd.
//!
//! Download a model first:
//! ```sh
//! curl -L -o models/ggml-base.bin \
//!   https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin
//! ```
//!
//! Run:
//! ```sh
//! cargo run -p speech-capture --features stt --example streaming_stt
//! ```

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config = speech_capture::SpeechConfig {
        emit_vad_status: false,
        stt: Some(speech_capture::SttConfig {
            model_path: "models/ggml-base.bin".to_string(),
            language: None,
            mode: speech_capture::SttMode::Streaming {
                interim_interval_ms: 1000,
            },
        }),
        ..Default::default()
    };

    let mut sc = speech_capture::SpeechCapture::new(config)?;
    println!("Listening (streaming STT)... Speak into your microphone. Press Ctrl+C to stop.\n");

    sc.start(|event| match event {
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
            match transcript {
                Some(text) => println!("           >>> {text}"),
                None => println!("           (no transcript)"),
            }
        }
        _ => {}
    })?;

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
