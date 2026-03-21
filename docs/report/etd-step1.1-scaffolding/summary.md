# Step 1.1: ETD クレート scaffolding

## 実行日時
2026-03-21 11:27 JST

## 実行コマンド
```bash
# Cargo.toml 作成
# root Cargo.toml に "crates/etd" を追加
# src/lib.rs, src/error.rs, src/audio.rs (stub), src/mel.rs (stub) を作成
cargo check -p etd  # OK
```

## 作成ファイル
- `crates/etd/Cargo.toml` — ort, ndarray, thiserror, log, rustfft 依存
- `crates/etd/src/lib.rs` — モジュール宣言
- `crates/etd/src/error.rs` — EtdError 型
- `crates/etd/src/audio.rs` — stub
- `crates/etd/src/mel.rs` — stub

## 結果
- `cargo check -p etd` 成功
