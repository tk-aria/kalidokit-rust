# ETD .gitignore 作成

## 実行日時
2026-03-21 11:25 JST

## 実行コマンド
```bash
gibo dump Rust macOS Windows Linux VisualStudioCode > crates/etd/.gitignore
# 追加: *.onnx, tests/fixtures/*.npy, tarpaulin-report.html
```

## 結果
- gibo で Rust/macOS/Windows/Linux/VSCode テンプレートを生成
- ETD 固有の除外パターンを追加 (ONNX モデル, Python テストフィクスチャ, カバレッジ)
