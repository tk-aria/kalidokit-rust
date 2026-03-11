# Step 9.2: セットアップスクリプトの更新

## 作業日時
2026-03-11 14:22 JST

## 対象ファイル
- `scripts/setup.sh`

## 実行した操作

1. `_get_target()` 関数の Linux ターゲットを変更:
   - 変更前: `linux) echo "${_arch}-unknown-linux-musl" ;;`
   - 変更後: `linux) echo "${_arch}-unknown-linux-gnu" ;;`

## 実行コマンド
```bash
# ファイル編集 (Edit tool で実施)
# features.md のチェックボックス更新 (Edit tool で実施)
```

## 結果
- Linux 環境でのインストールスクリプトが glibc ターゲットのバイナリをダウンロードするようになった
