# Phase 8: Android バックエンド (MediaCodec) — 作業報告

## 実行日時
2026-03-17 18:44-18:49 JST

## 完了タスク
- Cargo.toml: cfg(android) に ndk 0.9 + ash 0.38 追加
- media_codec.rs: McVideoSession stub (cfg(android) gated)
- backend/mod.rs: Android detect_backends + create_with_backend
- テスト: 1 new test (enum validity), 53 tests pass

## 実行コマンド
```
cargo check -p video-decoder
cargo test -p video-decoder     # 53 tests pass
cargo clippy -p video-decoder -- -D warnings
cargo fmt -p video-decoder
```

## 未完了 (Android 環境で実施)
- MediaCodec API 実装 (AMediaExtractor, AMediaCodec, AHardwareBuffer)
- Vulkan import + NV12→RGBA 変換
- クロスビルド確認 (aarch64-linux-android)
- Android 実機動作確認
