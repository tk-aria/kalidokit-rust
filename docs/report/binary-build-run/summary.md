# バイナリビルド & 実行確認

## 実行日時
2026-03-10 13:31 JST

## ビルド
```bash
cargo build --release
# Finished `release` profile [optimized] in 9.32s
```

## 実行
```bash
RUST_LOG=info ./target/release/kalidokit-rust
```

### 出力ログ
```
[2026-03-10T04:31:16Z INFO  kalidokit_rust::tracker_thread] Tracker worker thread started
[2026-03-10T04:31:17Z WARN  kalidokit_rust::init] Failed to initialize webcam: Failed to create camera. Falling back to dummy frames.
```

### 結果
- ビルド: OK (release profile)
- 起動: OK (パニックなし)
- Tracker スレッド: 正常開始
- Webcam: 未接続のため dummy frames フォールバック (想定通り)
- 終了: SIGTERM で正常停止 (exit code 143)

## 確認済みアセット
- `assets/models/default_avatar.vrm` — VRM モデル
- `assets/models/face_landmark.onnx` — 顔メッシュモデル
- `assets/models/hand_landmark.onnx` — 手ランドマークモデル
- `assets/models/pose_landmark.onnx` — ポーズランドマークモデル
