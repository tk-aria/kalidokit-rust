# Phase 16: Whisper STT 統合 — 作業報告

## 実行日時
2026-03-19 15:18-15:35 JST

## 完了タスク

### Step 16.1: whisper-rs 依存追加
- `whisper-rs = { version = "0.16", optional = true }` + `stt` feature flag
- stt feature なし: 既存動作維持

### Step 16.2: SttConfig + SttMode + SpeechEvent 拡張
- stt_types.rs: SttMode (Disabled/Batch/Streaming), SttConfig
- SpeechEvent: TranscriptInterim 追加, VoiceEnd.transcript 追加
- SpeechConfig.stt: Option<SttConfig>

### Step 16.3: WhisperEngine ラッパー
- whisper_engine.rs (cfg(feature = "stt"))
- transcribe(&[i16]) → String (i16→f32変換内蔵)

### Step 16.4: VAD ワーカーに STT 統合
- Disabled: 既存動作
- Batch: VoiceEnd 時に transcribe → transcript 設定
- Streaming: interim_interval_ms ごとに TranscriptInterim 発火

### Step 16.5-16.6: Examples
- streaming_stt.rs: 中間結果 + 確定テキスト
- batch_stt.rs: 一括文字起こし

## ビルド環境 (macOS arm64)
whisper-rs は whisper.cpp をソースからビルド。以下の環境変数が必要:
```
LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib
CC=/usr/bin/clang CXX=/usr/bin/clang++
```

## 検証結果
- `cargo check -p speech-capture` — pass (feature なし)
- `cargo check -p speech-capture --features stt` — pass (37.9s)
- `cargo test -p speech-capture` — 3 tests pass
- `cargo clippy -p speech-capture -- -D warnings` — pass
