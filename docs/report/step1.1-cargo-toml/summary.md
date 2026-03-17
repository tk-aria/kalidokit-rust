# Step 1.1: Cargo.toml + ワークスペース追加 — 作業報告

## 実行日時
2026-03-17 12:10-12:13 JST

## 実行コマンド

1. `cat .gitignore` — 既存 .gitignore 確認 (9行、非空のためスキップ)
2. `mkdir -p crates/video-decoder/src` — ディレクトリ作成
3. ルート `Cargo.toml` の `members` に `"crates/video-decoder"` を追加
4. `crates/video-decoder/Cargo.toml` を新規作成
5. `crates/video-decoder/src/lib.rs` を空クレートとして作成
6. `cargo check -p video-decoder` — 初回: `expose-ids` feature エラー
7. `Cargo.toml` の wgpu 依存を `{ workspace = true }` に修正
8. `cargo check -p video-decoder` — 成功 (8.59s)

## トラブルシューティング
- wgpu 24.0 には `expose-ids` feature が存在しない → `{ workspace = true }` でワークスペースの wgpu 設定を継承するよう修正
- 設計書の Cargo.toml 例は `expose-ids` を前提としていたが、実際の wgpu 24.0 では不要

## 成果物
- `Cargo.toml` (ルート) — members に video-decoder 追加
- `crates/video-decoder/Cargo.toml` — 新規作成
- `crates/video-decoder/src/lib.rs` — 空クレート
