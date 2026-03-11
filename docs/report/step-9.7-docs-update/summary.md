# Step 9.7: ドキュメント更新

## 作業日時
2026-03-11 14:32 JST

## 対象ファイル
- `CLAUDE.md`
- `features.md`
- `README.md`

## 実行した操作

### CLAUDE.md
- ORT ビルドの musl 注記を更新:
  - 変更前: `ORT ビルド (Linux musl): execinfo.h スタブ、Eigen 事前クローン、FLATBUFFERS_LOCALE_INDEPENDENT=0 が必要`
  - 変更後: `ORT ビルド (Linux): cargo-zigbuild で glibc 2.17+ ターゲットにクロスビルド。musl ワークアラウンドは不要`

### features.md
- ライブラリバージョン一覧に `nokhwa 0.10.7` が残っていることを確認（変更なし）

### README.md
- アーキテクチャ図の Camera 部分が nokhwa を参照していることを確認
- Linux ダウンロードセクションのターゲットを `x86_64-unknown-linux-musl` → `x86_64-unknown-linux-gnu` に3箇所変更

## 実行コマンド
```bash
cargo check --workspace  # コンパイル成功確認（ドキュメントのみの変更だが念のため）
```

## 結果
- 全ドキュメントが glibc+zigbuild 移行を反映
