# ETD (End-of-Turn Detection) クレート — Statement of Work (SoW)

> **プロジェクト名**: etd
> **バージョン**: 0.1.0
> **作成日**: 2026-03-21
> **目的**: VAD 後段で発話完了/未完了を判定する独立ライブラリクレート。smart-turn v3 (Whisper Tiny + 分類器) の ONNX モデルを使用。

---

## 1. スコープ

### 1.1 In Scope (対象)

| カテゴリ | 内容 |
|----------|------|
| 音声前処理 | i16/f32 PCM → トランケート/パディング (8秒, 先頭ゼロ埋め) |
| 特徴量抽出 | Whisper 互換 log-mel スペクトログラム (80×800) を Rust で実装 |
| ONNX 推論 | smart-turn v3 モデルの推論 (ort クレート) |
| 公開 API | `EndOfTurnDetector`, `EtdResult`, `EtdConfig` |
| speech-capture 統合 | optional dependency として ETD 判定を SpeechEvent に付与 |
| テスト | ユニットテスト + Python 版との数値一致検証 |
| Example | WAV ファイルからの ETD 判定デモ |

### 1.2 Out of Scope (対象外)

| カテゴリ | 理由 |
|----------|------|
| モデルの学習/ファインチューニング | 推論のみ。学習は Python 側 |
| マイク入力の直接処理 | audio-capture / speech-capture の責務 |
| VAD 処理 | ten-vad の責務。ETD は VAD の後段 |
| Whisper STT | speech-capture の責務。ETD は発話完了判定のみ |
| GPU 推論の最適化 | Phase 1 は CPU 推論。GPU は将来拡張 |

---

## 2. 成果物

| # | 成果物 | 形式 | 説明 |
|---|--------|------|------|
| D1 | `crates/etd/` | Rust crate | ライブラリ本体 |
| D2 | RFC | `crates/etd/docs/rfc.md` | 設計根拠と代替案 |
| D3 | 本 SoW | `crates/etd/docs/sow.md` | 本ドキュメント |
| D4 | ユニットテスト | `src/*.rs` 内 `#[cfg(test)]` | mel, audio, inference の各テスト |
| D5 | Example | `examples/etd_demo.rs` | WAV → ETD 判定デモ |
| D6 | ONNX モデル | `assets/models/smart_turn_v3.onnx` | HuggingFace からダウンロード |

---

## 3. E-R 図 (型関係図)

```
┌─────────────────────────────────────────────────────┐
│                      etd crate                       │
│                                                      │
│  ┌──────────┐    creates    ┌───────────────────┐   │
│  │ EtdConfig │─────────────▶│ EndOfTurnDetector │   │
│  └──────────┘               └───────┬───────────┘   │
│  │ model_path: PathBuf              │               │
│  │ threshold: f32                   │ holds         │
│  │ max_audio_seconds: f32           │               │
│  │ sample_rate: u32                 ▼               │
│  │                          ┌──────────────┐        │
│  │                          │ ort::Session  │        │
│  │                          └──────────────┘        │
│  │                          ┌─────────┐             │
│  │                          │MelConfig│             │
│  │                          └─────────┘             │
│  │                                                  │
│  │  predict_i16(&[i16])                             │
│  │  predict(&[f32])                                 │
│  │         │                                        │
│  │         ▼                                        │
│  │  ┌───────────┐                                   │
│  │  │ EtdResult  │                                  │
│  │  ├───────────┤                                   │
│  │  │prediction │ bool  (true = 発話完了)            │
│  │  │probability│ f32   [0.0, 1.0]                  │
│  │  └───────────┘                                   │
│  │                                                  │
│  │  ┌──────────┐                                    │
│  │  │ EtdError  │                                   │
│  │  ├──────────┤                                    │
│  │  │ModelLoad │ モデル読み込み失敗                   │
│  │  │Inference │ ONNX 推論エラー                     │
│  │  │InvalidAudio│ 不正な音声入力                    │
│  │  └──────────┘                                    │
│                                                      │
└─────────────────────────────────────────────────────┘

外部依存:
  ┌─────────────────┐     ┌──────────────┐
  │ speech-capture   │────▶│ etd (optional)│
  │ SpeechEvent::    │     │              │
  │   VoiceEnd.audio │────▶│ predict_i16()│
  └─────────────────┘     └──────────────┘
```

---

## 4. シーケンス図

### 4.1 Batch モード (VoiceEnd 時に判定)

```
 User          AudioCapture    SpeechCapture     ETD            App
  │                │                │              │              │
  │  発話開始       │                │              │              │
  │ ─── audio ────▶│                │              │              │
  │                │── AudioFrame ─▶│              │              │
  │                │                │─ VAD ───────▶│              │
  │                │                │  is_voice=T  │              │
  │                │                │◀─────────────│              │
  │                │                │ [蓄積開始]    │              │
  │                │                │──── VoiceStart ───────────▶│
  │                │                │              │              │
  │  発話継続...    │── AudioFrame ─▶│              │              │
  │                │                │ [音声蓄積中]  │              │
  │                │                │              │              │
  │  沈黙          │── AudioFrame ─▶│              │              │
  │                │                │─ VAD ───────▶│              │
  │                │                │  is_voice=F  │              │
  │                │                │◀─────────────│              │
  │                │                │              │              │
  │  (silence_timeout経過)          │              │              │
  │                │                │── predict_i16(audio) ─────▶│
  │                │                │              │  [mel→ONNX]  │
  │                │                │◀── EtdResult ─────────────│
  │                │                │              │              │
  │                │                │── VoiceEnd { end_of_turn } ▶│
  │                │                │              │              │
```

### 4.2 Streaming モード (Early-cut)

```
 User          AudioCapture    SpeechCapture     ETD            App
  │                │                │              │              │
  │  発話開始       │── AudioFrame ─▶│              │              │
  │                │                │──── VoiceStart ───────────▶│
  │  発話継続...    │── AudioFrame ─▶│ [音声蓄積中]  │              │
  │                │                │              │              │
  │  沈黙          │── AudioFrame ─▶│              │              │
  │                │                │─ VAD: F ────▶│              │
  │                │                │ [TrailingSilence突入]       │
  │                │                │              │              │
  │                │                │── predict_i16(audio) ─────▶│
  │                │                │              │  probability │
  │                │                │◀── EtdResult { pred=true } │
  │                │                │              │              │
  │                │                │── VoiceEnd (即発火) ──────▶│
  │                │                │   (silence_timeout待たず)   │
  │                │                │              │              │
  ────────────────────────────────────────────────────────────────
  別パターン: ETD が incomplete を返した場合
  ────────────────────────────────────────────────────────────────
  │                │                │              │              │
  │  沈黙          │── AudioFrame ─▶│              │              │
  │                │                │─ VAD: F ────▶│              │
  │                │                │ [TrailingSilence突入]       │
  │                │                │── predict_i16(audio) ─────▶│
  │                │                │◀── EtdResult { pred=false }│
  │                │                │ [タイムアウト待機継続]       │
  │                │                │              │              │
  │  発話再開      │── AudioFrame ─▶│              │              │
  │                │                │─ VAD: T ────▶│              │
  │                │                │ [Speaking復帰]│              │
  │                │                │ [蓄積継続]    │              │
  │                │                │              │              │
```

---

## 5. ディレクトリ構成図

```
crates/etd/
├── Cargo.toml                  # 依存: ort, ndarray, rustfft, thiserror, log
├── docs/
│   ├── rfc.md                  # RFC (本 RFC)
│   └── sow.md                 # SoW (本ドキュメント)
├── src/
│   ├── lib.rs                  # EndOfTurnDetector, EtdResult, EtdConfig, EtdError
│   ├── audio.rs                # i16→f32 変換, truncate/pad (8秒先頭ゼロ埋め)
│   └── mel.rs                  # Whisper 互換 log-mel スペクトログラム (STFT + mel filterbank)
├── examples/
│   └── etd_demo.rs             # WAV → ETD 判定デモ
└── tests/
    └── mel_accuracy.rs         # Python (WhisperFeatureExtractor) との数値一致テスト
```

---

## 6. ライブラリ・モジュール構成

```
etd (crate)
│
├── lib.rs ─────────────────────────────────────────────
│   pub struct EtdConfig {
│       model_path: PathBuf,
│       threshold: f32,            // default: 0.5
│       max_audio_seconds: f32,    // default: 8.0
│       sample_rate: u32,          // default: 16000
│   }
│   pub struct EtdResult {
│       prediction: bool,
│       probability: f32,
│   }
│   pub enum EtdError {
│       ModelLoad(String),
│       Inference(String),
│       InvalidAudio(String),
│   }
│   pub struct EndOfTurnDetector {
│       session: ort::Session,
│       config: EtdConfig,
│       mel_config: mel::MelConfig,
│   }
│   impl EndOfTurnDetector {
│       pub fn new(config: EtdConfig) -> Result<Self, EtdError>;
│       pub fn predict_i16(&self, audio: &[i16]) -> Result<EtdResult, EtdError>;
│       pub fn predict(&self, audio: &[f32]) -> Result<EtdResult, EtdError>;
│   }
│
├── audio.rs ───────────────────────────────────────────
│   pub fn i16_to_f32(samples: &[i16]) -> Vec<f32>;
│   pub fn truncate_or_pad(
│       audio: &[f32],
│       sample_rate: u32,
│       max_seconds: f32,
│   ) -> Vec<f32>;
│
└── mel.rs ─────────────────────────────────────────────
    pub struct MelConfig {
        n_fft: usize,           // 400
        hop_length: usize,      // 160
        n_mels: usize,          // 80
        sample_rate: u32,       // 16000
        fmin: f32,              // 80.0
        fmax: f32,              // 7600.0
        chunk_length: f32,      // 8.0 秒
    }
    pub fn log_mel_spectrogram(
        audio: &[f32],
        config: &MelConfig,
    ) -> Vec<f32>;   // shape: (n_mels, n_frames) = (80, 800) row-major

    // 内部関数:
    fn hann_window(size: usize) -> Vec<f32>;
    fn stft(audio: &[f32], n_fft: usize, hop: usize) -> Vec<Vec<f32>>;
    fn mel_filterbank(n_mels: usize, n_fft: usize, sr: u32, fmin: f32, fmax: f32) -> Vec<Vec<f32>>;
    fn normalize(spec: &mut [f32]);
```

### 依存グラフ

```
                    ┌──────────────┐
                    │   etd crate  │
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
              ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │   ort    │ │ rustfft  │ │ ndarray  │
        │ (ONNX)   │ │ (STFT)   │ │ (tensor) │
        └──────────┘ └──────────┘ └──────────┘

        ┌──────────────────┐      ┌──────────┐
        │  speech-capture  │─ ─ ─▶│   etd    │  (optional dep)
        │                  │      │          │
        │  SpeechEvent::   │      │predict   │
        │  VoiceEnd.audio ─┼─────▶│_i16()    │
        └──────────────────┘      └──────────┘
```

---

## 7. ワークパッケージ (WP)

### WP-1: 音声前処理 + 特徴量抽出

**目的:** Whisper 互換の log-mel スペクトログラム生成

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| 1.1 | クレート scaffolding | Cargo.toml, lib.rs, workspace 追加 | `crates/etd/Cargo.toml` | `cargo check -p etd` が通る |
| 1.2 | 音声前処理 | i16→f32 変換, truncate/pad (8秒, 先頭ゼロ埋め) | `src/audio.rs` | 境界値テスト: 0秒/4秒/8秒/12秒 入力 |
| 1.3 | Hann 窓 + STFT | rustfft で Short-Time Fourier Transform | `src/mel.rs` | 既知正弦波で周波数ピーク位置を検証 |
| 1.4 | Mel filterbank | 三角フィルタバンク (80 bins, 80-7600Hz) | `src/mel.rs` | フィルタ形状の合計 ≈ 1.0、周波数範囲の正確性 |
| 1.5 | Log-mel + 正規化 | log10, clamp, zero-mean/unit-variance 正規化 | `src/mel.rs` | Python `WhisperFeatureExtractor` との出力差 < 1e-4 |

**WP-1 完了基準:**
- `cargo check -p etd` が通る
- `cargo test -p etd` で全 audio/mel テストが通る
- Python 版との数値一致テスト (mel_accuracy.rs) が通る

---

### WP-2: ONNX 推論 + 公開 API

**目的:** smart-turn v3 モデルの推論と公開 API

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| 2.1 | モデル取得 | HuggingFace から smart-turn-v3 ONNX をダウンロード | `assets/models/smart_turn_v3.onnx` | ファイルが存在し、ort でロードできる |
| 2.2 | ONNX 推論 | ort::Session で (1,80,800) → (1,1) 推論 | `src/lib.rs` | 無音入力 → probability < 0.3 |
| 2.3 | 公開 API | EndOfTurnDetector, EtdResult, EtdConfig | `src/lib.rs` | `predict_i16(&[i16])` が動作する |
| 2.4 | Example | WAV → ETD 判定デモ | `examples/etd_demo.rs` | `cargo run --example etd_demo` が動作する |

**WP-2 完了基準:**
- `cargo test -p etd` で推論テストが通る
- example が WAV ファイルから判定結果を出力する
- `cargo clippy -p etd -- -D warnings` が通る

---

### WP-3: speech-capture 統合

**目的:** ETD を speech-capture の optional dependency として統合

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| 3.1 | feature flag | `etd` feature を speech-capture に追加 | `speech-capture/Cargo.toml` | `cargo check -p speech-capture --features etd` が通る |
| 3.2 | SpeechEvent 拡張 | VoiceEnd に `end_of_turn`, `turn_probability` 追加 | `speech-capture/src/lib.rs` | 既存テストが壊れないこと (フィールドは Option) |
| 3.3 | Batch 統合 | VoiceEnd 時に ETD 判定を実行 | `speech-capture/src/lib.rs` | VoiceEnd イベントに ETD 結果が付与される |
| 3.4 | Streaming 統合 | TrailingSilence 突入時に ETD 判定 → complete なら即 VoiceEnd | `speech-capture/src/segmenter.rs` | complete 判定時に silence_timeout を待たず VoiceEnd が発火する |
| 3.5 | Example | ETD 付き speech-capture デモ | `speech-capture/examples/etd_speech.rs` | マイク入力で ETD 判定結果が表示される |

**WP-3 完了基準:**
- `cargo test -p speech-capture` が通る (ETD 無効時の後方互換)
- `cargo test -p speech-capture --features etd` が通る
- Streaming モードで early-cut が動作する

---

## 8. スケジュール

```
WP-1 (mel + audio)  ████████░░░░░░░░  Phase 1
WP-2 (ONNX + API)   ░░░░████████░░░░  Phase 2
WP-3 (統合)          ░░░░░░░░████████  Phase 3
```

WP-1 → WP-2 → WP-3 の順に直列実行。WP-1 の mel スペクトログラム実装が最もリスクが高い（Python との数値一致が必要）。

---

## 9. リスクと軽減策

| リスク | 影響 | 軽減策 |
|--------|------|--------|
| mel スペクトログラムの数値精度 | 推論結果の不一致 | Python 版と同一テストケースで 1e-4 以内を検証 |
| ONNX モデルの入力形式変更 | 推論エラー | smart-turn v3 の固定バージョンを使用 |
| ort セッション並行使用 | ランタイムエラー | EndOfTurnDetector は `&self` で推論可 (ort::Session は内部 Sync) |
| rustfft の精度 | mel 計算誤差 | f64 で計算し最後に f32 に変換する戦略 |
