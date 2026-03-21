# ETD (End-of-Turn Detection) クレート — RFC

> **クレート名**: `etd`
> **バージョン**: 0.1.0
> **作成日**: 2026-03-21
> **ステータス**: Draft
> **リファレンス**: [pipecat-ai/smart-turn](https://github.com/pipecat-ai/smart-turn)

---

## 1. 動機 (Motivation)

現在の音声パイプライン (`speech-capture`) は VAD (Voice Activity Detection) による無音検出で発話区間を切り出している。しかし VAD は「声が出ているか否か」しか判定できず、以下の問題がある:

1. **考え中の沈黙を発話終了と誤判定** — ユーザーが言葉を選んでいる間の短い無音で VoiceEnd が発火し、不完全な発話が STT に渡される
2. **応答タイミングが遅い** — `silence_timeout_ms` (例: 500-1000ms) を待つ必要があり、実際には発話が完了しているのに応答開始が遅れる
3. **言語的コンテキストを無視** — 文法、韻律、トーンなど人間が自然に行うターン判定の手がかりを活用していない

ETD は音声の音響特徴量（Whisper Tiny エンコーダ）と学習された分類器により、「発話が完了したか否か」を判定する。これにより VAD の弱点を補完し、より自然な応答タイミングを実現する。

---

## 2. ガイドレベル説明 (Guide-level Explanation)

### 2.1 位置づけ

```
audio-capture → speech-capture (VAD + Whisper STT)
                     │
                     │ VoiceEnd / TrailingSilence
                     ▼
                etd::EndOfTurnDetector
                     │
                     ▼
                EtdResult { prediction, probability }
```

ETD は VAD の**後段**に位置する。VAD が無音を検出したタイミングで、蓄積された音声バッファに対して ETD 推論を実行し、発話完了/未完了を判定する。

### 2.2 基本的な使い方

```rust
use etd::{EndOfTurnDetector, EtdConfig};

let detector = EndOfTurnDetector::new(EtdConfig {
    model_path: "assets/models/smart_turn_v3.onnx".into(),
    ..Default::default()
})?;

// speech-capture の VoiceEnd で得た audio をそのまま渡す
let result = detector.predict_i16(&audio_i16)?;

if result.prediction {
    // 発話完了 → 応答開始
} else {
    // 未完了 → 待機継続
}
```

### 2.3 動作モード

| モード | トリガー | 説明 |
|--------|----------|------|
| **Batch** | `VoiceEnd` 時 | silence_timeout 後に1回だけ判定。結果を `VoiceEnd` に付与 |
| **Streaming (Early-cut)** | `TrailingSilence` 突入時 | 無音検出の瞬間に即判定。complete なら timeout を待たず即 VoiceEnd |

---

## 3. リファレンスレベル説明 (Reference-level Explanation)

### 3.1 モデルアーキテクチャ (smart-turn v3)

```
入力: 16kHz mono PCM (最大8秒)
  ↓
Log-Mel Spectrogram (80 mel bins × 800 frames)
  ↓
Whisper Tiny Encoder (~8M params)
  ↓ hidden_states
Attention Pooling (Linear→Tanh→Linear)
  ↓ pooled vector
Classifier (Linear→LayerNorm→GELU→Dropout→Linear→GELU→Linear)
  ↓
Sigmoid → probability ∈ [0.0, 1.0]
  ↓
Threshold (0.5) → prediction: complete / incomplete
```

- **入力**: `input_features` shape `(batch_size, 80, 800)` f32
- **出力**: `(batch_size, 1)` f32 (sigmoid 確率)
- **ONNX モデルサイズ**: FP32 ~32MB, INT8 ~8MB
- **推論速度**: CPU ~10ms, GPU <100ms

### 3.2 特徴量抽出 (Whisper 互換)

Python の `transformers.WhisperFeatureExtractor` と同一仕様を Rust で実装する:

| パラメータ | 値 | 説明 |
|---|---|---|
| sample_rate | 16000 | 入力サンプルレート |
| n_fft | 400 | FFT ウィンドウ長 (25ms) |
| hop_length | 160 | フレームシフト (10ms) |
| n_mels | 80 | mel フィルタバンク数 |
| chunk_length | 8秒 | 最大入力長 |
| n_frames | 800 | 出力フレーム数 (8s × 100fps) |
| fmin | 80.0 Hz | mel 最低周波数 |
| fmax | 7600.0 Hz | mel 最高周波数 |
| window | Hann | 窓関数 |
| normalize | true | zero mean, unit variance |
| padding | 先頭ゼロパディング | 8秒未満の場合 |

処理フロー:
1. `i16 → f32` 変換 ([-1.0, 1.0] 正規化)
2. 8秒にトランケート/パディング（先頭パディング = smart-turn 仕様）
3. STFT (Hann window, n_fft=400, hop=160)
4. パワースペクトル → mel filterbank 適用
5. log10 変換 → clamp (max - 8.0)
6. 正規化 (zero mean, unit variance)

### 3.3 speech-capture 統合

`SpeechConfig` に `etd` フィールドを追加:

```rust
pub struct SpeechConfig {
    // 既存フィールド ...
    pub etd: Option<etd::EtdConfig>,  // None = ETD 無効
}
```

`VoiceEnd` に判定結果を追加:

```rust
pub enum SpeechEvent {
    VoiceEnd {
        timestamp: Duration,
        audio: Vec<i16>,
        duration: Duration,
        transcript: Option<String>,
        end_of_turn: Option<bool>,       // ETD 有効時のみ Some
        turn_probability: Option<f32>,   // ETD 有効時のみ Some
    },
    // ...
}
```

Streaming モードでは `VadSegmenter` の状態遷移を拡張:

```
Speaking → TrailingSilence 突入
  ↓
ETD 判定 (蓄積音声)
  ├── prediction=true  → 即座に VoiceEnd 発火 (silence_timeout を待たない)
  └── prediction=false → 通常の silence_timeout 待機
                          └── 発話再開 → Speaking に戻る
```

---

## 4. 欠点 (Drawbacks)

1. **ONNX モデルサイズ**: FP32 で ~32MB。アセットサイズが増加する
2. **推論レイテンシ**: CPU で ~10ms は十分だが、mel スペクトログラム計算のオーバーヘッドが加わる
3. **多言語精度のばらつき**: smart-turn v3 は 23 言語対応だが、言語により精度差がある
4. **ort 依存の増加**: tracker クレートと同じ ort を使うが、同時使用時のセッション管理に注意が必要

---

## 5. 代替案 (Alternatives)

| 代替案 | 利点 | 欠点 |
|--------|------|------|
| silence_timeout のチューニングのみ | 追加依存なし | 根本的に VAD の限界は超えられない |
| Silero VAD の確率値でヒューリスティック | 軽量 | 言語的コンテキストを考慮できない |
| LLM ベースの EOT 判定 | 高精度 | レイテンシが大きすぎる (100ms+) |
| Whisper のタイムスタンプ解析 | STT と統合 | 発話完了の直接判定ではない |

smart-turn の Whisper Tiny ベースアプローチは、精度とレイテンシのバランスが最も優れている。

---

## 6. 未解決の問題 (Unresolved Questions)

1. **mel filterbank 実装**: 既存 Rust クレート (`mel-spec`, `whisper-rs` 内部) を流用するか、自作するか
2. **モデル配布**: HuggingFace からの自動ダウンロード vs アセットに同梱
3. **INT8 量子化モデル対応**: CPU 向け INT8 モデル (8MB) への対応優先度
4. **speech-capture への統合粒度**: etd クレートを speech-capture の optional dependency にするか、app 側で組み合わせるか
