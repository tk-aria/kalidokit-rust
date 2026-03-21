# ETD (End-of-Turn Detection) — Implementation TODO

> **リファレンス**: [pipecat-ai/smart-turn](https://github.com/pipecat-ai/smart-turn)
> **設計**: [RFC](./rfc.md) / [SoW](./sow.md)
> **作成日**: 2026-03-21

---

## Phase 1: クレート基盤 + 音声前処理

### Step 1.1: クレート scaffolding

- [x] `crates/etd/Cargo.toml` を作成 <!-- 2026-03-21 11:27 JST -->

```toml
[package]
name = "etd"
version = "0.1.0"
edition = "2021"
description = "End-of-Turn Detection using smart-turn v3 ONNX model"

[dependencies]
ort = { workspace = true }           # 2.0.0-rc.12
ndarray = { workspace = true }       # 0.17
thiserror = { workspace = true }     # 2.0
log = { workspace = true }           # 0.4
rustfft = "6.4"                      # STFT 計算

[dev-dependencies]
hound = "3.5"                        # WAV 読み込み (test/example)
approx = "0.5"                       # 浮動小数点近似比較
```

- [x] ルート `Cargo.toml` の `workspace.members` に <!-- 2026-03-21 11:27 JST --> `"crates/etd"` を追加
- [x] `crates/etd/src/lib.rs` を作成 — モジュール宣言のみ <!-- 2026-03-21 11:27 JST -->

```rust
pub mod audio;
pub mod mel;
// pub mod inference;  // Phase 2 で追加

mod error;
pub use error::EtdError;
```

- [x] `crates/etd/src/error.rs` を作成 — エラー型定義 <!-- 2026-03-21 11:27 JST -->

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EtdError {
    #[error("model load failed: {0}")]
    ModelLoad(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("invalid audio: {0}")]
    InvalidAudio(String),
}
```

- [x] `cargo check -p etd` が通ることを確認 <!-- 2026-03-21 11:27 JST -->

### Step 1.2: 音声前処理 — `audio.rs` (domain 層)

`speech-capture` の `Vec<i16>` 16kHz mono をそのまま受け取り、ETD 推論に適した `Vec<f32>` に変換する。

- [ ] `crates/etd/src/audio.rs` を作成

**実装する関数:**

```rust
/// i16 PCM [-32768, 32767] → f32 [-1.0, 1.0]
/// リファレンス: smart-turn/audio_utils.py (暗黙的に行われる変換)
pub fn i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32 / 32768.0).collect()
}

/// 8秒に切り詰め/パディング
/// - 8秒超: 末尾8秒を切り出し
/// - 8秒未満: **先頭をゼロパディング** (smart-turn の仕様)
/// リファレンス: smart-turn/audio_utils.py `truncate_audio_to_last_n_seconds()`
pub fn truncate_or_pad(audio: &[f32], sample_rate: u32, max_seconds: f32) -> Vec<f32> {
    let max_samples = (sample_rate as f32 * max_seconds) as usize;
    if audio.len() > max_samples {
        // 末尾 max_samples を取得
        audio[audio.len() - max_samples..].to_vec()
    } else if audio.len() < max_samples {
        // 先頭ゼロパディング
        let mut padded = vec![0.0f32; max_samples - audio.len()];
        padded.extend_from_slice(audio);
        padded
    } else {
        audio.to_vec()
    }
}
```

- [ ] ユニットテスト (正常系):
  - `test_i16_to_f32_zero` — `0i16 → 0.0f32`
  - `test_i16_to_f32_max` — `32767i16 → ≈ 1.0f32`
  - `test_i16_to_f32_min` — `-32768i16 → -1.0f32`
  - `test_truncate_exact_8s` — 128000 サンプル (8s @ 16kHz) → そのまま返却
  - `test_truncate_longer` — 192000 サンプル (12s) → 末尾 128000 サンプル
  - `test_pad_shorter` — 64000 サンプル (4s) → 先頭に 64000 ゼロ + 元データ
  - `test_pad_empty` — 空配列 → 128000 ゼロ

- [ ] ユニットテスト (異常系):
  - `test_truncate_zero_max_seconds` — `max_seconds=0.0` → 空の Vec を返す
  - `test_i16_to_f32_empty` — 空スライス → 空 Vec

- [ ] `cargo test -p etd` が通ることを確認

### Step 1.3: Mel フィルタバンク — `mel.rs` 内部関数 (domain 層)

Whisper 互換の mel フィルタバンクを構築する。三角フィルタの生成。

- [ ] `crates/etd/src/mel.rs` を作成

**実装する型と関数:**

```rust
/// Whisper 互換の mel スペクトログラム設定
/// リファレンス: transformers WhisperFeatureExtractor のデフォルト値
pub struct MelConfig {
    pub n_fft: usize,           // 400 (25ms @ 16kHz)
    pub hop_length: usize,      // 160 (10ms @ 16kHz)
    pub n_mels: usize,          // 80
    pub sample_rate: u32,       // 16000
    pub fmin: f32,              // 80.0 Hz
    pub fmax: f32,              // 7600.0 Hz
    pub chunk_length: f32,      // 8.0 秒
}

impl Default for MelConfig { /* Whisper デフォルト値 */ }

/// Hz → mel スケール変換
/// リファレンス: librosa mel_frequencies, HTK 式
/// mel = 2595 * log10(1 + hz / 700)
fn hz_to_mel(hz: f32) -> f32;

/// mel → Hz 変換
fn mel_to_hz(mel: f32) -> f32;

/// 三角フィルタバンク生成 (n_mels × (n_fft/2 + 1))
/// リファレンス: smart-turn/train.py WhisperFeatureExtractor 内部
/// 出力: Vec<Vec<f32>> — shape (n_mels, n_fft/2+1)
fn mel_filterbank(n_mels: usize, n_fft: usize, sr: u32, fmin: f32, fmax: f32) -> Vec<Vec<f32>>;
```

- [ ] ユニットテスト (正常系):
  - `test_hz_to_mel_known_values` — 80Hz, 1000Hz, 7600Hz の変換値を検証
  - `test_mel_to_hz_roundtrip` — `mel_to_hz(hz_to_mel(x)) ≈ x`
  - `test_filterbank_shape` — 80 × 201 (n_fft/2+1 = 400/2+1)
  - `test_filterbank_sum_approx_one` — 各周波数ビンでフィルタの合計 ≈ 1.0 (帯域内)
  - `test_filterbank_no_negative` — 全要素 ≥ 0.0

- [ ] ユニットテスト (異常系):
  - `test_filterbank_fmin_equals_fmax` — fmin == fmax → 全ゼロフィルタ
  - `test_filterbank_zero_mels` — n_mels=0 → 空ベクタ

> **注意**: `mel.rs` は mel filterbank + STFT + log-mel 正規化を含むため、300行を超える可能性あり。
> STFT 部分を `stft.rs` に分割することを推奨。

### Step 1.4: STFT 実装 — `stft.rs` (domain 層)

Hann 窓付き Short-Time Fourier Transform を実装。

- [ ] `crates/etd/src/stft.rs` を作成

```rust
use rustfft::{FftPlanner, num_complex::Complex};

/// Hann 窓関数を生成
/// リファレンス: numpy.hanning(n)
/// w(n) = 0.5 * (1 - cos(2π * n / (N-1)))
pub fn hann_window(size: usize) -> Vec<f32>;

/// Short-Time Fourier Transform
/// 入力: f32 PCM, n_fft: FFTサイズ, hop: フレームシフト
/// 出力: Vec<Vec<f32>> — shape (n_frames, n_fft/2+1) パワースペクトル
/// リファレンス: librosa.stft + np.abs()**2
pub fn stft_power(audio: &[f32], n_fft: usize, hop: usize, window: &[f32]) -> Vec<Vec<f32>> {
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n_fft);
    let half = n_fft / 2 + 1;
    let n_frames = (audio.len().saturating_sub(n_fft)) / hop + 1;

    let mut result = Vec::with_capacity(n_frames);
    for i in 0..n_frames {
        let start = i * hop;
        let mut frame: Vec<Complex<f32>> = (0..n_fft)
            .map(|j| {
                let sample = if start + j < audio.len() { audio[start + j] } else { 0.0 };
                Complex::new(sample * window[j], 0.0)
            })
            .collect();

        fft.process(&mut frame);

        let power: Vec<f32> = frame[..half]
            .iter()
            .map(|c| c.norm_sqr()) // |c|^2 = re^2 + im^2
            .collect();
        result.push(power);
    }
    result
}
```

- [ ] ユニットテスト (正常系):
  - `test_hann_window_endpoints` — hann[0] ≈ 0.0, hann[N/2] ≈ 1.0
  - `test_hann_window_symmetry` — hann[i] ≈ hann[N-1-i]
  - `test_stft_silence` — 全ゼロ入力 → 全ゼロパワー
  - `test_stft_sine_wave` — 440Hz 正弦波 → 440Hz 付近にピーク
  - `test_stft_output_shape` — 128000 サンプル → (n_frames, 201) の形状

- [ ] ユニットテスト (異常系):
  - `test_stft_empty_audio` — 空入力 → 空出力
  - `test_stft_shorter_than_nfft` — n_fft 未満の入力 → 0 フレーム

### Step 1.5: Log-mel スペクトログラム生成 — `mel.rs` 公開関数

STFT + mel filterbank + log + 正規化を統合する。

- [ ] `mel.rs` に以下の公開関数を追加

```rust
/// PCM f32 → log-mel spectrogram
/// 処理フロー:
///   1. STFT (Hann窓, n_fft=400, hop=160)
///   2. mel filterbank 適用
///   3. log10 変換 + clamp (max - 8.0 dB)
///   4. 正規化 ((x - max) / max_val * 4.0 + 4.0) — Whisper 方式
/// リファレンス: transformers WhisperFeatureExtractor.__call__()
/// 出力: Vec<f32> — row-major (n_mels, n_frames) = (80, 800)
pub fn log_mel_spectrogram(audio: &[f32], config: &MelConfig) -> Vec<f32> {
    let window = stft::hann_window(config.n_fft);
    let power_spec = stft::stft_power(audio, config.n_fft, config.hop_length, &window);
    let filters = mel_filterbank(config.n_mels, config.n_fft, config.sample_rate, config.fmin, config.fmax);

    let n_frames = power_spec.len();
    let mut mel_spec = vec![0.0f32; config.n_mels * n_frames];

    // mel filterbank 適用
    for (t, frame) in power_spec.iter().enumerate() {
        for (m, filter) in filters.iter().enumerate() {
            let val: f32 = filter.iter().zip(frame.iter()).map(|(f, p)| f * p).sum();
            mel_spec[m * n_frames + t] = val.max(1e-10); // フロア
        }
    }

    // log10 変換
    for v in mel_spec.iter_mut() {
        *v = v.log10();
    }

    // Whisper 正規化: clamp(max - 8.0) → scale to [-1, 1] range
    let max_val = mel_spec.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    for v in mel_spec.iter_mut() {
        *v = (*v).max(max_val - 8.0);
        *v = (*v - max_val) / 4.0 + 1.0;
    }

    mel_spec
}
```

- [ ] `lib.rs` に `pub mod stft;` を追加

- [ ] ユニットテスト (正常系):
  - `test_log_mel_silence` — 全ゼロ入力 → 出力形状が (80, 800)、全値が同一 (正規化後)
  - `test_log_mel_output_shape` — 128000 サンプル (8s) → 80 × 800 = 64000 要素
  - `test_log_mel_non_nan` — 出力に NaN/Inf が含まれないこと
  - `test_log_mel_range` — 出力値が概ね [-1.0, 1.0] の範囲内

- [ ] ユニットテスト (異常系):
  - `test_log_mel_short_audio` — 160 サンプル (10ms) → パディング後に正常出力

### Step 1.6: Phase 1 検証

- [ ] テストカバレッジ確認:
  ```bash
  cargo install cargo-tarpaulin  # 未インストールの場合
  cargo tarpaulin -p etd --out stdout
  ```
  - カバレッジ 90% 以上であること
  - 未カバーの分岐がある場合、正常系/異常系のテストケースを追加

- [ ] ビルド検証:
  ```bash
  cargo check -p etd
  cargo build -p etd
  cargo test -p etd
  cargo clippy -p etd -- -D warnings
  cargo fmt -p etd --check
  ```
  - 全コマンドがエラーなしで完了すること
  - 警告が出る場合は修正すること

---

## Phase 2: ONNX 推論 + 公開 API

### Step 2.1: ONNX モデル取得

- [ ] HuggingFace から smart-turn v3 ONNX モデルをダウンロード
  ```bash
  # FP32 版 (~32MB)
  curl -L -o assets/models/smart_turn_v3.onnx \
    "https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/main/onnx/model.onnx"
  ```
- [ ] `.gitignore` に ONNX モデルが含まれていないことを確認（大きいファイルの場合は追加）
- [ ] ort でモデルがロードできることを手動確認

### Step 2.2: 推論エンジン — `inference.rs` (usecase 層)

ONNX モデルの読み込みと推論を行うモジュール。

- [ ] `crates/etd/src/inference.rs` を作成

```rust
use ort::Session;
use ndarray::Array3;
use crate::error::EtdError;

/// ONNX セッションのラッパー
/// ort::Session は内部で Sync なので &self で推論可能
pub struct EtdSession {
    session: Session,
}

impl EtdSession {
    /// ONNX モデルをロード
    /// リファレンス: smart-turn/inference.py predict_endpoint()
    pub fn load(model_path: &std::path::Path) -> Result<Self, EtdError> {
        let session = Session::builder()
            .and_then(|b| b.with_intra_threads(1))  // CPU 最適化
            .and_then(|b| b.commit_from_file(model_path))
            .map_err(|e| EtdError::ModelLoad(e.to_string()))?;
        Ok(Self { session })
    }

    /// mel スペクトログラムから推論
    /// 入力: mel_features shape (80, n_frames) row-major
    /// 出力: sigmoid 確率値 [0.0, 1.0]
    pub fn infer(&self, mel_features: &[f32], n_mels: usize, n_frames: usize) -> Result<f32, EtdError> {
        let input = Array3::from_shape_vec(
            (1, n_mels, n_frames),
            mel_features.to_vec(),
        ).map_err(|e| EtdError::Inference(format!("shape error: {e}")))?;

        let outputs = self.session
            .run(ort::inputs![input].map_err(|e| EtdError::Inference(e.to_string()))?)
            .map_err(|e| EtdError::Inference(e.to_string()))?;

        let output_tensor = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| EtdError::Inference(e.to_string()))?;

        let logit = output_tensor.as_slice()
            .ok_or_else(|| EtdError::Inference("empty output".into()))?[0];

        // sigmoid
        let probability = 1.0 / (1.0 + (-logit).exp());
        Ok(probability)
    }
}
```

- [ ] `lib.rs` に `mod inference;` を追加（非公開 — 公開 API 経由でのみアクセス）

- [ ] ユニットテスト (正常系): ※ モデルファイルが必要なため `#[ignore]` 付き
  - `test_session_load` — モデルのロードが成功すること
  - `test_infer_silence` — 全ゼロ mel → probability < 0.5 (incomplete)
  - `test_infer_output_range` — 出力が [0.0, 1.0] の範囲内

- [ ] ユニットテスト (異常系):
  - `test_session_load_missing_file` — 存在しないパス → `EtdError::ModelLoad`
  - `test_infer_wrong_shape` — 不正な shape → `EtdError::Inference`

### Step 2.3: 公開 API — `lib.rs` (interface 層)

全モジュールを統合した公開 API。

- [ ] `crates/etd/src/lib.rs` を更新

```rust
use std::path::PathBuf;

pub mod audio;
pub mod mel;
pub mod stft;
mod inference;
mod error;

pub use error::EtdError;

/// ETD 設定
#[derive(Debug, Clone)]
pub struct EtdConfig {
    /// ONNX モデルファイルパス
    pub model_path: PathBuf,
    /// 判定閾値 (default: 0.5)
    pub threshold: f32,
    /// 最大入力長 (default: 8.0 秒)
    pub max_audio_seconds: f32,
    /// 入力サンプルレート (default: 16000)
    pub sample_rate: u32,
}

impl Default for EtdConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("assets/models/smart_turn_v3.onnx"),
            threshold: 0.5,
            max_audio_seconds: 8.0,
            sample_rate: 16000,
        }
    }
}

/// ETD 判定結果
#[derive(Debug, Clone, Copy)]
pub struct EtdResult {
    /// true = 発話完了 (probability >= threshold)
    pub prediction: bool,
    /// 生の確率値 [0.0, 1.0]
    pub probability: f32,
}

/// End-of-Turn Detector
pub struct EndOfTurnDetector {
    session: inference::EtdSession,
    config: EtdConfig,
    mel_config: mel::MelConfig,
}

impl EndOfTurnDetector {
    pub fn new(config: EtdConfig) -> Result<Self, EtdError> {
        let session = inference::EtdSession::load(&config.model_path)?;
        let mel_config = mel::MelConfig::default();
        Ok(Self { session, config, mel_config })
    }

    /// i16 16kHz mono PCM から判定 (speech-capture の audio をそのまま渡せる)
    pub fn predict_i16(&self, audio: &[i16]) -> Result<EtdResult, EtdError> {
        let f32_audio = audio::i16_to_f32(audio);
        self.predict(&f32_audio)
    }

    /// f32 PCM から判定
    pub fn predict(&self, audio: &[f32]) -> Result<EtdResult, EtdError> {
        if audio.is_empty() {
            return Err(EtdError::InvalidAudio("empty audio".into()));
        }
        let padded = audio::truncate_or_pad(audio, self.config.sample_rate, self.config.max_audio_seconds);
        let mel = mel::log_mel_spectrogram(&padded, &self.mel_config);
        let n_frames = (self.config.max_audio_seconds * self.config.sample_rate as f32) as usize
            / self.mel_config.hop_length;
        let probability = self.session.infer(&mel, self.mel_config.n_mels, n_frames)?;
        Ok(EtdResult {
            prediction: probability >= self.config.threshold,
            probability,
        })
    }
}
```

- [ ] ユニットテスト (正常系): ※ `#[ignore]` 付き (モデルファイル依存)
  - `test_predict_i16_silence` — 全ゼロ i16 → prediction = false
  - `test_predict_f32_silence` — 全ゼロ f32 → prediction = false
  - `test_predict_result_fields` — prediction と probability が矛盾しないこと

- [ ] ユニットテスト (異常系):
  - `test_predict_empty_audio` — 空スライス → `EtdError::InvalidAudio`
  - `test_new_invalid_model_path` — 不正パス → `EtdError::ModelLoad`

### Step 2.4: Example — `etd_demo.rs`

- [ ] `crates/etd/examples/etd_demo.rs` を作成

```rust
/// WAV ファイルから ETD 判定を行うデモ
/// 使い方: cargo run -p etd --example etd_demo -- path/to/audio.wav
use etd::{EndOfTurnDetector, EtdConfig};
use hound::WavReader;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).expect("Usage: etd_demo <wav_file>");
    let mut reader = WavReader::open(&path)?;
    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();

    println!("Audio: {} samples ({:.2}s)", samples.len(), samples.len() as f32 / 16000.0);

    let detector = EndOfTurnDetector::new(EtdConfig::default())?;
    let result = detector.predict_i16(&samples)?;

    println!("Prediction: {}", if result.prediction { "COMPLETE" } else { "INCOMPLETE" });
    println!("Probability: {:.4}", result.probability);
    Ok(())
}
```

- [ ] `cargo run -p etd --example etd_demo -- <test.wav>` が動作することを確認

### Step 2.5: Phase 2 検証

- [ ] テストカバレッジ確認:
  ```bash
  cargo tarpaulin -p etd --out stdout
  ```
  - カバレッジ 90% 以上であること
  - `inference.rs` のモデル依存テストは `#[ignore]` 付き — カバレッジ対象外を考慮し、audio/mel/stft のカバレッジで補う
  - 未カバー分岐があれば正常系/異常系テストを追加

- [ ] ビルド検証:
  ```bash
  cargo check -p etd
  cargo build -p etd
  cargo test -p etd
  cargo test -p etd -- --ignored  # モデル依存テスト (ONNX ファイルが必要)
  cargo clippy -p etd -- -D warnings
  cargo fmt -p etd --check
  ```
  - 全コマンドがエラーなしで完了すること

---

## Phase 3: speech-capture 統合

### Step 3.1: speech-capture に `etd` feature flag を追加

- [ ] `crates/speech-capture/Cargo.toml` に追加:

```toml
[features]
default = []
stt = ["dep:whisper-rs"]
etd = ["dep:etd"]        # ← 追加

[dependencies]
etd = { path = "../etd", optional = true }  # ← 追加
```

- [ ] `cargo check -p speech-capture` が通ること (etd 無効)
- [ ] `cargo check -p speech-capture --features etd` が通ること

### Step 3.2: SpeechEvent 拡張 — `VoiceEnd` に ETD フィールド追加

- [ ] `crates/speech-capture/src/lib.rs` の `SpeechEvent::VoiceEnd` を更新

```rust
pub enum SpeechEvent {
    VoiceStart { timestamp: Duration },
    TranscriptInterim { timestamp: Duration, text: String },
    VoiceEnd {
        timestamp: Duration,
        audio: Vec<i16>,
        duration: Duration,
        transcript: Option<String>,
        /// ETD 判定結果 (etd feature 有効時のみ Some)
        end_of_turn: Option<bool>,
        /// ETD 確率値 (etd feature 有効時のみ Some)
        turn_probability: Option<f32>,
    },
    VadStatus { timestamp: Duration, probability: f32, is_voice: bool },
}
```

- [ ] 既存テストが壊れないことを確認 — 新フィールドは `Option` なので `None` で互換
- [ ] `json_log.rs` の `SpeechRecord` にも `end_of_turn`, `turn_probability` フィールドを追加

### Step 3.3: SpeechConfig に ETD 設定を追加

- [ ] `crates/speech-capture/src/lib.rs` の `SpeechConfig` を更新

```rust
pub struct SpeechConfig {
    // 既存フィールド ...
    /// ETD 設定 (None = ETD 無効)
    #[cfg(feature = "etd")]
    pub etd: Option<etd::EtdConfig>,
}
```

- [ ] ETD 有効時、`SpeechCapture::new()` 内で `EndOfTurnDetector` を初期化
- [ ] ETD 無効時 (feature off) は従来通りの動作

### Step 3.4: Batch モード統合

`VoiceEnd` 発火時に ETD 判定を実行し、結果を `end_of_turn` / `turn_probability` に付与。

- [ ] `crates/speech-capture/src/lib.rs` のワーカースレッド内、`VoiceEnd` 生成箇所に ETD 呼び出しを追加

```rust
// segmenter.feed() が VoiceEnd を返した場合
SpeechEvent::VoiceEnd { audio, .. } => {
    #[cfg(feature = "etd")]
    let (end_of_turn, turn_probability) = if let Some(ref etd) = self.etd_detector {
        match etd.predict_i16(&audio) {
            Ok(result) => (Some(result.prediction), Some(result.probability)),
            Err(e) => {
                log::warn!("ETD inference failed: {e}");
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    #[cfg(not(feature = "etd"))]
    let (end_of_turn, turn_probability) = (None, None);

    // VoiceEnd イベントに付与して callback に渡す
}
```

- [ ] ユニットテスト (正常系): `#[ignore]` (モデル + マイク依存)
  - `test_batch_etd_returns_result` — VoiceEnd に end_of_turn が Some であること
- [ ] ユニットテスト (異常系):
  - `test_batch_etd_disabled` — `etd: None` 時に end_of_turn = None であること

### Step 3.5: Streaming モード統合 (Early-cut)

`VadSegmenter` の `TrailingSilence` 突入時に ETD を呼び出し、complete なら即 `VoiceEnd` を発火。

- [ ] `crates/speech-capture/src/segmenter.rs` に ETD コールバックを追加

```rust
/// ETD 判定関数の型エイリアス
#[cfg(feature = "etd")]
pub type EtdPredictor = Box<dyn Fn(&[i16]) -> Option<etd::EtdResult> + Send>;

pub struct VadSegmenter {
    // 既存フィールド ...
    #[cfg(feature = "etd")]
    etd_predictor: Option<EtdPredictor>,
}

impl VadSegmenter {
    pub fn feed(&mut self, is_voice: bool, samples: &[i16], timestamp: Duration) -> Vec<SpeechEvent> {
        // ... 既存ロジック ...

        // Speaking → TrailingSilence 遷移時
        if was_speaking && !is_voice {
            #[cfg(feature = "etd")]
            if let Some(ref predictor) = self.etd_predictor {
                if let Some(result) = predictor(&self.audio_buffer) {
                    if result.prediction {
                        // Early-cut: silence_timeout を待たず即 VoiceEnd
                        return vec![SpeechEvent::VoiceEnd {
                            end_of_turn: Some(true),
                            turn_probability: Some(result.probability),
                            // ... other fields ...
                        }];
                    }
                }
            }
        }

        // ... 通常の silence_timeout 待機 ...
    }
}
```

- [ ] ユニットテスト (正常系): `#[ignore]` (モデル依存)
  - `test_streaming_early_cut` — ETD complete 時に silence_timeout 前に VoiceEnd が発火
  - `test_streaming_no_early_cut` — ETD incomplete 時は通常の timeout 待機
- [ ] ユニットテスト (異常系):
  - `test_streaming_etd_error_falls_through` — ETD エラー時は通常動作にフォールバック

### Step 3.6: Example — `etd_speech.rs`

- [ ] `crates/speech-capture/examples/etd_speech.rs` を作成

```rust
/// マイク入力 → VAD + ETD でリアルタイム判定するデモ
/// 使い方: cargo run -p speech-capture --features etd --example etd_speech
use speech_capture::{SpeechCapture, SpeechConfig, SpeechEvent};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SpeechConfig {
        emit_vad_status: false,
        #[cfg(feature = "etd")]
        etd: Some(etd::EtdConfig::default()),
        ..Default::default()
    };

    let mut capture = SpeechCapture::new(config)?;
    capture.start(|event| match event {
        SpeechEvent::VoiceStart { .. } => println!("[Speaking...]"),
        SpeechEvent::VoiceEnd { duration, end_of_turn, turn_probability, .. } => {
            let eot = end_of_turn.map_or("N/A".into(), |v| format!("{v}"));
            let prob = turn_probability.map_or("N/A".into(), |v| format!("{v:.3}"));
            println!("[VoiceEnd] duration={duration:?} end_of_turn={eot} probability={prob}");
        }
        _ => {}
    })?;

    println!("Listening... Press Ctrl+C to stop.");
    std::thread::park();  // メインスレッドを待機
    Ok(())
}
```

### Step 3.7: Phase 3 検証

- [ ] テストカバレッジ確認:
  ```bash
  cargo tarpaulin -p speech-capture --features etd --out stdout
  ```
  - etd 統合部分のカバレッジ 90% 以上
  - 未カバーの分岐があれば正常系/異常系テストを追加

- [ ] ビルド検証:
  ```bash
  # ETD なし (後方互換)
  cargo check -p speech-capture
  cargo test -p speech-capture

  # ETD あり
  cargo check -p speech-capture --features etd
  cargo test -p speech-capture --features etd
  cargo clippy -p speech-capture --features etd -- -D warnings

  # ワークスペース全体
  cargo check --workspace
  cargo clippy --workspace -- -D warnings
  cargo fmt --check
  ```
  - 全コマンドがエラーなしで完了すること

---

## Phase 4: Python 精度検証 + E2E テスト

### Step 4.1: Python との数値一致テスト — `mel_accuracy.rs`

Rust の mel スペクトログラム出力と Python `WhisperFeatureExtractor` の出力を比較。

- [ ] テスト用 WAV ファイルを用意 (16kHz mono, 3秒程度の発話)
- [ ] Python スクリプトで期待値を生成:

```python
# scripts/generate_mel_reference.py
from transformers import WhisperFeatureExtractor
import numpy as np
import soundfile as sf

audio, sr = sf.read("test_audio.wav")
extractor = WhisperFeatureExtractor(chunk_length=8)
features = extractor(audio, sampling_rate=sr, return_tensors="np")
np.save("tests/fixtures/mel_reference.npy", features.input_features[0])
```

- [ ] `crates/etd/tests/mel_accuracy.rs` を作成:
  - `test_mel_matches_python` — Rust 出力と Python 出力の差が全要素で `< 1e-3` であること
  - テストフィクスチャ: `tests/fixtures/test_audio.wav`, `tests/fixtures/mel_reference.npy`

- [ ] 差分が `1e-3` を超える場合は `mel.rs` / `stft.rs` のアルゴリズムを修正

### Step 4.2: E2E 推論テスト

- [ ] 既知の「完了発話」WAV と「未完了発話」WAV を用意
- [ ] `crates/etd/tests/e2e_inference.rs` を作成:

```rust
#[test]
#[ignore] // ONNX モデルファイル依存
fn test_complete_utterance() {
    let detector = EndOfTurnDetector::new(EtdConfig::default()).unwrap();
    let audio = load_wav("tests/fixtures/complete_utterance.wav");
    let result = detector.predict_i16(&audio).unwrap();
    assert!(result.prediction, "Expected complete, got probability={}", result.probability);
}

#[test]
#[ignore]
fn test_incomplete_utterance() {
    let detector = EndOfTurnDetector::new(EtdConfig::default()).unwrap();
    let audio = load_wav("tests/fixtures/incomplete_utterance.wav");
    let result = detector.predict_i16(&audio).unwrap();
    assert!(!result.prediction, "Expected incomplete, got probability={}", result.probability);
}
```

### Step 4.3: Phase 4 検証

- [ ] テストカバレッジ確認:
  ```bash
  cargo tarpaulin -p etd --out stdout
  ```
  - カバレッジ 90% 以上であること

- [ ] ビルド検証:
  ```bash
  cargo check --workspace
  cargo build --workspace
  cargo test --workspace
  cargo clippy --workspace -- -D warnings
  cargo fmt --check
  cargo build --release
  ```
  - 全コマンドがエラーなしで完了すること

---

## Phase 5: 動作確認 + ドキュメント

### Step 5.1: 動作確認 TODO リスト

features.md の全実装内容に基づく動作確認:

- [ ] **etd クレート単体**:
  - [ ] `cargo run -p etd --example etd_demo -- <完了発話.wav>` → `COMPLETE` と表示
  - [ ] `cargo run -p etd --example etd_demo -- <未完了発話.wav>` → `INCOMPLETE` と表示
  - [ ] 8秒超の WAV → 正常に判定 (末尾8秒で判定)
  - [ ] 1秒未満の WAV → 正常に判定 (先頭ゼロパディング)

- [ ] **speech-capture + ETD (Batch モード)**:
  - [ ] `cargo run -p speech-capture --features etd --example etd_speech`
  - [ ] マイクに向かって完全な文を話す → `end_of_turn=true` と表示
  - [ ] 途中で止める (「えーと...」) → `end_of_turn=false` と表示
  - [ ] ETD feature なしでビルド → 従来通り動作 (end_of_turn=None)

- [ ] **speech-capture + ETD (Streaming Early-cut)**:
  - [ ] 完全な文を話した後の無音 → silence_timeout を待たず即 VoiceEnd
  - [ ] 途中停止 → silence_timeout まで待機し、発話再開で Speaking 復帰

- [ ] **ビルド・テスト最終確認**:
  - [ ] `cargo check --workspace`
  - [ ] `cargo build --workspace`
  - [ ] `cargo test --workspace`
  - [ ] `cargo test --workspace -- --ignored` (モデル依存テスト)
  - [ ] `cargo clippy --workspace -- -D warnings`
  - [ ] `cargo fmt --check`
  - [ ] `cargo build --release`

- [ ] 上記いずれかでエラーまたは設計通りの動作にならない場合 → 原因を調査し修正 → 再確認を繰り返す

### Step 5.2: README.md 更新 (英語)

- [ ] `README.md` を更新:
  - ETD クレートの説明を追加
  - クレート構成図を更新 (etd を追加)
  - Install 手順:
    - ソースからのビルド (`cargo build --release`)
    - バイナリダウンロード (`curl` で GitHub Release から `/usr/local/bin` に配置)
    - ONNX モデルのダウンロード手順
  - 環境構築手順 (Rust toolchain, ONNX Runtime, 依存ライブラリ)
  - 絵文字は適度に使用 (各セクション見出しに1つ程度)
  - 英語で記載

```markdown
## 🎤 ETD (End-of-Turn Detection)

Detects whether a user has finished speaking, powered by
[smart-turn v3](https://github.com/pipecat-ai/smart-turn) ONNX model.

### Quick Start

\`\`\`rust
use etd::{EndOfTurnDetector, EtdConfig};

let detector = EndOfTurnDetector::new(EtdConfig::default())?;
let result = detector.predict_i16(&audio_samples)?;
println!("End of turn: {} ({:.2}%)", result.prediction, result.probability * 100.0);
\`\`\`

### Install (Binary)

\`\`\`bash
# Download latest release
curl -L -o /usr/local/bin/kalidokit-rust \
  https://github.com/tk-aria/kalidokit-rust/releases/latest/download/kalidokit-rust-$(uname -m)-apple-darwin.tar.gz
\`\`\`
```

### Step 5.3: README_ja.md 作成 (日本語)

- [ ] `README_ja.md` を作成 — `README.md` の日本語版
  - 全セクションを日本語に翻訳
  - Install 手順、環境構築手順も日本語で記載
  - 絵文字は README.md と同一

### Step 5.4: Phase 5 最終検証

- [ ] `README.md` の Install 手順を実際に実行して動作確認
- [ ] `README_ja.md` の内容が `README.md` と一致していることを確認
- [ ] 全ドキュメントのリンク切れがないことを確認
- [ ] features.md の全チェックボックスが `[x]` であることを確認 (ヘッドレス環境の制約は注記)
