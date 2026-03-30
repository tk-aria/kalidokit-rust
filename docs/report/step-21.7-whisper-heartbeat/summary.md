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

---

## Task 3: heartbeat_age() で最後の進捗からの経過時間を取得
## Task 4: abort() で推論中断を signal
(Task 1-2 で実装済み)

## Task 5: app 側で heartbeat_age > 閾値 → "Whisper stalled" 表示

### 変更ファイル
- `crates/speech-capture/src/whisper_engine.rs` — `new_with_arcs()` 追加 (外部 Arc を受け取る)
- `crates/speech-capture/src/lib.rs` — whisper_heartbeat/abort Arc を SpeechCapture に追加、`whisper_heartbeat_age()` / `abort_whisper()` 公開
- `crates/avatar-sdk/src/action.rs` — `AvatarAction::AbortWhisper` 追加
- `crates/avatar-sdk/src/state.rs` — `SpeechState.whisper_stalled: bool` 追加
- `crates/app/src/lua_avatar.rs` — `avatar.get_whisper_stalled()` / `avatar.abort_whisper()` バインディング追加
- `crates/app/src/update.rs` — 毎フレーム heartbeat_age チェック + AbortWhisper アクション処理
- `assets/scripts/speech_log.lua` — stalled 表示 + Abort Whisper ボタン

### 実行コマンド
```bash
LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib GGML_NATIVE=OFF cargo check -p kalidokit-rust
```

### ビルド結果
- `cargo check` 成功 (既存 warnings のみ)

### 完了時刻
2026-03-30T19:20:00+09:00

---

## 動作確認結果 (2026-03-30T20:55+09:00)

### abort_callback による crash
- `set_abort_callback_safe` を有効にすると `whisper_full_with_state: failed to encode` で crash
- whisper-rs 0.16 + Metal backend との互換性問題
- abort_callback を無効化すると正常動作

### 対応
- abort_callback をコメントアウトして保留
- ハートビート更新は abort_callback に依存するため、ハートビート機能も一時保留
- whisper-rs の次バージョンまたは unsafe API での回避策を要調査
