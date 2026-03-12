# Step 10.3: Rust → ObjC フレーム送信パイプライン — 作業報告

**日時**: 2026-03-12 15:45 JST
**ステータス**: 完了

## 実行した操作

### 1. Cargo.toml 更新
- `objc2-core-foundation` 0.3 追加 (features: CFString, CFCGTypes)
- `objc2-core-media-io` features: CMIOHardwareSystem, CMIOHardwareObject, CMIOHardwareDevice, CMIOHardwareStream
- `objc2-core-media` features: CMSampleBuffer, CMFormatDescription, CMTime, CMSimpleQueue, objc2-core-video
- `objc2-core-video` features: CVBuffer, CVImageBuffer, CVPixelBuffer, CVReturn

### 2. macos.rs 完全実装
- `MacOsVirtualCamera` struct: device_id, sink_stream_id, buffer_queue, frame_count
- `discover_device()`: CMIOObjectGetPropertyData で全デバイス列挙 → "KalidoKit" を名前で検索
- `acquire_buffer_queue()`: CMIOStreamCopyBufferQueue でシンクストリームのキュー取得
- `rgba_to_bgra()`: チャンネル 0,2 をスワップ
- `create_pixel_buffer()`: CVPixelBufferCreateWithBytes (BGRA, width*4 stride)
- `create_sample_buffer()`: CMVideoFormatDescriptionCreateForImageBuffer → CMSampleTimingInfo → create_ready_with_image_buffer
- `VirtualCamera::start()`: discover → acquire queue → CMIODeviceStartStream
- `VirtualCamera::send_frame()`: RGBA→BGRA → CVPixelBuffer → CMSampleBuffer → CMSimpleQueue::enqueue
- `VirtualCamera::stop()`: CMIODeviceStopStream

### 3. ヘルパー関数
- `get_cmio_devices()`: kCMIOHardwarePropertyDevices で全デバイスID取得
- `get_device_streams()`: kCMIODevicePropertyStreams でストリームID取得
- `get_object_name()`: kCMIOObjectPropertyName → CFString → Rust String 変換
- `get_stream_direction()`: kCMIOStreamPropertyDirection (0=source, 1=sink)

### 4. コンパイル検証
```bash
cargo check -p virtual-camera
```
結果: 成功 (0.90s)

## トラブルシューティング
- `objc2_core_foundation` が unresolved → `objc2-core-foundation` crate を依存に追加
- `CFStringEncoding(...)` → type alias `u32` なので直接 `u32` 値を使用
- `CFString::get_bytes` → 正しいメソッド名は `CFString::bytes`
