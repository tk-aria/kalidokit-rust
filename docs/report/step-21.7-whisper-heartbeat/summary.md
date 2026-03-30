# Step 21.7: Whisper Progress-Based Heartbeat

## Task 1: whisper_engine に heartbeat + abort_flag 追加
## Task 2: abort_callback 内で heartbeat 更新 + abort_flag チェック

### 変更ファイル
- `crates/speech-capture/src/whisper_engine.rs`

### 実行コマンド
```bash
# ディスク容量確保
cargo clean
rm -f models/ggml-tiny.bin
rm -rf /tmp/etd-venv
rm -rf /tmp/kalidokit.log

# ビルド確認
LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib GGML_NATIVE=OFF cargo check -p speech-capture --features stt-metal,end-of-turn
```

### 実装内容
1. `WhisperEngine` に `heartbeat_ms: Arc<AtomicU64>` と `abort_flag: Arc<AtomicBool>` フィールドを追加
2. `new()` で初期化 (heartbeat=0, abort=false)
3. `transcribe_with_prob()` で:
   - 推論開始前に abort_flag をリセット、heartbeat を更新
   - `params.set_abort_callback_safe()` で abort_callback を登録
   - abort_callback はトークン生成毎に呼ばれ、heartbeat を epoch ms で更新し、abort_flag を返す
   - abort された場合は空の TranscribeResult を返す
4. `heartbeat_age()`: 最後の進捗からの経過時間を返す
5. `abort()`: abort_flag を true にセット
6. `is_aborting()`: abort_flag を読み取り

### ビルド結果
- `cargo check` 成功 (warnings: heartbeat_age/abort/is_aborting が未使用 — 後続タスクで使用予定)

### 完了時刻
2026-03-30T19:11:43+09:00
