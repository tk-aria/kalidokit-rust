//! Benchmark STT latency with a fixed WAV file.
//!
//! ```sh
//! cargo run -p speech-capture --features stt --example bench_stt --release -- <model.bin> <audio.wav> [runs]
//! ```

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: bench_stt <model_path> <wav_path> [runs=5]");
        std::process::exit(1);
    }
    let model_path = &args[1];
    let wav_path = &args[2];
    let runs: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(5);

    // Load WAV
    let mut reader = hound::WavReader::open(wav_path)?;
    let spec = reader.spec();
    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    let audio_duration_s = samples.len() as f64 / spec.sample_rate as f64;
    println!(
        "Audio: {wav_path} ({} Hz, {:.1}s)",
        spec.sample_rate, audio_duration_s
    );

    // Load model
    println!("Loading model: {model_path}");
    let load_start = std::time::Instant::now();
    let ctx = whisper_rs::WhisperContext::new_with_params(
        model_path,
        whisper_rs::WhisperContextParameters::default(),
    )
    .map_err(|e| format!("Failed to load model: {e}"))?;
    println!(
        "Model loaded in {:.1}s\n",
        load_start.elapsed().as_secs_f64()
    );

    let audio_f32: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();

    fn get_text(state: &whisper_rs::WhisperState) -> String {
        let mut text = String::new();
        let n = state.full_n_segments();
        for i in 0..n {
            if let Some(seg) = state.get_segment(i) {
                if let Ok(s) = seg.to_str() {
                    text.push_str(s);
                }
            }
        }
        text.trim().to_string()
    }

    // Warmup
    {
        let mut state = ctx.create_state().map_err(|e| format!("{e}"))?;
        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("ja"));
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        state.full(params, &audio_f32).map_err(|e| format!("{e}"))?;
        println!("Warmup: {}\n", get_text(&state));
    }

    // Benchmark
    println!(
        "Benchmarking {runs} runs ({:.1}s audio)...\n",
        audio_duration_s
    );
    let mut latencies = Vec::new();

    for run in 0..runs {
        let mut state = ctx.create_state().map_err(|e| format!("{e}"))?;
        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("ja"));
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let start = std::time::Instant::now();
        state.full(params, &audio_f32).map_err(|e| format!("{e}"))?;
        let elapsed = start.elapsed();
        latencies.push(elapsed);

        println!(
            "  Run {}: {:.0}ms ({:.2}x realtime) | {}",
            run + 1,
            elapsed.as_millis(),
            elapsed.as_secs_f64() / audio_duration_s,
            get_text(&state)
        );
    }

    let avg_ms =
        latencies.iter().map(|d| d.as_millis()).sum::<u128>() as f64 / latencies.len() as f64;
    let min_ms = latencies.iter().map(|d| d.as_millis()).min().unwrap();
    let max_ms = latencies.iter().map(|d| d.as_millis()).max().unwrap();
    println!("\n--- Summary ---");
    println!("Audio: {:.1}s | Runs: {runs}", audio_duration_s);
    println!(
        "Latency: avg={:.0}ms  min={min_ms}ms  max={max_ms}ms",
        avg_ms
    );
    println!("Realtime: {:.2}x", avg_ms / 1000.0 / audio_duration_s);

    Ok(())
}
