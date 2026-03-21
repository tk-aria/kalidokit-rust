//! WAV file → VAD + ETD + Whisper STT 統合検証
//!
//! マイクの代わりに WAV ファイルを読み込み、speech-capture パイプライン全体
//! (VAD segmentation → ETD end-of-turn → Whisper transcription) を検証する。
//!
//! Usage:
//!   # VAD のみ
//!   cargo run -p speech-capture --example wav_pipeline -- <wav_file>
//!
//!   # VAD + ETD
//!   cargo run -p speech-capture --features end-of-turn --example wav_pipeline -- <wav_file>
//!
//!   # VAD + ETD + Whisper STT
//!   cargo run -p speech-capture --features "end-of-turn,stt" --example wav_pipeline -- <wav_file> --model <whisper_model_path>
//!
//! Example:
//!   cargo run -p speech-capture --features end-of-turn --example wav_pipeline -- \
//!     crates/ten-vad/vendor/testset/testset-audio-01.wav

use speech_capture::SpeechEvent;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: wav_pipeline <wav_file> [--model <whisper_model>]");
        std::process::exit(1);
    }
    let wav_path = &args[1];
    let model_path = args
        .iter()
        .position(|a| a == "--model")
        .map(|i| args[i + 1].clone());
    let etd_threshold: f32 = args
        .iter()
        .position(|a| a == "--etd-threshold")
        .map(|i| args[i + 1].parse().expect("invalid --etd-threshold"))
        .unwrap_or(0.5);

    // --- Load WAV ---
    let mut reader = hound::WavReader::open(wav_path)?;
    let spec = reader.spec();
    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    let duration_s = samples.len() as f64 / spec.sample_rate as f64;

    println!("=== WAV Pipeline Test ===");
    println!(
        "Input: {} ({:.2}s, {}Hz, {} ch, {} samples)",
        wav_path, duration_s, spec.sample_rate, spec.channels, samples.len()
    );

    // --- Initialize VAD ---
    let vad_threshold = 0.5;
    let hop_size = vad::HopSize::Samples256;
    let mut vad_instance = vad::TenVad::new(hop_size, vad_threshold)?;

    // --- Initialize VadSegmenter (uses the real speech-capture segmenter) ---
    let min_speech_ms = 200u32;
    let silence_timeout_ms = 500u32;
    let mut seg = speech_capture::segmenter::VadSegmenter::new(min_speech_ms, silence_timeout_ms);

    // --- Initialize ETD and wire to segmenter ---
    #[cfg(feature = "end-of-turn")]
    let etd_detector = {
        let mut config = etd::EtdConfig::default();
        config.threshold = etd_threshold;
        println!("ETD threshold: {:.2}", config.threshold);
        match etd::EndOfTurnDetector::new(config) {
            Ok(det) => {
                println!("ETD: initialized ✓");
                let det = std::sync::Arc::new(std::sync::Mutex::new(det));
                // Wire ETD to segmenter for streaming early-cut + merge mode.
                let det_for_seg = std::sync::Arc::clone(&det);
                seg.set_etd_predict(Box::new(move |audio: &[i16]| {
                    let mut d = det_for_seg.lock().unwrap();
                    match d.predict_i16(audio) {
                        Ok(r) => {
                            println!(
                                "           [ETD stream] {} (prob={:.4})",
                                if r.prediction {
                                    "COMPLETE"
                                } else {
                                    "INCOMPLETE"
                                },
                                r.probability
                            );
                            Some(speech_capture::segmenter::EarlyCutResult {
                                prediction: r.prediction,
                                probability: r.probability,
                            })
                        }
                        Err(e) => {
                            println!("           [ETD stream] error ({e})");
                            None
                        }
                    }
                }));
                Some(det)
            }
            Err(e) => {
                println!("ETD: failed to initialize ({e}), skipping");
                None
            }
        }
    };

    #[cfg(not(feature = "end-of-turn"))]
    println!("ETD: disabled (build with --features end-of-turn)");

    // --- Initialize Whisper ---
    #[cfg(feature = "stt")]
    let whisper_ctx = model_path.as_ref().map(|path| {
        println!("Whisper: loading model from {path}...");
        let start = std::time::Instant::now();
        let ctx = whisper_rs::WhisperContext::new_with_params(
            path,
            whisper_rs::WhisperContextParameters::default(),
        )
        .expect("Failed to load Whisper model");
        println!("Whisper: loaded in {:.1}s ✓", start.elapsed().as_secs_f64());
        ctx
    });

    #[cfg(not(feature = "stt"))]
    if model_path.is_some() {
        println!("Whisper: disabled (build with --features stt)");
    }

    println!("\n--- Processing ---\n");

    // --- Process frames through VAD + Segmenter ---
    let hop_samples = hop_size.as_usize();
    let mut segment_count = 0u32;

    for (i, chunk) in samples.chunks(hop_samples).enumerate() {
        if chunk.len() < hop_samples {
            break;
        }

        let timestamp = Duration::from_secs_f64(i as f64 * hop_samples as f64 / 16000.0);
        let result = vad_instance.process(chunk)?;

        #[allow(unused_mut)]
        for mut event in seg.feed(result.is_voice, chunk, timestamp) {
            match &mut event {
                SpeechEvent::VoiceStart { timestamp } => {
                    println!("[{:>7.2}s] 🎤 Voice START", timestamp.as_secs_f64());
                }
                SpeechEvent::VoiceEnd {
                    timestamp,
                    audio,
                    duration,
                    end_of_turn,
                    turn_probability,
                    ..
                } => {
                    segment_count += 1;
                    println!(
                        "[{:>7.2}s] 🔇 Voice END   (segment #{}, {:.2}s, {} samples)",
                        timestamp.as_secs_f64(),
                        segment_count,
                        duration.as_secs_f64(),
                        audio.len()
                    );

                    // ETD batch fallback (if streaming didn't already set it)
                    #[cfg(feature = "end-of-turn")]
                    if end_of_turn.is_none() {
                        if let Some(ref det) = etd_detector {
                            let etd_start = std::time::Instant::now();
                            match det.lock().unwrap().predict_i16(audio) {
                                Ok(etd_result) => {
                                    let etd_ms = etd_start.elapsed().as_millis();
                                    *end_of_turn = Some(etd_result.prediction);
                                    *turn_probability = Some(etd_result.probability);
                                    println!(
                                        "           ETD: {} (prob={:.4}, {etd_ms}ms) [batch]",
                                        if etd_result.prediction {
                                            "COMPLETE ✓"
                                        } else {
                                            "INCOMPLETE"
                                        },
                                        etd_result.probability,
                                    );
                                }
                                Err(e) => println!("           ETD: error ({e})"),
                            }
                        }
                    } else {
                        #[cfg(feature = "end-of-turn")]
                        println!(
                            "           ETD: {} (prob={:.4}) [early-cut]",
                            if *end_of_turn == Some(true) {
                                "COMPLETE ✓"
                            } else {
                                "INCOMPLETE"
                            },
                            turn_probability.unwrap_or(0.0),
                        );
                    }

                    // Whisper STT
                    #[cfg(feature = "stt")]
                    if let Some(ref ctx) = whisper_ctx {
                        let stt_start = std::time::Instant::now();
                        let mut stt_state = ctx.create_state().expect("whisper state");
                        let mut params = whisper_rs::FullParams::new(
                            whisper_rs::SamplingStrategy::Greedy { best_of: 1 },
                        );
                        params.set_language(Some("ja"));
                        params.set_print_progress(false);
                        params.set_print_realtime(false);
                        params.set_print_timestamps(false);

                        let audio_f32: Vec<f32> =
                            audio.iter().map(|&s| s as f32 / 32768.0).collect();
                        if stt_state.full(params, &audio_f32).is_ok() {
                            let n = stt_state.full_n_segments();
                            let mut text = String::new();
                            for seg_i in 0..n {
                                if let Some(segment) = stt_state.get_segment(seg_i) {
                                    if let Ok(t) = segment.to_str() {
                                        text.push_str(t);
                                    }
                                }
                            }
                            let stt_ms = stt_start.elapsed().as_millis();
                            let rtf = stt_start.elapsed().as_secs_f64()
                                / duration.as_secs_f64();
                            println!(
                                "           STT: \"{}\" ({stt_ms}ms, {rtf:.2}x RT)",
                                text.trim()
                            );
                        }
                    }
                    println!();
                }
                _ => {}
            }
        }
    }

    // Flush: feed a few silent frames to trigger any pending VoiceEnd
    for extra in 0..100 {
        let timestamp = Duration::from_secs_f64(
            (samples.len() as f64 / 16000.0) + (extra as f64 * hop_samples as f64 / 16000.0),
        );
        let silent = vec![0i16; hop_samples];
        for event in seg.feed(false, &silent, timestamp) {
            if let SpeechEvent::VoiceEnd {
                audio, duration, ..
            } = &event
            {
                segment_count += 1;
                println!(
                    "[{:>7.2}s] 🔇 Voice END   (segment #{}, {:.2}s, {} samples) [flush]",
                    timestamp.as_secs_f64(),
                    segment_count,
                    duration.as_secs_f64(),
                    audio.len()
                );

                #[cfg(feature = "end-of-turn")]
                if let Some(ref det) = etd_detector {
                    let etd_start = std::time::Instant::now();
                    match det.lock().unwrap().predict_i16(audio) {
                        Ok(etd_result) => {
                            let etd_ms = etd_start.elapsed().as_millis();
                            println!(
                                "           ETD: {} (prob={:.4}, {etd_ms}ms)",
                                if etd_result.prediction {
                                    "COMPLETE ✓"
                                } else {
                                    "INCOMPLETE"
                                },
                                etd_result.probability,
                            );
                        }
                        Err(e) => println!("           ETD: error ({e})"),
                    }
                }

                #[cfg(feature = "stt")]
                if let Some(ref ctx) = whisper_ctx {
                    let stt_start = std::time::Instant::now();
                    let mut stt_state = ctx.create_state().expect("whisper state");
                    let mut params = whisper_rs::FullParams::new(
                        whisper_rs::SamplingStrategy::Greedy { best_of: 1 },
                    );
                    params.set_language(Some("ja"));
                    params.set_print_progress(false);
                    params.set_print_realtime(false);
                    params.set_print_timestamps(false);

                    let audio_f32: Vec<f32> =
                        audio.iter().map(|&s| s as f32 / 32768.0).collect();
                    if stt_state.full(params, &audio_f32).is_ok() {
                        let n = stt_state.full_n_segments();
                        let mut text = String::new();
                        for seg_i in 0..n {
                            if let Some(segment) = stt_state.get_segment(seg_i) {
                                if let Ok(t) = segment.to_str() {
                                    text.push_str(t);
                                }
                            }
                        }
                        let stt_ms = stt_start.elapsed().as_millis();
                        let rtf =
                            stt_start.elapsed().as_secs_f64() / duration.as_secs_f64();
                        println!(
                            "           STT: \"{}\" ({stt_ms}ms, {rtf:.2}x RT)",
                            text.trim()
                        );
                    }
                }
                println!();
            }
        }
    }

    println!("--- Done ---");
    println!("Total segments: {segment_count}");
    println!("Audio duration: {duration_s:.2}s");

    Ok(())
}
