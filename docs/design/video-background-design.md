# 動画背景レンダリング — プラットフォーム別設計

## 1. 全体アーキテクチャ

```
┌─────────────────────────────────────────────────────┐
│                  video-decoder crate                 │
│                                                     │
│  pub trait VideoDecoder {                            │
│      fn open(path) -> Result<Self>;                  │
│      fn next_frame() -> Result<Option<VideoFrame>>;  │
│      fn seek(time) -> Result<()>;                    │
│      fn duration() -> Duration;                      │
│      fn fps() -> f64;                                │
│      fn dimensions() -> (u32, u32);                  │
│      fn is_looping() -> bool;                        │
│      fn set_looping(bool);                           │
│  }                                                   │
│                                                     │
│  pub enum VideoFrame {                               │
│      /// CPU-side RGBA pixels (software fallback)    │
│      Rgba { data: Vec<u8>, width: u32, height: u32 },│
│      /// OS-native GPU-interop handle (zero-copy)    │
│      NativeTexture(NativeTextureHandle),             │
│  }                                                   │
│                                                     │
│  pub enum NativeTextureHandle {                      │
│      Metal { cv_pixel_buffer: *mut c_void },         │
│      D3d11 { texture: *mut c_void },                 │
│      DmaBuf { fd: i32, fourcc: u32, ... },           │
│      AHardwareBuffer { buffer: *mut c_void },        │
│  }                                                   │
└───────────┬──────────────────────────┬──────────────┘
            │ VideoFrame::NativeTexture│ VideoFrame::Rgba
            ▼                          ▼
┌───────────────────────┐  ┌─────────────────────────┐
│  wgpu HAL interop     │  │  queue.write_texture()  │
│  (zero-copy path)     │  │  (CPU upload fallback)  │
└───────────┬───────────┘  └────────────┬────────────┘
            │                           │
            ▼                           ▼
┌─────────────────────────────────────────────────────┐
│  BgVideo (renderer::scene.rs)                       │
│  - wgpu::Texture (updated every frame)              │
│  - bind_group + pipeline (既存 BgImage と共通)       │
│  - playback state (position, looping, paused)       │
└─────────────────────────────────────────────────────┘
```

## 2. プラットフォーム別技術選定

### 2.1 macOS — VideoToolbox + AVFoundation

```
MP4/MOV file
  → AVAssetReader (demux + read CMSampleBuffer)
    → VTDecompressionSession (H.264/HEVC HW decode)
      → CVPixelBuffer (IOSurface-backed, kCVPixelFormatType_32BGRA)
        → CVMetalTextureCacheCreateTextureFromImage()
          → MTLTexture (zero-copy, IOSurface 共有)
            → wgpu::hal::metal::Device::texture_from_raw()
              → wgpu::Texture
```

| 項目 | 詳細 |
|------|------|
| **Demuxer** | `AVAssetReader` + `AVAssetReaderTrackOutput` |
| **Decoder** | `VTDecompressionSession` (AVAssetReader が内部で使用) |
| **出力形式** | `CVPixelBuffer` (kCVPixelFormatType_32BGRA) |
| **GPU 転送** | `CVMetalTextureCache` — IOSurface 経由でゼロコピー |
| **Rust FFI** | `objc2` + `objc2-av-foundation` + `objc2-core-video` + `objc2-metal` |
| **対応コーデック** | H.264, HEVC, ProRes, VP9 (macOS 11+) |
| **wgpu interop** | `wgpu::hal::api::Metal` で `MTLTexture` → `wgpu::Texture` |

**ゼロコピーパス詳細:**
```rust
// 1. CVPixelBuffer から IOSurface を取得
let io_surface = CVPixelBufferGetIOSurface(pixel_buffer);

// 2. CVMetalTextureCache で MTLTexture にマップ
let mut cv_metal_tex: CVMetalTextureRef = null_mut();
CVMetalTextureCacheCreateTextureFromImage(
    kCFAllocatorDefault,
    texture_cache,        // CVMetalTextureCacheRef (device から作成)
    pixel_buffer,
    null(),
    MTLPixelFormatBGRA8Unorm,
    width, height,
    0,                    // plane index
    &mut cv_metal_tex,
);
let mtl_texture = CVMetalTextureGetTexture(cv_metal_tex);

// 3. wgpu HAL interop
let wgpu_texture = unsafe {
    device.create_texture_from_hal::<wgpu::hal::api::Metal>(
        hal_texture_from_mtl(mtl_texture),
        &wgpu::TextureDescriptor { ... },
    )
};
```

### 2.2 Windows — Media Foundation

```
MP4/WebM file
  → IMFSourceReader (demux + decode pipeline)
    → IMFMediaBuffer → IMFDXGIBuffer
      → ID3D11Texture2D (GPU memory)
        → wgpu::hal::dx12 shared texture
          → wgpu::Texture
```

| 項目 | 詳細 |
|------|------|
| **Demuxer + Decoder** | `IMFSourceReader` (MF が demux〜decode を一括管理) |
| **出力形式** | `IMFDXGIBuffer` → `ID3D11Texture2D` |
| **GPU 転送** | DXGI shared handle で D3D11→D3D12 interop、またはステージング経由 |
| **Rust FFI** | `windows` crate (`windows::Media::MediaFoundation`, `windows::Graphics::Direct3D11`) |
| **対応コーデック** | H.264, HEVC (拡張), VP9, AV1 (Win10 1809+) |
| **wgpu interop** | `wgpu::hal::api::Dx12` で共有テクスチャをインポート |

**フロー詳細:**
```rust
// 1. MFSourceReader を D3D11 デバイス付きで作成
let mut attributes: IMFAttributes = MFCreateAttributes(3)?;
attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &dxgi_manager)?;
attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
let reader = MFCreateSourceReaderFromURL(path, &attributes)?;

// 2. 出力を NV12 or RGB32 に設定
let media_type = MFCreateMediaType()?;
media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
media_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM, &media_type)?;

// 3. フレーム読み取り
let (_, _, _, sample) = reader.ReadSample(
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, 0
)?;
let buffer: IMFMediaBuffer = sample.ConvertToContiguousBuffer()?;
let dxgi_buffer: IMFDXGIBuffer = buffer.cast()?;
let texture: ID3D11Texture2D = dxgi_buffer.GetResource()?;

// 4. D3D11 → D3D12 shared handle → wgpu
//    (DXGI keyed mutex or NT handle で共有)
```

**注意:** MF の D3D11 出力と wgpu の D3D12 バックエンド間の interop は DXGI shared handle (`IDXGIResource1::CreateSharedHandle`) を使う。NV12 → RGBA の色変換が必要な場合は MF の Video Processor MFT か、WGSL コンピュートシェーダで変換。

### 2.3 Linux — Vulkan Video (優先) + GStreamer VA-API (フォールバック) + V4L2 (SBC)

#### フォールバック戦略

```
┌─────────────────────────────────────────────────────┐
│              Linux Video Decoder                     │
│                                                      │
│  1. Vulkan Video (VK_KHR_video_decode_h264)          │
│     → ランタイムで VkQueueVideoDecodeKHR 検出        │
│     → Vulkan 内完結、DMA-BUF 不要                    │
│     │                                                │
│     │ (非対応ドライバの場合)                          │
│     ▼                                                │
│  2. GStreamer + VA-API (cfg feature: gstreamer)       │
│     → gstreamer-rs 経由、DMA-BUF → Vulkan import    │
│     → decodebin3 が VA-API / SW を自動選択           │
│     │                                                │
│     │ (GStreamer 無し / 組み込み環境)                 │
│     ▼                                                │
│  3. V4L2 Stateless (cfg feature: v4l2)               │
│     → ioctl 直接、RPi/Rockchip 等 SBC 向け          │
│     → DMA-BUF export → Vulkan import                │
│     │                                                │
│     │ (全 HW デコーダ非対応)                          │
│     ▼                                                │
│  4. CPU ソフトウェア (常に利用可能)                    │
│     → image crate (GIF/APNG) + VideoFrame::Rgba     │
└─────────────────────────────────────────────────────┘
```

#### 2.3.1 Vulkan Video (優先パス — 追加依存なし)

```
MP4 file
  → mp4parse (Rust pure) で demux → H.264 NAL units
    → h264-reader (Rust pure) で SPS/PPS/Slice パース
      → VkVideoSessionKHR + VkVideoDecodeInfoKHR
        → VkImage (デコード結果、GPU メモリ上)
          → wgpu::hal::api::Vulkan::texture_from_raw()
            → wgpu::Texture
```

| 項目 | 詳細 |
|------|------|
| **Demuxer** | `mp4parse` crate (Rust pure、Mozilla製) |
| **NAL パーサ** | `h264-reader` crate (SPS/PPS/Slice header 解析) |
| **Decoder** | Vulkan Video Extensions (`VK_KHR_video_decode_queue` + `VK_KHR_video_decode_h264`) |
| **出力形式** | `VkImage` (GPU メモリ上、NV12 or RGBA) |
| **GPU 転送** | 不要 — Vulkan 内で完結 |
| **Rust FFI** | `ash` crate (Vulkan FFI — 型定義あり) |
| **外部ライブラリ依存** | **なし** (Vulkan ドライバのみ) |
| **対応ドライバ** | NVIDIA 535+, Mesa 23.1+ (RADV/ANV) |

**ランタイム検出:**
```rust
// ash で Video Decode キュー対応を確認
fn has_vulkan_video(instance: &ash::Instance, phys: vk::PhysicalDevice) -> bool {
    let queue_families = unsafe {
        instance.get_physical_device_queue_family_properties(phys)
    };
    queue_families.iter().any(|qf|
        qf.queue_flags.contains(vk::QueueFlags::VIDEO_DECODE_KHR)
    )
}
```

**Vulkan Video デコードセッション:**
```rust
// Video Session 作成 (H.264 プロファイル)
let profile_info = vk::VideoDecodeH264ProfileInfoKHR {
    std_profile_idc: StdVideoH264ProfileIdc::HIGH,
    picture_layout: vk::VideoDecodeH264PictureLayoutFlagsKHR::PROGRESSIVE,
    ..Default::default()
};
let profile = vk::VideoProfileInfoKHR {
    video_codec_operation: vk::VideoCodecOperationFlagsKHR::DECODE_H264,
    chroma_subsampling: vk::VideoChromaSubsamplingFlagsKHR::TYPE_420,
    luma_bit_depth: vk::VideoComponentBitDepthFlagsKHR::TYPE_8,
    chroma_bit_depth: vk::VideoComponentBitDepthFlagsKHR::TYPE_8,
    p_next: &profile_info as *const _ as *const _,
    ..Default::default()
};

let session_create = vk::VideoSessionCreateInfoKHR {
    queue_family_index: video_queue_family,
    p_video_profile: &profile,
    picture_format: vk::Format::G8_B8R8_2PLANE_420_UNORM, // NV12
    max_coded_extent: vk::Extent2D { width: 1920, height: 1080 },
    max_dpb_slots: 17,      // H.264 Level 4.1
    max_active_reference_pictures: 16,
    ..Default::default()
};
let video_session = video_decode_fn.create_video_session(device, &session_create, None)?;

// DPB (Decoded Picture Buffer) 用 VkImage 配列を確保
// → デコード後の VkImage を wgpu にインポート
```

#### 2.3.2 GStreamer + VA-API (フォールバック — cfg feature)

```
MP4/WebM file
  → GStreamer pipeline:
    filesrc → decodebin3 → vaapidecodebin (自動選択)
      → video/x-raw(memory:DMABuf)
        → DMA-BUF fd → VK_EXT_external_memory_dma_buf
          → wgpu::hal::api::Vulkan
```

| 項目 | 詳細 |
|------|------|
| **Demuxer + Decoder** | GStreamer `decodebin3` (VA-API / SW 自動選択) |
| **出力形式** | DMA-BUF fd |
| **GPU 転送** | `VK_EXT_external_memory_dma_buf` → Vulkan import |
| **Rust FFI** | `gstreamer` + `gstreamer-app` + `gstreamer-video` + `gstreamer-allocators` |
| **cfg feature** | `gstreamer` — 無効時はコンパイルから完全除外 |

**GStreamer 依存の最小化 (Cargo.toml):**
```toml
[target.'cfg(target_os = "linux")'.dependencies]
# GStreamer — VA-API フォールバック用 (feature flag で有効化)
gstreamer = { version = "0.23", optional = true }
gstreamer-app = { version = "0.23", optional = true }
gstreamer-video = { version = "0.23", optional = true }
gstreamer-allocators = { version = "0.23", optional = true }

# Vulkan Video (常に有効 — 優先パス)
ash = { version = "0.38", default-features = false, features = ["std", "debug"] }

# MP4 demux + H.264 parse (Vulkan Video / V4L2 用、pure Rust)
mp4parse = "0.17"
h264-reader = "0.7"

[features]
default = ["linux-vulkan-video"]
linux-vulkan-video = []             # 常に有効、ランタイム検出
linux-gstreamer = [                 # VA-API フォールバック
    "gstreamer",
    "gstreamer-app",
    "gstreamer-video",
    "gstreamer-allocators",
]
linux-v4l2 = []                     # V4L2 Stateless (SBC 向け)
```

**GStreamer パイプライン (最小構成):**
```rust
#[cfg(feature = "linux-gstreamer")]
mod gst_decoder {
    use gstreamer as gst;
    use gstreamer_app as gst_app;

    pub struct GstVaapiDecoder {
        pipeline: gst::Pipeline,
        appsink: gst_app::AppSink,
    }

    impl GstVaapiDecoder {
        pub fn new(path: &str) -> anyhow::Result<Self> {
            gst::init()?;

            // decodebin3 が VA-API 対応時は自動で vaapidecodebin を使用
            // VA-API 非対応時はソフトウェアデコーダにフォールバック
            let pipeline = gst::parse::launch(&format!(
                "filesrc location={path} ! decodebin3 ! \
                 video/x-raw(memory:DMABuf),format=NV12 ! appsink name=sink"
            ))?;
            // ...
            Ok(Self { pipeline, appsink })
        }
    }

    impl VideoDecoder for GstVaapiDecoder {
        fn next_frame(&mut self) -> Result<Option<VideoFrame>> {
            let sample = self.appsink.try_pull_sample(gst::ClockTime::from_mseconds(0));
            match sample {
                Some(s) => {
                    let buffer = s.buffer().unwrap();
                    let memory = buffer.peek_memory(0);
                    // DMA-BUF fd を取得して NativeTexture として返す
                    if let Some(dmabuf) = memory.downcast_ref::<gst_allocators::DmaBufMemory>() {
                        Ok(Some(VideoFrame::NativeTexture(NativeTextureHandle::DmaBuf {
                            fd: dmabuf.fd(),
                            // ...
                        })))
                    } else {
                        // ソフトウェアデコード → CPU RGBA
                        let map = buffer.map_readable()?;
                        Ok(Some(VideoFrame::Rgba { data: map.to_vec(), /* ... */ }))
                    }
                }
                None => Ok(None),
            }
        }
    }
}
```

#### 2.3.3 V4L2 Stateless (SBC 向け — cfg feature)

```
MP4 file
  → mp4parse (Rust) で demux → H.264 NAL units
    → V4L2 Stateless ioctl:
        VIDIOC_QBUF (NAL + SPS/PPS/Slice header)
          → VIDIOC_DQBUF (デコード済みフレーム)
            → VIDIOC_EXPBUF (DMA-BUF fd export)
              → VK_EXT_external_memory_dma_buf → wgpu
```

| 項目 | 詳細 |
|------|------|
| **Demuxer** | `mp4parse` (Vulkan Video と共通) |
| **NAL パーサ** | `h264-reader` (Vulkan Video と共通) |
| **Decoder** | V4L2 Stateless API (ioctl 直接) |
| **出力形式** | DMA-BUF fd (VIDIOC_EXPBUF) |
| **GPU 転送** | GStreamer VA-API と同じ DMA-BUF → Vulkan パス |
| **Rust FFI** | `nix` crate (ioctl) or `v4l` crate |
| **対応 HW** | Raspberry Pi (bcm2835), Rockchip (rkvdec), Hantro, Cedrus |
| **cfg feature** | `v4l2` — 組み込み向け、デフォルト無効 |

### 2.4 iOS — AVFoundation (VideoToolbox 内蔵)

```
MP4/MOV file
  → AVAssetReader + AVAssetReaderTrackOutput
    → CMSampleBuffer → CVPixelBuffer (IOSurface-backed)
      → CVMetalTextureCacheCreateTextureFromImage()
        → MTLTexture (zero-copy)
          → wgpu::hal::api::Metal
```

| 項目 | 詳細 |
|------|------|
| **Demuxer + Decoder** | `AVAssetReader` (macOS と共通 API) |
| **出力形式** | `CVPixelBuffer` (kCVPixelFormatType_32BGRA) |
| **GPU 転送** | `CVMetalTextureCache` — macOS と同一パス |
| **Rust FFI** | `objc2` 系クレート (macOS と共通) |
| **対応コーデック** | H.264, HEVC (A9 チップ以降) |
| **wgpu interop** | macOS と同一 (`wgpu::hal::api::Metal`) |

**macOS との差分:**
- API はほぼ同一。`#[cfg(any(target_os = "macos", target_os = "ios"))]` で共通コード化
- iOS は `AVAssetReader` の `timeRange` 制限に注意（バックグラウンドでの長時間再生制約）
- Metal Feature Set の差異は wgpu が吸収

### 2.5 Android — MediaCodec + AHardwareBuffer

```
MP4/WebM file
  → AMediaExtractor (demux, NDK C API)
    → AMediaCodec (HW decode, configure with ANativeWindow or Surface)
      → AHardwareBuffer (output buffer)
        → VK_ANDROID_external_memory_android_hardware_buffer
          → VkImage (zero-copy)
            → wgpu::hal::api::Vulkan
```

| 項目 | 詳細 |
|------|------|
| **Demuxer** | `AMediaExtractor` (NDK) |
| **Decoder** | `AMediaCodec` (NDK) |
| **出力形式** | `AHardwareBuffer` (API 26+) |
| **GPU 転送** | `VK_ANDROID_external_memory_android_hardware_buffer` でゼロコピー |
| **Rust FFI** | `ndk` crate (`ndk::media::MediaCodec`, `ndk::hardware_buffer`) |
| **対応コーデック** | H.264 (全デバイス), HEVC (大半), VP9, AV1 (Pixel 6+) |
| **wgpu interop** | `wgpu::hal::api::Vulkan` で `VkImage` を wrap |

**フロー詳細:**
```rust
// NDK crate を使用
let extractor = AMediaExtractor::new()?;
extractor.set_data_source(path)?;

let codec = AMediaCodec::create_decoder_by_type("video/avc")?;
codec.configure(&format, None /* no surface, use buffers */, 0)?;
codec.start()?;

// デコードループ
let buf_idx = codec.dequeue_output_buffer(timeout)?;
let ahardware_buffer = codec.get_output_hardware_buffer(buf_idx)?;

// Vulkan import
// VkImportAndroidHardwareBufferInfoANDROID → VkImage → wgpu HAL
```

## 3. 比較まとめ

| | macOS | Windows | Linux (優先) | Linux (FB) | iOS | Android |
|---|---|---|---|---|---|---|
| **Demuxer** | AVAssetReader | IMFSourceReader | mp4parse (Rust) | GStreamer | AVAssetReader | AMediaExtractor |
| **Decoder** | VideoToolbox | MF Transform | Vulkan Video | VA-API (auto) | VideoToolbox | AMediaCodec |
| **GPU interop** | CVMetalTextureCache | DXGI SharedHandle | Vulkan 内完結 | DMA-BUF→Vulkan | CVMetalTextureCache | AHardwareBuffer→Vulkan |
| **wgpu backend** | Metal | DX12 | Vulkan | Vulkan | Metal | Vulkan |
| **ゼロコピー** | ✅ IOSurface | ⚠️ D3D11→D3D12 | ✅ Vulkan内 | ✅ DMA-BUF | ✅ IOSurface | ✅ AHardwareBuffer |
| **外部依存** | OS 標準 | OS 標準 | **なし** | libgstreamer | OS 標準 | NDK |
| **Rust FFI** | objc2 系 | windows crate | ash | gstreamer-rs | objc2 系 | ndk crate |
| **色変換** | BGRA 直接 | NV12→RGBA 要 | NV12→RGBA 要 | NV12→RGBA 要 | BGRA 直接 | NV12→RGBA 要 |

## 4. クレート構成

```
crates/
  video-decoder/
    Cargo.toml
    src/
      lib.rs          # VideoDecoder trait, VideoFrame, NativeTextureHandle
      demux.rs        # mp4parse ベース demuxer (Vulkan Video / V4L2 共通)
      macos.rs        # AVFoundation + CVMetalTextureCache
      ios.rs          # → macos.rs を re-export (cfg alias)
      windows.rs      # Media Foundation
      linux/
        mod.rs        # Linux デコーダ選択ロジック (ランタイムフォールバック)
        vulkan_video.rs  # Vulkan Video (ash 直接)
        gst_vaapi.rs     # GStreamer + VA-API (#[cfg(feature = "linux-gstreamer")])
        v4l2.rs          # V4L2 Stateless (#[cfg(feature = "linux-v4l2")])
      android.rs      # MediaCodec + AHardwareBuffer
      software.rs     # CPU ソフトウェアデコード (最終フォールバック)
```

**Cargo.toml:**
```toml
[package]
name = "video-decoder"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = { workspace = true }
log = { workspace = true }

# 共通: demux + NAL parse (pure Rust, 全プラットフォーム)
mp4parse = "0.17"
h264-reader = "0.7"

# 共通: 最終フォールバック
image = { workspace = true }

[target.'cfg(any(target_os = "macos", target_os = "ios"))'.dependencies]
objc2 = "0.6"
objc2-foundation = { version = "0.3", features = ["NSString", "NSURL"] }
objc2-av-foundation = { version = "0.3", features = [
    "AVAssetReader", "AVAssetReaderOutput",
    "AVAssetReaderTrackOutput", "AVAsset",
    "AVURLAsset", "AVAssetTrack", "AVMediaFormat",
] }
objc2-core-media = { version = "0.3", features = ["CMSampleBuffer", "CMTime"] }
objc2-core-video = { version = "0.3", features = [
    "CVPixelBuffer", "CVMetalTextureCache", "CVMetalTexture",
    "CVImageBuffer",
] }
objc2-metal = { version = "0.3", features = ["MTLTexture", "MTLDevice"] }

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Media_MediaFoundation",              # IMFSourceReader, IMFMediaType
    "Win32_Media_MediaFoundation",        # MFCreateSourceReaderFromURL
    "Win32_Graphics_Direct3D11",          # ID3D11Texture2D, ID3D11Device
    "Win32_Graphics_Dxgi",               # IDXGIResource1, SharedHandle
] }

[target.'cfg(target_os = "linux")'.dependencies]
ash = { version = "0.38", default-features = false, features = ["std", "debug"] }
# GStreamer (VA-API フォールバック) — feature flag で有効化
gstreamer = { version = "0.23", optional = true }
gstreamer-app = { version = "0.23", optional = true }
gstreamer-video = { version = "0.23", optional = true }
gstreamer-allocators = { version = "0.23", optional = true }
# V4L2 (SBC 向け) — feature flag で有効化
nix = { version = "0.29", features = ["ioctl", "mman"], optional = true }

[target.'cfg(target_os = "android")'.dependencies]
ndk = { version = "0.9", features = ["media", "hardware_buffer"] }

[features]
default = ["linux-vulkan-video"]
linux-vulkan-video = []                    # Vulkan Video (常時有効、ランタイム検出)
linux-gstreamer = [                        # GStreamer + VA-API フォールバック
    "dep:gstreamer",
    "dep:gstreamer-app",
    "dep:gstreamer-video",
    "dep:gstreamer-allocators",
]
linux-v4l2 = ["dep:nix"]                   # V4L2 Stateless (SBC 向け)
linux-all = ["linux-vulkan-video", "linux-gstreamer", "linux-v4l2"]
```

**Linux デコーダ選択ロジック (linux/mod.rs):**
```rust
pub fn create_decoder(path: &str, vk_instance: &ash::Instance, vk_phys: vk::PhysicalDevice) -> Box<dyn VideoDecoder> {
    // 1. Vulkan Video (常にコンパイルされる、ランタイム検出)
    if vulkan_video::is_supported(vk_instance, vk_phys) {
        if let Ok(dec) = vulkan_video::VulkanVideoDecoder::new(path, vk_instance, vk_phys) {
            log::info!("Using Vulkan Video decoder");
            return Box::new(dec);
        }
    }

    // 2. GStreamer + VA-API (feature flag でコンパイル制御)
    #[cfg(feature = "linux-gstreamer")]
    {
        if let Ok(dec) = gst_vaapi::GstVaapiDecoder::new(path) {
            log::info!("Using GStreamer VA-API decoder");
            return Box::new(dec);
        }
    }

    // 3. V4L2 Stateless (feature flag でコンパイル制御)
    #[cfg(feature = "linux-v4l2")]
    {
        if let Ok(dec) = v4l2::V4l2Decoder::new(path) {
            log::info!("Using V4L2 Stateless decoder");
            return Box::new(dec);
        }
    }

    // 4. CPU ソフトウェア (常に利用可能)
    log::warn!("No HW decoder available, falling back to software decode");
    Box::new(software::SoftwareDecoder::new(path).expect("software decode must succeed"))
}
```

## 5. renderer 統合

既存の `BgImage` を `BgMedia` に拡張し、静止画 / GIF / 動画を統一的に扱う:

```rust
enum BgMedia {
    /// 既存の静止画 / GIF パス
    Image(BgImage),
    /// 動画デコーダ経由
    Video(BgVideo),
}

struct BgVideo {
    decoder: Box<dyn VideoDecoder>,
    /// 毎フレーム更新される GPU テクスチャ
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    /// ネイティブ interop の場合、テクスチャ自体が差し替わる
    /// → bind_group も再作成が必要
    needs_bind_group_update: bool,
    playback: PlaybackState,
}

struct PlaybackState {
    position: std::time::Duration,
    duration: std::time::Duration,
    fps: f64,
    looping: bool,
    paused: bool,
    last_tick: std::time::Instant,
}
```

**tick 処理:**
```rust
impl BgVideo {
    fn tick(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, dt: Duration) {
        self.playback.position += dt;
        if self.playback.position >= self.playback.duration {
            if self.playback.looping {
                self.decoder.seek(Duration::ZERO).ok();
                self.playback.position = Duration::ZERO;
            } else {
                return;
            }
        }

        match self.decoder.next_frame() {
            Ok(Some(VideoFrame::NativeTexture(handle))) => {
                // ゼロコピーパス: ネイティブテクスチャを wgpu に wrap
                // → self.texture を差し替え、bind_group を再作成
                self.texture = wrap_native_texture(device, handle);
                self.needs_bind_group_update = true;
            }
            Ok(Some(VideoFrame::Rgba { data, width, height })) => {
                // CPU フォールバック: queue.write_texture() (既存 GIF と同じパス)
                queue.write_texture(/* ... */);
            }
            _ => {}
        }
    }
}
```

## 6. NV12 色変換シェーダ (Windows / Linux / Android 共通)

macOS/iOS は BGRA 出力のため不要。他プラットフォームの HW デコーダは NV12 出力が一般的。

```wgsl
// nv12_to_rgba.wgsl
@group(0) @binding(0) var y_tex: texture_2d<f32>;
@group(0) @binding(1) var uv_tex: texture_2d<f32>;
@group(0) @binding(2) var out_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let y  = textureLoad(y_tex, gid.xy, 0).r;
    let uv = textureLoad(uv_tex, gid.xy / 2, 0).rg;
    let u = uv.x - 0.5;
    let v = uv.y - 0.5;
    // BT.709
    let r = y + 1.5748 * v;
    let g = y - 0.1873 * u - 0.4681 * v;
    let b = y + 1.8556 * u;
    textureStore(out_tex, gid.xy, vec4<f32>(r, g, b, 1.0));
}
```

## 7. 実装フェーズ

### Phase A: 共通基盤 (全プラットフォーム共通)
1. `video-decoder` クレート作成、`VideoDecoder` trait 定義
2. `demux.rs` — `mp4parse` + `h264-reader` で MP4 → NAL unit ストリーム
3. `software.rs` — CPU ソフトウェアデコード (最終フォールバック)
4. `BgMedia` enum で `BgImage` / `BgVideo` を統合
5. `Scene` の既存 API を拡張して動画対応
6. NV12 → RGBA WGSL コンピュートシェーダ

### Phase B: macOS / iOS (Metal ゼロコピー)
1. `objc2-av-foundation` で `AVAssetReader` → `CVPixelBuffer` フレーム取得
2. `CVMetalTextureCache` → `MTLTexture` ゼロコピーマッピング
3. `wgpu::hal::api::Metal::texture_from_raw()` で wgpu テクスチャ化
4. seek / looping / duration 実装

### Phase C: Windows (Media Foundation + D3D12)
1. `windows` crate で `IMFSourceReader` 初期化 (D3D11 デバイスマネージャ付き)
2. `IMFDXGIBuffer` → `ID3D11Texture2D` フレーム取得
3. D3D11 → D3D12 DXGI shared handle interop
4. NV12 → RGBA 変換 (WGSL compute shader)
5. `wgpu::hal::api::Dx12` で wgpu テクスチャ化

### Phase D: Linux — Vulkan Video (優先パス)
1. `ash` で Video Session 作成、ランタイム検出ロジック
2. `mp4parse` + `h264-reader` で demux → NAL ストリーム
3. DPB (Decoded Picture Buffer) 管理
4. VkImage → wgpu テクスチャ化
5. NV12 → RGBA 変換 (WGSL compute shader)

### Phase E: Linux — GStreamer VA-API (フォールバック)
1. `gstreamer-rs` でパイプライン構築 (`decodebin3` → `appsink`)
2. DMA-BUF fd 取得 → Vulkan external memory import
3. `#[cfg(feature = "linux-gstreamer")]` でコンパイル制御
4. VA-API 非対応時の GStreamer 自動ソフトウェアフォールバック確認

### Phase F: Linux — V4L2 Stateless (SBC 向け、オプション)
1. ioctl で V4L2 Stateless デバイス検出
2. NAL submit → デコード → DMA-BUF export
3. `#[cfg(feature = "linux-v4l2")]` でコンパイル制御

### Phase G: Android (MediaCodec + AHardwareBuffer)
1. `ndk` crate で `AMediaExtractor` + `AMediaCodec` セットアップ
2. `AHardwareBuffer` 出力取得
3. `VK_ANDROID_external_memory_android_hardware_buffer` で Vulkan インポート
4. `wgpu::hal::api::Vulkan` で wgpu テクスチャ化

## 8. 優先度とリスク

| 優先度 | プラットフォーム | デコーダ | リスク | 備考 |
|--------|----------------|----------|--------|------|
| **1** | macOS | VideoToolbox | 低 | 現在の開発環境。Phase B を最優先 |
| **2** | Linux | Vulkan Video | 中 — ash FFI 手書き、DPB 管理複雑 | 外部依存ゼロが魅力 |
| **3** | Linux | GStreamer VA-API | 低 — gstreamer-rs 成熟 | cfg で排除可能なフォールバック |
| **4** | Windows | Media Foundation | 中 — D3D11→D3D12 interop | windows crate は公式で安定 |
| **5** | iOS | VideoToolbox | 低 — macOS コード 90% 再利用 | Phase B 完了後すぐ対応可能 |
| **6** | Android | MediaCodec | 高 — ndk media FFI 薄い | NDK C API を直接 FFI する可能性 |
| **7** | Linux | V4L2 Stateless | 中 — ioctl 直接 | SBC 限定、必要時のみ実装 |

## 9. wgpu HAL interop の制約事項

- `wgpu::hal` API は **unstable** — wgpu のメジャーバージョンアップで壊れる可能性あり
- `device.create_texture_from_hal()` を使う場合、`wgpu` の `"expose-ids"` feature が必要
- ネイティブテクスチャの寿命管理に注意 — CVPixelBuffer / ID3D11Texture2D / VkImage がドロップされるとテクスチャが無効化
- フォールバック (CPU upload) パスは常に維持し、HAL interop 失敗時に自動的に切り替わる設計にする

## 10. 共有コンポーネント

```
demux (mp4parse + h264-reader)
  → Vulkan Video / V4L2 Stateless 共通 (Linux)

DMA-BUF → Vulkan import
  → GStreamer VA-API / V4L2 Stateless 共通 (Linux)

NV12 → RGBA WGSL compute shader
  → Windows / Linux / Android 共通

CVMetalTextureCache → wgpu Metal HAL
  → macOS / iOS 共通

VideoDecoder trait + BgVideo
  → 全プラットフォーム共通
```
