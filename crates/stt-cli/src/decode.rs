//! Decode audio files (Opus/OGG/WAV/MP3) to i16 16kHz mono using Symphonia + Rubato.

use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CodecRegistry, DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Build a codec registry that includes libopus for Opus decoding.
fn codec_registry() -> CodecRegistry {
    let mut registry = CodecRegistry::new();
    symphonia::default::register_enabled_codecs(&mut registry);
    registry.register_all::<symphonia_adapter_libopus::OpusDecoder>();
    registry
}

/// Decode an audio file to i16 samples at 16kHz mono.
pub fn decode_to_i16_16khz(path: &Path) -> Result<Vec<i16>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot open file: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("Probe failed: {e}"))?;

    let mut format = probed.format;

    // Find the first audio track
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("No audio track found")?;

    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or("Unknown sample rate")?;
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(1);

    log::info!(
        "Audio track: {}Hz, {} channels, codec={:?}",
        sample_rate,
        channels,
        track.codec_params.codec
    );

    let registry = codec_registry();
    let mut decoder = registry
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Decoder creation failed: {e}"))?;

    // Decode all packets → f32 interleaved
    let mut all_f32: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                log::warn!("Packet read error: {e}");
                break;
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Decode error: {e}");
                continue;
            }
        };

        let spec = *decoded.spec();
        let n_frames = decoded.capacity();
        let mut sample_buf = SampleBuffer::<f32>::new(n_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        all_f32.extend_from_slice(sample_buf.samples());
    }

    if all_f32.is_empty() {
        return Err("No audio samples decoded".into());
    }

    // Mix down to mono if multi-channel
    let mono_f32 = if channels > 1 {
        let mut mono = Vec::with_capacity(all_f32.len() / channels);
        for chunk in all_f32.chunks(channels) {
            let sum: f32 = chunk.iter().sum();
            mono.push(sum / channels as f32);
        }
        mono
    } else {
        all_f32
    };

    // Resample to 16kHz if needed
    let resampled = if sample_rate != 16000 {
        resample(&mono_f32, sample_rate, 16000)?
    } else {
        mono_f32
    };

    // Convert f32 → i16
    let i16_samples: Vec<i16> = resampled
        .iter()
        .map(|&s| {
            let clamped = s.clamp(-1.0, 1.0);
            (clamped * 32767.0) as i16
        })
        .collect();

    Ok(i16_samples)
}

/// Resample mono f32 audio from src_rate to dst_rate using Rubato.
fn resample(input: &[f32], src_rate: u32, dst_rate: u32) -> Result<Vec<f32>, String> {
    use rubato::{FftFixedIn, Resampler};

    let chunk_size = 1024;
    let mut resampler = FftFixedIn::<f32>::new(
        src_rate as usize,
        dst_rate as usize,
        chunk_size,
        2, // sub_chunks
        1, // channels (mono)
    )
    .map_err(|e| format!("Resampler init failed: {e}"))?;

    let mut output = Vec::new();
    let mut pos = 0;

    while pos + chunk_size <= input.len() {
        let chunk = vec![input[pos..pos + chunk_size].to_vec()];
        let out = resampler
            .process(&chunk, None)
            .map_err(|e| format!("Resample error: {e}"))?;
        output.extend_from_slice(&out[0]);
        pos += chunk_size;
    }

    // Handle remaining samples by zero-padding
    if pos < input.len() {
        let mut last_chunk = input[pos..].to_vec();
        last_chunk.resize(chunk_size, 0.0);
        let chunk = vec![last_chunk];
        let out = resampler
            .process(&chunk, None)
            .map_err(|e| format!("Resample error (tail): {e}"))?;
        // Only take proportional amount
        let expected = ((input.len() - pos) as f64 * dst_rate as f64 / src_rate as f64) as usize;
        let take = expected.min(out[0].len());
        output.extend_from_slice(&out[0][..take]);
    }

    log::info!(
        "Resampled: {}Hz → {}Hz ({} → {} samples)",
        src_rate,
        dst_rate,
        input.len(),
        output.len()
    );
    Ok(output)
}
