# 🎤 ETD (End-of-Turn Detection / 発話終了検出)

[smart-turn v3](https://github.com/pipecat-ai/smart-turn) ONNX モデルを使用して、ユーザーの発話が完了したかどうかを検出します。

## 概要

ETD は VAD（Voice Activity Detection）を補完し、無音が「考え中の沈黙」か「発話完了」かを音響特徴量から判定します。これにより、音声ベースアプリケーションでの応答タイミングが改善されます。

### アーキテクチャ

```
16kHz mono PCM 音声 (最大8秒)
  → Log-Mel スペクトログラム (80 mel bins × 800 frames)
    → Whisper Tiny エンコーダ (~8M パラメータ)
      → Attention Pooling → 分類器
        → Sigmoid → 確率値 [0.0, 1.0]
          → 閾値 (0.5) → 完了 / 未完了
```

## クイックスタート

```rust
use etd::{EndOfTurnDetector, EtdConfig};

let mut detector = EndOfTurnDetector::new(EtdConfig::default())?;

// i16 PCM から判定 (speech-capture の audio をそのまま渡せる)
let result = detector.predict_i16(&audio_i16)?;

// f32 PCM [-1.0, 1.0] から判定
let result = detector.predict(&audio_f32)?;

println!("発話完了: {} (確率: {:.2}%)",
    result.prediction, result.probability * 100.0);
```

## モデルのセットアップ

smart-turn v3 ONNX モデル (~32MB) をダウンロード:

```bash
curl -L -o assets/models/smart_turn_v3.onnx \
  "https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/main/onnx/model.onnx"
```

## サンプル

### WAV ファイルデモ

```bash
cargo run -p etd --example etd_demo -- path/to/audio.wav
```

### speech-capture 統合

```bash
cargo run -p speech-capture --features end-of-turn --example etd_speech
```

## 設定

| パラメータ | デフォルト | 説明 |
|-----------|---------|------|
| `model_path` | `assets/models/smart_turn_v3.onnx` | ONNX モデルファイルパス |
| `threshold` | `0.5` | 「発話完了」と判定する確率閾値 |
| `max_audio_seconds` | `8.0` | 最大音声ウィンドウ長（発話末尾基準） |
| `sample_rate` | `16000` | 入力サンプルレート (Hz) |

## テスト

```bash
# ユニットテスト (モデル不要)
cargo test -p etd

# 統合テスト (ONNX モデルが必要)
cargo test -p etd -- --ignored

# 全テスト
cargo test -p etd -- --include-ignored
```

## 設計ドキュメント

- [RFC](docs/rfc.md) — 動機、アーキテクチャ、代替案
- [SoW](docs/sow.md) — 作業パッケージ、スケジュール、図表
- [features.md](docs/features.md) — 実装 TODO リスト

## ライセンス

MIT
