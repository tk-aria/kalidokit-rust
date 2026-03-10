# Step 8.8: Face / Pose 並列推論

## 完了日時
2026-03-10 13:06 JST

## 変更ファイル
- `crates/tracker/Cargo.toml` — `rayon = "1.10"` 追加
- `crates/tracker/src/face_mesh.rs` — `session: Session` → `session: Mutex<Session>`, `detect(&mut self)` → `detect(&self)`
- `crates/tracker/src/pose.rs` — 同上
- `crates/tracker/src/hand.rs` — 同上
- `crates/tracker/src/holistic.rs` — `detect(&mut self)` → `detect(&self)`, `rayon::join` で Face/Pose 並列化
- `crates/app/src/tracker_thread.rs` — `mut tracker` → `tracker` (detect が &self に変更)

## 実行コマンド
```bash
cargo check --workspace        # OK
cargo test -p solver -p vrm -p renderer  # 63 tests passed
```

## 実装内容
1. `rayon` 依存追加
2. 3つのディテクター (FaceMesh, Pose, Hand) の Session を `Mutex<Session>` でラップ
   - `ort::Session::run` が `&mut self` を要求するため、interior mutability が必要
3. `detect()` を `&self` に変更 (rayon::join の Send 要件)
4. `rayon::join` で Face と Pose を並列実行
5. Hand は Pose 結果の ROI に依存するため、Pose 完了後に順次実行
