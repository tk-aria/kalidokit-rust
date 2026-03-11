# Step 9.3: nokhwa 依存の復活

## 作業日時
2026-03-11 14:27 JST

## 対象ファイル
- `Cargo.toml` (ワークスペースルート)
- `crates/app/Cargo.toml`

## 実行した操作

1. ワークスペースルート `Cargo.toml` の `[workspace.dependencies]` に `nokhwa` が既に定義されていることを確認
   - `nokhwa = { version = "0.10", features = ["input-native"] }`
2. `crates/app/Cargo.toml` の `[dependencies]` に `nokhwa = { workspace = true }` を追加

## 実行コマンド
```bash
cargo check --workspace  # コンパイル成功確認
```

## 結果
- nokhwa 0.10.10 が依存に追加され、正常にコンパイルされた
- Rust ツールチェーンを 1.85.0 → 1.94.0 にアップデート（image, ort の MSRV 要件）
