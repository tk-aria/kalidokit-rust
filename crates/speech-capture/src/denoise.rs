//! Real-time noise suppression using nnnoiseless (RNNoise, pure Rust).
//!
//! nnnoiseless operates at 48 kHz with 480-sample frames.
//! Our pipeline is 16 kHz, so we upsample 3x before denoising
//! and downsample 3x after.

use nnnoiseless::DenoiseState;

const DENOISE_FRAME: usize = 480; // nnnoiseless fixed frame size (48 kHz)
const RATIO: usize = 3; // 48000 / 16000
const INPUT_CHUNK: usize = DENOISE_FRAME / RATIO; // 160 samples at 16 kHz

pub struct AudioDenoiser {
    state: Box<DenoiseState<'static>>,
    /// Accumulator for incoming 16 kHz i16 samples.
    in_buf: Vec<i16>,
    /// Denoised 16 kHz i16 output ready to be drained.
    out_buf: Vec<i16>,
}

impl AudioDenoiser {
    pub fn new() -> Self {
        Self {
            state: DenoiseState::new(),
            in_buf: Vec::with_capacity(INPUT_CHUNK * 2),
            out_buf: Vec::with_capacity(INPUT_CHUNK * 2),
        }
    }

    /// Push 16 kHz i16 samples in, get denoised 16 kHz i16 samples out.
    ///
    /// Output may be shorter or longer than input due to frame buffering.
    pub fn process(&mut self, samples: &[i16]) -> Vec<i16> {
        self.in_buf.extend_from_slice(samples);
        self.out_buf.clear();

        while self.in_buf.len() >= INPUT_CHUNK {
            let chunk: Vec<i16> = self.in_buf.drain(..INPUT_CHUNK).collect();

            // Upsample 16 kHz → 48 kHz (3x linear interpolation)
            let upsampled = upsample_3x(&chunk);

            // Convert i16 → f32 (nnnoiseless expects [-32768, 32767] range)
            let input_f32: Vec<f32> = upsampled.iter().map(|&s| s as f32).collect();

            let mut output_f32 = vec![0.0f32; DENOISE_FRAME];
            self.state.process_frame(&mut output_f32, &input_f32);

            // Downsample 48 kHz → 16 kHz (take every 3rd sample)
            let denoised = downsample_3x(&output_f32);

            self.out_buf.extend(denoised);
        }

        std::mem::take(&mut self.out_buf)
    }
}

/// Upsample by 3x using linear interpolation.
fn upsample_3x(samples: &[i16]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(samples.len() * RATIO);
    for i in 0..samples.len() {
        let s0 = samples[i] as f32;
        let s1 = if i + 1 < samples.len() {
            samples[i + 1] as f32
        } else {
            s0
        };
        out.push(s0);
        out.push(s0 + (s1 - s0) / 3.0);
        out.push(s0 + (s1 - s0) * 2.0 / 3.0);
    }
    out.truncate(DENOISE_FRAME);
    out
}

/// Downsample by 3x (pick every 3rd sample), convert f32 → i16.
fn downsample_3x(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .step_by(RATIO)
        .map(|&s| s.clamp(-32768.0, 32767.0) as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denoise_preserves_length() {
        let mut d = AudioDenoiser::new();
        // Feed exactly INPUT_CHUNK samples
        let input = vec![0i16; INPUT_CHUNK];
        let output = d.process(&input);
        assert_eq!(output.len(), INPUT_CHUNK);
    }

    #[test]
    fn denoise_buffers_partial() {
        let mut d = AudioDenoiser::new();
        // Feed less than INPUT_CHUNK — should buffer, no output yet
        let input = vec![0i16; INPUT_CHUNK / 2];
        let output = d.process(&input);
        assert!(output.is_empty());

        // Feed the rest — now should produce output
        let output = d.process(&input);
        assert_eq!(output.len(), INPUT_CHUNK);
    }

    #[test]
    fn upsample_downsample_roundtrip() {
        let input = vec![100i16, 200, 300, 400];
        let up = upsample_3x(&input);
        assert_eq!(up.len(), 12);
        let down = downsample_3x(&up);
        assert_eq!(down.len(), 4);
        // First sample should be preserved exactly
        assert_eq!(down[0], 100);
    }
}
