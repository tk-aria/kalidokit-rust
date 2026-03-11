# CLAUDE.md - Project Rules for kalidokit-rust

## Project Overview

Rust implementation of KalidoKit (VRM motion capture). Uses wgpu + winit for rendering, ort for ML inference, nokhwa for webcam capture.

Workspace crates: `app`, `renderer`, `vrm`, `solver`, `tracker`

## Implementation Rules

### features.md のチェックボックス運用ルール

1. **チェックをつける条件**: 以下の **全て** を満たすこと
   - コードが存在し、`cargo check` が通る
   - **他のクレート/モジュールから実際に呼び出されている**（デッドコードでない）
   - **実行して動作が確認できている**、または実行不可能な環境の場合はその旨を明記して `[ ]` のままにする

2. **「コードが存在する」だけではチェックしない**: 構造体・関数が定義されていても、統合（パイプラインへの組み込み、update ループでの呼び出し等）がされていなければ未完了

3. **ダミー実装・TODO コメント付きコードは未完了**: `// TODO` コメントがある時点でそのチェックボックスは `[ ]` でなければならない

4. **ヘッドレス環境での制約を明記する**: GPU/ウィンドウ/カメラ等の実行検証ができない場合、チェックボックスを `[ ]` のままにし `— ヘッドレス環境のため未検証` と注記する

### コード品質ルール

1. **`cargo check` が通る ≠ 動作する**: コンパイル成功だけで完了としない。特に以下は実行確認が必須:
   - winit の `ApplicationHandler` イベントハンドラ（`about_to_wait`, `resumed` 等）
   - wgpu のレンダーパス・パイプライン
   - カメラキャプチャの初期化と毎フレーム取得

2. **統合テストの確認**: 新しいモジュールを追加したら、以下を確認する:
   - そのモジュールが実際に呼び出し元から使われているか
   - VrmModel 等のメイン構造体にフィールドが追加されているか
   - update ループやレンダーパイプラインに組み込まれているか

3. **デッドコードを残さない**: 実装したが統合していないコードがある場合、features.md に `[ ]` で統合タスクを追記する

## Build & Test Commands

```bash
cargo check --workspace          # 型チェック
cargo test -p renderer -p vrm -p solver  # 単体テスト (tracker は ort-sys リンク制約で除外)
cargo clippy --workspace -- -D warnings  # lint
cargo fmt --check                # フォーマット
cargo build --release            # リリースビルド (要 ONNX Runtime)
```

## Key Architecture Notes

- winit 0.30: `ApplicationHandler` trait 実装。`about_to_wait()` で毎アイドル `request_redraw()` を呼ぶことでレンダーループを駆動
- wgpu Metal backend: macOS 13.x 互換のため `.cargo/config.toml` で CoreML シンボルを `-U` リンカフラグで許容
- ORT ビルド (Linux): cargo-zigbuild で glibc 2.17+ ターゲットにクロスビルド。musl ワークアラウンドは不要
- features.md が実装計画の信頼できる唯一のソース (Single Source of Truth)
