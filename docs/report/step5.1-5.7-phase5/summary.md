# Phase 5: macOS VideoToolbox バックエンド — 作業報告

## 実行日時
2026-03-17 12:44-18:34 JST

## 完了タスク

### Step 5.1: Cargo.toml に macOS 依存追加
- `cfg(target_os = "macos")` ブロックに objc2 系 6 クレートを追加
- objc2 0.6, objc2-foundation 0.3, objc2-av-foundation 0.3 (objc2-core-media feature), objc2-core-media 0.3, objc2-core-video 0.3, objc2-core-foundation 0.3

### Step 5.2: AppleVideoSession (backend/apple.rs, ~250行)
- AVURLAsset → AVAssetReader → AVAssetReaderTrackOutput (BGRA 出力設定)
- VideoInfo: duration (CMTime), fps (nominalFrameRate), codec (H264)
- decode_frame: copyNextSampleBuffer → CVPixelBuffer → LockBaseAddress → BGRA→RGBA swizzle
- SendWrapper で ObjC !Send 型を Send に (VideoSession: Send 要件)

### Step 5.3: seek 実装
- AVAssetReader を timeRange 指定で再作成 (AVAssetReader は seek 非対応)

### Step 5.4: リソース解放
- objc2 の Retained<T> (ARC) で自動管理

### Step 5.5: backend/mod.rs 接続
- NativeHandle::Metal → [VideoToolbox]
- Backend::VideoToolbox → AppleVideoSession::new()

### Step 5.6: テスト
- 3 新規テスト: nonexistent file, invalid file, backend variant
- 全 45 tests + 1 doctest pass

### Step 5.7: Phase 5 検証
- clippy 0 warnings, fmt OK
- Metal ゼロコピー (CVMetalTextureCache) は将来フェーズ
- 正常系テスト (フィクスチャ依存) は保留

## 実行コマンド
```
# Cargo.toml に macOS 依存追加
# subagent で apple.rs 実装 + backend/mod.rs 更新
cargo check -p video-decoder
cargo test -p video-decoder     # 45 tests pass
cargo clippy -p video-decoder -- -D warnings
cargo fmt -p video-decoder
```

## 設計判断
- 初期実装は CPU バッファ経由 (BGRA→RGBA swizzle)。CVMetalTextureCache ゼロコピーは将来追加
- ObjC types は !Send のため send_wrapper crate ではなく自前 SendWrapper で対応
