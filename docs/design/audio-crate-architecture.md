# 音声クレートアーキテクチャ設計

## 1. クレート構成

```
speech-capture (音声文字キャプチャ)
  ├── audio-capture (マイク機能)
  └── ten-vad (VAD)
```

### audio-capture — マイク音声キャプチャ

**責務**: OS のマイクから音声をキャプチャし、16kHz mono i16 のストリームとして提供

```rust
use audio_capture::{AudioCapture, AudioFrame};

let mut capture = AudioCapture::new()?;
capture.start(|frame: AudioFrame| {
    // frame.samples: &[i16] — 16kHz mono PCM
    // frame.sample_rate: 16000
    // frame.timestamp: Duration
})?;
```

| 項目 | 内容 |
|------|------|
| 入力 | OS デフォルトマイク (cpal) |
| 出力 | 16kHz mono i16 フレーム (コールバック or channel) |
| 処理 | リサンプリング (48kHz→16kHz), ステレオ→モノ変換, f32→i16 変換 |
| 依存 | `cpal` |

### ten-vad — 音声区間検出 (既存)

**責務**: 16kHz i16 のオーディオフレームを受け取り、voice/non-voice を判定

```rust
use vad::{TenVad, HopSize};

let mut vad = TenVad::new(HopSize::Samples256, 0.5)?;
let result = vad.process(&frame)?;  // → VadResult { probability, is_voice }
```

### speech-capture — 音声文字キャプチャ

**責務**: マイクからリアルタイムで音声をキャプチャし、VAD で音声区間を検出、セグメント化して将来の STT (Speech-to-Text) に渡せる形にする

```rust
use speech_capture::{SpeechCapture, SpeechEvent};

let mut sc = SpeechCapture::new(Default::default())?;
sc.start(|event: SpeechEvent| {
    match event {
        SpeechEvent::VoiceStart { timestamp } => { /* 発話開始 */ }
        SpeechEvent::VoiceEnd { timestamp, audio } => {
            // audio: Vec<i16> — 発話区間の全サンプル
            // → ここで STT に渡す
        }
        SpeechEvent::VadStatus { probability, is_voice } => { /* フレーム毎 */ }
    }
})?;
```

| 項目 | 内容 |
|------|------|
| 入力 | audio-capture からの 16kHz ストリーム |
| 処理 | ten-vad でフレーム毎に VAD → 音声区間セグメント化 |
| 出力 | SpeechEvent (VoiceStart, VoiceEnd + audio data, VadStatus) |
| 依存 | `audio-capture`, `ten-vad` |

## 2. データフロー

```
OS Microphone (48kHz f32 stereo)
  │
  ▼
┌─────────────────────────────────┐
│  audio-capture                  │
│  cpal → resample → mono → i16  │
│  Output: AudioFrame (256 samples @ 16kHz) │
└──────────────┬──────────────────┘
               │ channel / callback
               ▼
┌─────────────────────────────────┐
│  speech-capture                 │
│                                 │
│  ┌──────────┐                   │
│  │ ten-vad  │ process(frame)    │
│  │          │ → VadResult       │
│  └──────────┘                   │
│                                 │
│  State machine:                 │
│    Idle → Speaking → Idle       │
│                                 │
│  Speaking 中は audio を蓄積     │
│  → VoiceEnd で audio chunk を   │
│    コールバックに渡す            │
└──────────────┬──────────────────┘
               │ SpeechEvent
               ▼
         Application / STT
```

## 3. ファイル構成

```
crates/
├── audio-capture/
│   ├── Cargo.toml         # depends: cpal
│   └── src/
│       ├── lib.rs         # AudioCapture, AudioFrame, AudioConfig
│       └── resample.rs    # リサンプリング + フォーマット変換
│
├── ten-vad/               # 既存 (変更なし)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── ffi.rs
│
└── speech-capture/
    ├── Cargo.toml         # depends: audio-capture, ten-vad (vad)
    └── src/
        ├── lib.rs         # SpeechCapture, SpeechEvent, SpeechConfig
        └── segmenter.rs   # VAD ベースの音声区間セグメンター
```

## 4. 各クレート API

### audio-capture/src/lib.rs

```rust
pub struct AudioFrame {
    pub samples: Vec<i16>,      // 16kHz mono PCM
    pub sample_rate: u32,       // 常に 16000
    pub timestamp: std::time::Duration,
}

pub struct AudioConfig {
    pub device_name: Option<String>,  // None = デフォルト
    pub frame_size: usize,            // default: 256
}

pub struct AudioCapture { /* ... */ }

impl AudioCapture {
    pub fn new(config: AudioConfig) -> Result<Self, AudioError>;
    pub fn start<F: FnMut(AudioFrame) + Send + 'static>(&mut self, callback: F) -> Result<(), AudioError>;
    pub fn stop(&mut self);
    pub fn is_running(&self) -> bool;
    /// List available input devices.
    pub fn list_devices() -> Result<Vec<String>, AudioError>;
}
```

### speech-capture/src/lib.rs

```rust
pub enum SpeechEvent {
    /// Voice activity started.
    VoiceStart { timestamp: std::time::Duration },
    /// Voice activity ended. `audio` contains the complete utterance.
    VoiceEnd {
        timestamp: std::time::Duration,
        audio: Vec<i16>,
        duration: std::time::Duration,
    },
    /// Per-frame VAD status (optional, for visualization).
    VadStatus {
        timestamp: std::time::Duration,
        probability: f32,
        is_voice: bool,
    },
}

pub struct SpeechConfig {
    pub vad_threshold: f32,          // default: 0.5
    pub hop_size: vad::HopSize,      // default: Samples256
    pub min_speech_duration_ms: u32, // default: 200 (ignore < 200ms)
    pub silence_timeout_ms: u32,     // default: 500 (end after 500ms silence)
    pub emit_vad_status: bool,       // default: false
    pub audio_config: audio_capture::AudioConfig,
}

pub struct SpeechCapture { /* ... */ }

impl SpeechCapture {
    pub fn new(config: SpeechConfig) -> Result<Self, SpeechError>;
    pub fn start<F: FnMut(SpeechEvent) + Send + 'static>(&mut self, callback: F) -> Result<(), SpeechError>;
    pub fn stop(&mut self);
    pub fn is_running(&self) -> bool;
}
```
