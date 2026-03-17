# Phase 7: Linux バックエンド (Vulkan Video + GStreamer + V4L2) — 作業報告

## 実行日時
2026-03-17 18:39-18:44 JST

## 完了タスク

### Step 7.1: Cargo.toml に Linux 依存追加
- ash 0.38 (常時), gstreamer 0.23 系 (optional), nix 0.29 (optional)
- features: gstreamer, v4l2

### Step 7.2: Vulkan Video stub (vulkan_video.rs)
- VkVideoSession struct stub, is_supported() stub
- cfg(target_os = "linux") gated

### Step 7.3: GStreamer VA-API stub (gst_vaapi.rs)
- GstVideoSession struct stub
- cfg(all(target_os = "linux", feature = "gstreamer")) gated

### Step 7.4: V4L2 Stateless stub (v4l2.rs)
- V4l2VideoSession struct stub, is_supported() stub
- cfg(all(target_os = "linux", feature = "v4l2")) gated

### Step 7.5: backend/mod.rs 接続
- Vulkan handle → Linux backends dispatch (cfg-gated)

### Step 7.6-7.7: テスト・検証
- 3 new tests (Vulkan handle non-Linux, enum validity)
- 52 tests + 1 doctest pass on macOS

## 実行コマンド
```
cargo check -p video-decoder
cargo test -p video-decoder     # 52 tests pass
cargo clippy -p video-decoder -- -D warnings
cargo fmt -p video-decoder
```

## 未完了 (Linux 環境で実施)
- Vulkan Video API 実装 (vkCreateVideoSessionKHR, vkCmdDecodeVideoKHR, DPB)
- GStreamer パイプライン実装 (decodebin3 → DMA-BUF → Vulkan import)
- V4L2 ioctl 実装 (VIDIOC_QBUF/DQBUF/EXPBUF)
- Linux 正常系テスト + cfg feature ビルドテスト
