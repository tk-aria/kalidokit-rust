//! Chunk-based transcription using whisper-rs directly.

use std::cell::RefCell;

/// A transcribed segment with timing.
#[derive(Debug, serde::Serialize)]
pub struct Segment {
    pub start_secs: f64,
    pub end_secs: f64,
    pub text: String,
}

pub struct WhisperConfig {
    pub model_path: String,
    pub language: Option<String>,
}

/// Transcribe audio by splitting into chunks and running Whisper on each.
pub fn transcribe_chunks(
    samples: &[i16],
    config: &WhisperConfig,
    chunk_secs: u32,
) -> Result<Vec<Segment>, String> {
    let ctx = whisper_rs::WhisperContext::new_with_params(
        &config.model_path,
        whisper_rs::WhisperContextParameters::default(),
    )
    .map_err(|e| format!("Failed to load Whisper model: {e}"))?;

    let state = RefCell::new(
        ctx.create_state()
            .map_err(|e| format!("Failed to create Whisper state: {e}"))?,
    );

    log::info!("Whisper model loaded, state created (reusable)");

    let audio_f32: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();
    let chunk_samples = chunk_secs as usize * 16000;
    let mut segments = Vec::new();
    let mut offset = 0;

    while offset < audio_f32.len() {
        let end = (offset + chunk_samples).min(audio_f32.len());
        let chunk = &audio_f32[offset..end];

        if chunk.len() < 8000 {
            break;
        }

        let start_secs = offset as f64 / 16000.0;
        let end_secs = end as f64 / 16000.0;

        log::info!(
            "Transcribing [{:.1}s - {:.1}s] ({} samples)",
            start_secs,
            end_secs,
            chunk.len()
        );

        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        if let Some(lang) = &config.language {
            params.set_language(Some(lang));
        }
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_single_segment(false);

        let mut st = state.borrow_mut();
        if let Err(e) = st.full(params, chunk) {
            log::warn!("Whisper error at {:.1}s: {e}", start_secs);
            offset = end;
            continue;
        }

        let n_seg = st.full_n_segments();
        let mut text = String::new();
        let mut max_no_speech: f32 = 0.0;
        for i in 0..n_seg {
            if let Some(seg) = st.get_segment(i) {
                let prob = seg.no_speech_probability();
                if prob > max_no_speech {
                    max_no_speech = prob;
                }
                if let Ok(t) = seg.to_str() {
                    text.push_str(t);
                }
            }
        }

        let text = text.trim().to_string();
        if !text.is_empty() && max_no_speech < 0.6 {
            segments.push(Segment {
                start_secs,
                end_secs,
                text,
            });
        }

        offset = end;
    }

    Ok(segments)
}
