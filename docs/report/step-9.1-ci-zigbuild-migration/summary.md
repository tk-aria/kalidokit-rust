# Step 9.1: CI — Linux ビルドジョブを cargo-zigbuild に移行

## 作業日時
2026-03-11 14:22 JST

## 対象ファイル
- `.github/workflows/release.yml`

## 実行した操作

1. `build-linux` ジョブを全面書き換え:
   - ジョブ名: `Build (x86_64-unknown-linux-musl)` → `Build (x86_64-unknown-linux-gnu)`
   - コンテナ: Alpine 3.21 削除 → `ubuntu-latest` で直接実行
   - システム依存: `apk add` → `sudo apt-get install -y cmake pkg-config libx11-dev libxkbcommon-dev libwayland-dev`
   - Rust: 手動 rustup → `dtolnay/rust-toolchain@stable` (target: `x86_64-unknown-linux-gnu`)
   - ツール追加: `cargo install cargo-zigbuild` + `pip3 install ziglang`

2. 以下の musl ワークアラウンドを全て削除:
   - execinfo.h スタブ (backtrace no-op)
   - Eigen 事前クローン (GitLab hash mismatch 回避)
   - sed パッチ (execinfo.h include 削除)
   - ORT ビルドフラグ: `FLATBUFFERS_LOCALE_INDEPENDENT=0`, `ENABLE_BACKTRACE=OFF`
   - re2 スタンドアロンビルド (ORT が libre2.a を生成しない問題)

3. ORT ビルドステップを簡素化:
   - glibc 環境ではデフォルト設定で動作
   - キャッシュキー: `ort-musl-static-v1.20.1-alpine321-v11` → `ort-glibc-static-v1.20.1-v1`

4. ビルドコマンド変更:
   - `cargo build --release --target x86_64-unknown-linux-musl`
   - → `cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17`

5. パッケージング・アーティファクト名を `x86_64-unknown-linux-gnu` に統一

## 実行コマンド
```bash
# ファイル編集 (Edit tool で実施)
# features.md のチェックボックス更新 (Edit tool で実施)
```

## 結果
- macOS/Windows の `build` ジョブと `release` ジョブは変更なし
- CI 定義ファイルの行数が約 168 行 → 168 行 (musl ワークアラウンド約 100 行を削除、zigbuild セットアップを追加)
