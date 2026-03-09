# ML Inference Thread - 作業報告

## タスク
features.md line 1222: `[ ] ML推論を別スレッドに移動`

## 実施内容

### 1. TrackerThread 新規作成
- `crates/app/src/tracker_thread.rs` 新規作成
- `TrackerThread` 構造体: frame_sender + result_receiver
- `TrackerThread::new(tracker)`: ワーカースレッド spawn、HolisticTracker を所有権移動
- `send_frame()`: try_send で非ブロッキング送信（処理中ならフレームドロップ）
- `try_recv_result()`: try_recv で非ブロッキング受信
- チャネルバッファサイズ: sync_channel(1)

### 2. AppState 更新
- `state.rs`: `tracker` → `tracker_thread: TrackerThread` に置き換え
- `last_tracking_result: Option<HolisticResult>` キャッシュ追加

### 3. init.rs 更新
- TrackerThread::new(tracker) でスレッド起動

### 4. update.rs 更新
- フレームをトラッカースレッドに非ブロッキング送信
- 結果を非ブロッキング受信、あれば last_tracking_result を更新
- キャッシュされた結果からソルバー入力を取得
- レンダリングがML推論をブロックしない

## 実行コマンド
- `cargo check --workspace` — コンパイル成功

## 結果
`[x]` に更新。ML推論の別スレッド化完了。
