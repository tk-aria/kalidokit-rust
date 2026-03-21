# 🎤 ETD (End-of-Turn Detection)

Detects whether a user has finished speaking, powered by [smart-turn v3](https://github.com/pipecat-ai/smart-turn) ONNX model.

## Overview

ETD complements VAD (Voice Activity Detection) by analyzing acoustic features to determine if silence means "thinking pause" or "turn complete". This enables faster response times in voice-based applications.

### Architecture

```
16kHz mono PCM audio (max 8s)
  → Log-Mel Spectrogram (80 mel bins × 800 frames)
    → Whisper Tiny Encoder (~8M params)
      → Attention Pooling → Classifier
        → Sigmoid → probability [0.0, 1.0]
          → Threshold (0.5) → complete / incomplete
```

## Quick Start

```rust
use etd::{EndOfTurnDetector, EtdConfig};

let mut detector = EndOfTurnDetector::new(EtdConfig::default())?;

// From i16 PCM (e.g., from speech-capture)
let result = detector.predict_i16(&audio_i16)?;

// Or from f32 PCM [-1.0, 1.0]
let result = detector.predict(&audio_f32)?;

println!("End of turn: {} (probability: {:.2}%)",
    result.prediction, result.probability * 100.0);
```

## Model Setup

Download the smart-turn v3 ONNX model (~32MB):

```bash
curl -L -o assets/models/smart_turn_v3.onnx \
  "https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/main/onnx/model.onnx"
```

## Examples

### WAV file demo

```bash
cargo run -p etd --example etd_demo -- path/to/audio.wav
```

### speech-capture integration

```bash
cargo run -p speech-capture --features end-of-turn --example etd_speech
```

## Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `model_path` | `assets/models/smart_turn_v3.onnx` | Path to ONNX model |
| `threshold` | `0.5` | Probability threshold for "turn complete" |
| `max_audio_seconds` | `8.0` | Max audio window (aligned to utterance end) |
| `sample_rate` | `16000` | Expected audio sample rate (Hz) |

## Tests

```bash
# Unit tests (no model required)
cargo test -p etd

# Integration tests (requires ONNX model)
cargo test -p etd -- --ignored

# All tests
cargo test -p etd -- --include-ignored
```

## Design Documents

- [RFC](docs/rfc.md) — Motivation, architecture, alternatives
- [SoW](docs/sow.md) — Work packages, schedule, diagrams
- [features.md](docs/features.md) — Implementation TODO list

## License

MIT
