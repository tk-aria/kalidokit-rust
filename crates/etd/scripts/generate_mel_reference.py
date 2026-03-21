#!/usr/bin/env python3
"""Generate mel spectrogram reference data for Rust accuracy tests.

Usage:
    pip install transformers numpy soundfile
    python scripts/generate_mel_reference.py <input.wav> <output_dir>

Example:
    python scripts/generate_mel_reference.py tests/fixtures/test_audio.wav tests/fixtures/

This generates:
    - <output_dir>/mel_reference.npy  (shape: (80, 800), float32)

The Rust test compares its mel spectrogram output against this reference.
"""

import sys
import numpy as np

def main():
    if len(sys.argv) < 3:
        print(__doc__)
        sys.exit(1)

    wav_path = sys.argv[1]
    output_dir = sys.argv[2]

    try:
        import soundfile as sf
    except ImportError:
        print("Error: pip install soundfile")
        sys.exit(1)

    try:
        from transformers import WhisperFeatureExtractor
    except ImportError:
        print("Error: pip install transformers")
        sys.exit(1)

    audio, sr = sf.read(wav_path)
    if sr != 16000:
        print(f"Warning: sample rate is {sr}, expected 16000. Resampling not implemented.")

    # Use Whisper feature extractor with 8-second chunk (matching ETD config)
    extractor = WhisperFeatureExtractor(chunk_length=8)
    features = extractor(audio, sampling_rate=sr, return_tensors="np")
    mel = features.input_features[0]  # shape (80, 800)

    output_path = f"{output_dir}/mel_reference.npy"
    np.save(output_path, mel)
    print(f"Saved mel reference: {output_path} shape={mel.shape} dtype={mel.dtype}")
    print(f"  min={mel.min():.6f} max={mel.max():.6f} mean={mel.mean():.6f}")


if __name__ == "__main__":
    main()
