# video-decoder crate — 詳細設計書

> **目的**: wgpu アプリケーションから GPU ネイティブテクスチャポインタを受け取り、
> プラットフォームネイティブ HW デコーダで動画フレームをゼロコピーで書き込む独立ライブラリクレート。
>
> **本ドキュメントの対象読者**: 実装担当者（別の開発者が読んでも同一の実装に至る粒度で記述）

---

## 1. 設計思想

### 1.1 コアコンセプト

このクレートは **「動画デコーダ」ではなく「動画テクスチャソース」** として設計する。

- アプリケーション側が GPU テクスチャ（wgpu::Texture または各バックエンドのネイティブハンドル）を**所有**する
- 本クレートはそのテクスチャに対してデコード済みフレームを**書き込む**
- テクスチャの生成・破棄・バインドグループへの組み込みはアプリケーション側の責務

```
┌──────────────────────┐          ┌──────────────────────────┐
│  wgpu Application    │          │  video-decoder crate     │
│                      │          │                          │
│  1. Texture 生成     │          │                          │
│  2. ネイティブハンドル取得 ──────→  3. open(path, handle)   │
│                      │          │  4. decode_next_frame()  │
│                      │  ←──────── (テクスチャに直接書込み) │
│  5. Texture を描画   │          │                          │
└──────────────────────┘          └──────────────────────────┘
```

### 1.2 なぜこの設計か

| 選択肢 | 問題 |
|--------|------|
| クレートがテクスチャを生成して返す | wgpu::Texture の寿命がクレート内部に閉じ、bind_group 再作成がフレーム毎に必要 |
| クレートが RGBA バイト列を返す | CPU→GPU コピーが毎フレーム発生、ゼロコピーの意味がない |
| **アプリがテクスチャを渡し、クレートが書き込む** | テクスチャ所有権がアプリ側、ゼロコピー可能、bind_group は初回のみ |

---

## 2. E-R 図 (Entity-Relationship)

```
┌─────────────────────┐       ┌──────────────────────────┐
│  VideoSource        │       │  OutputTarget            │
│─────────────────────│       │──────────────────────────│
│  PK: source_id      │       │  PK: target_id           │
│  path: String       │       │  native_handle: RawPtr   │
│  codec: Codec       │       │  format: PixelFormat     │
│  width: u32         │       │  width: u32              │
│  height: u32        │       │  height: u32             │
│  duration: Duration │       │  color_space: ColorSpace │
│  fps: f64           │       └──────────┬───────────────┘
└────────┬────────────┘                  │
         │ 1                             │ 1
         │                               │
         │ creates                       │ writes_to
         │                               │
         ▼ 1                             │
┌─────────────────────┐                  │
│  Decoder            │──────────────────┘
│─────────────────────│
│  PK: decoder_id     │       ┌──────────────────────────┐
│  backend: Backend   │       │  DemuxedStream           │
│  state: State       │       │──────────────────────────│
│  position: Duration │  1..* │  PK: stream_id           │
│  looping: bool      │◄──────│  FK: source_id           │
│  frame_count: u64   │       │  track_index: usize      │
└────────┬────────────┘       │  codec: Codec            │
         │                    │  parameter_sets: Vec<u8> │
         │ manages            │  (SPS/PPS/VPS)           │
         │                    └──────────────────────────┘
         ▼ 0..*
┌─────────────────────┐       ┌──────────────────────────┐
│  DpbSlot            │       │  NV12ConvertPass         │
│─────────────────────│       │──────────────────────────│
│  PK: slot_index     │       │  PK: pass_id             │
│  FK: decoder_id     │       │  y_texture: RawPtr       │
│  vk_image: RawPtr   │       │  uv_texture: RawPtr      │
│  in_use: bool       │       │  output_texture: RawPtr  │
│  poc: i32           │       │  pipeline: ComputePipe   │
│  (picture order cnt)│       │  color_space: ColorSpace │
└─────────────────────┘       └──────────────────────────┘
```

### エンティティ説明

| エンティティ | 責務 |
|---|---|
| **VideoSource** | 入力動画ファイルのメタデータ。open 時に確定 |
| **OutputTarget** | アプリから渡される GPU テクスチャ。フォーマット・サイズを保持 |
| **Decoder** | プラットフォーム固有の HW デコーダインスタンス。状態遷移を管理 |
| **DemuxedStream** | demux 済みのビデオトラック。parameter sets (SPS/PPS) を保持 |
| **DpbSlot** | Decoded Picture Buffer のスロット (Vulkan Video / V4L2 用) |
| **NV12ConvertPass** | NV12 → RGBA GPU 色変換パス (macOS/iOS 以外で使用) |

---

## 3. シーケンス図

### 3.1 初期化シーケンス

```
Application                    video-decoder                  OS / GPU
    │                              │                              │
    │  1. wgpu::Texture 生成       │                              │
    │     (RGBA8, VIDEO_DECODE用)  │                              │
    │                              │                              │
    │  2. get_native_handle()      │                              │
    │     → MTLTexture* /          │                              │
    │       ID3D11Texture2D* /     │                              │
    │       VkImage                │                              │
    │                              │                              │
    │  3. VideoSession::open(      │                              │
    │       path,                  │                              │
    │       OutputTarget {         │                              │
    │         native_handle,       │                              │
    │         format, w, h         │                              │
    │       },                     │                              │
    │       config                 │                              │
    │     )                        │                              │
    │──────────────────────────────►                              │
    │                              │  4. ファイルヘッダ読取        │
    │                              │     mp4parse::read_mp4()     │
    │                              │                              │
    │                              │  5. コーデック判定            │
    │                              │     → H.264 / HEVC / VP9     │
    │                              │                              │
    │                              │  6. HW デコーダ検出          │
    │                              │──────────────────────────────►
    │                              │     [macOS] VTDecompression   │
    │                              │     [Win]   MFSourceReader    │
    │                              │     [Linux] VkVideoSession    │
    │                              │       or GStreamer pipeline   │
    │                              │     [Android] AMediaCodec    │
    │                              │◄──────────────────────────────
    │                              │                              │
    │                              │  7. NV12→RGBA パイプライン   │
    │                              │     (macOS/iOS 以外)         │
    │                              │     WGSL compute shader 作成 │
    │                              │                              │
    │  8. Ok(VideoSession)         │                              │
    │◄──────────────────────────────                              │
    │                              │                              │
    │  9. bind_group に Texture    │                              │
    │     を組み込み               │                              │
```

### 3.2 フレームデコードシーケンス (macOS — ゼロコピー)

```
Application              VideoSession             AVFoundation        Metal / wgpu
    │                        │                        │                    │
    │  decode_frame(dt)      │                        │                    │
    │────────────────────────►                        │                    │
    │                        │  read_sample()         │                    │
    │                        │────────────────────────►                    │
    │                        │  CMSampleBuffer        │                    │
    │                        │◄────────────────────────                    │
    │                        │                        │                    │
    │                        │  CVPixelBuffer 取得    │                    │
    │                        │  (IOSurface-backed)    │                    │
    │                        │                        │                    │
    │                        │  CVMetalTextureCache   │                    │
    │                        │  CreateTextureFromImage │                   │
    │                        │─────────────────────────────────────────────►
    │                        │                        │   MTLTexture       │
    │                        │                        │   (IOSurface共有)  │
    │                        │◄─────────────────────────────────────────────
    │                        │                        │                    │
    │                        │  blit: MTLTexture      │                    │
    │                        │  → OutputTarget の     │                    │
    │                        │    MTLTexture にコピー │                    │
    │                        │─────────────────────────────────────────────►
    │                        │                        │  GPU blit cmd      │
    │                        │◄─────────────────────────────────────────────
    │                        │                        │                    │
    │  Ok(FrameStatus::      │                        │                    │
    │    NewFrame)            │                        │                    │
    │◄────────────────────────                        │                    │
    │                        │                        │                    │
    │  render (texture は    │                        │                    │
    │  既に更新済み)          │                        │                    │
```

### 3.3 フレームデコードシーケンス (Windows D3D12 Video — NV12 変換あり)

```
Application          VideoSession          D3D12 Video API         WGSL Compute
    │                    │                      │                      │
    │  decode_frame(dt)  │                      │                      │
    │────────────────────►                      │                      │
    │                    │  NAL unit 取得       │                      │
    │                    │  (mp4parse+h264)     │                      │
    │                    │                      │                      │
    │                    │  DecodeFrame()       │                      │
    │                    │  (ID3D12VideoDecode  │                      │
    │                    │   CommandList)       │                      │
    │                    │──────────────────────►                      │
    │                    │  D3D12 Texture (NV12)│                      │
    │                    │◄──────────────────────                      │
    │                    │                      │                      │
    │                    │  NV12→RGBA dispatch  │                      │
    │                    │  src: NV12 D3D12 Tex │                      │
    │                    │  dst: OutputTarget   │                      │
    │                    │─────────────────────────────────────────────►
    │                    │                      │   compute dispatch   │
    │                    │◄─────────────────────────────────────────────
    │                    │                      │                      │
    │                    │  ExecuteCommandLists │                      │
    │                    │  + fence signal      │                      │
    │                    │──────────────────────►                      │
    │                    │◄──────────────────────                      │
    │                    │                      │                      │
    │  Ok(FrameStatus::  │                      │                      │
    │    NewFrame)        │                      │                      │
    │◄────────────────────                      │                      │
```

### 3.4 フレームデコードシーケンス (Linux Vulkan Video — NV12 変換あり)

```
Application          VideoSession          ash / VulkanVideo       WGSL Compute
    │                    │                      │                      │
    │  decode_frame(dt)  │                      │                      │
    │────────────────────►                      │                      │
    │                    │  NAL unit 取得       │                      │
    │                    │  (mp4parse+h264)     │                      │
    │                    │                      │                      │
    │                    │  vkCmdDecodeVideo    │                      │
    │                    │──────────────────────►                      │
    │                    │  VkImage (NV12)      │                      │
    │                    │◄──────────────────────                      │
    │                    │                      │                      │
    │                    │  NV12→RGBA dispatch  │                      │
    │                    │  src: NV12 VkImage   │                      │
    │                    │  dst: OutputTarget   │                      │
    │                    │─────────────────────────────────────────────►
    │                    │                      │   compute dispatch   │
    │                    │◄─────────────────────────────────────────────
    │                    │                      │                      │
    │  Ok(FrameStatus::  │                      │                      │
    │    NewFrame)        │                      │                      │
    │◄────────────────────                      │                      │
```

### 3.5 フレームデコードシーケンス (GStreamer VA-API フォールバック)

```
Application          VideoSession          GStreamer               Vulkan
    │                    │                    │                      │
    │  decode_frame(dt)  │                    │                      │
    │────────────────────►                    │                      │
    │                    │  pull_sample()     │                      │
    │                    │───────────────────►│                      │
    │                    │  GstSample         │                      │
    │                    │  (DMA-BUF memory)  │                      │
    │                    │◄───────────────────│                      │
    │                    │                    │                      │
    │                    │  DMA-BUF fd 取得   │                      │
    │                    │                    │                      │
    │                    │  VkImportMemoryFd  │                      │
    │                    │  → temp VkImage    │                      │
    │                    │─────────────────────────────────────────►│
    │                    │                    │  external memory    │
    │                    │◄─────────────────────────────────────────│
    │                    │                    │                      │
    │                    │  NV12→RGBA dispatch│                      │
    │                    │  (Vulkan Video と  │                      │
    │                    │   同じパス)        │                      │
    │                    │─────────────────────────────────────────►│
    │                    │◄─────────────────────────────────────────│
    │                    │                    │                      │
    │  Ok(FrameStatus::  │                    │                      │
    │    NewFrame)        │                    │                      │
    │◄────────────────────                    │                      │
```

### 3.6 ループ再生 / seek シーケンス

```
Application              VideoSession             Decoder
    │                        │                        │
    │  decode_frame(dt)      │                        │
    │────────────────────────►                        │
    │                        │  position += dt        │
    │                        │  position > duration?  │
    │                        │  → Yes & looping=true  │
    │                        │                        │
    │                        │  seek(0)               │
    │                        │────────────────────────►
    │                        │  [macOS] AVAssetReader  │
    │                        │    再作成 (seek不可のため)│
    │                        │  [Win/D3D12] DPB reset  │
    │                        │    + demuxer seek       │
    │                        │  [Win/MF] IMFSourceReader│
    │                        │    .SetCurrentPosition()│
    │                        │  [Linux/Vk] DPB reset  │
    │                        │    + demuxer seek       │
    │                        │  [Linux/GSt] pipeline  │
    │                        │    seek event           │
    │                        │◄────────────────────────
    │                        │                        │
    │                        │  decode first frame    │
    │                        │────────────────────────►
    │                        │◄────────────────────────
    │                        │                        │
    │  Ok(FrameStatus::NewFrame)                      │
    │◄────────────────────────                        │
```

---

## 4. モジュール構成図

```
video-decoder (crate root)
│
├── lib.rs ─────────────────── パブリック API (trait, types, re-exports)
│   │
│   ├── pub trait VideoSession     ← メイン trait
│   ├── pub struct OutputTarget    ← GPU テクスチャハンドル
│   ├── pub struct SessionConfig   ← 設定
│   ├── pub enum FrameStatus       ← デコード結果
│   ├── pub enum VideoError        ← エラー型
│   ├── pub enum Codec             ← コーデック種別
│   ├── pub enum Backend           ← 使用中バックエンド
│   ├── pub struct VideoInfo       ← メタデータ
│   └── pub fn open()              ← エントリポイント (バックエンド自動選択)
│
├── demux/ ─────────────────── コンテナ demux (pure Rust)
│   ├── mod.rs                     pub trait Demuxer + factory
│   ├── mp4.rs                     MP4/MOV demuxer (mp4parse)
│   └── matroska.rs                WebM/MKV demuxer (将来)
│
├── nal/ ──────────────────── NAL unit パーサ (pure Rust)
│   ├── mod.rs                     pub trait NalParser
│   ├── h264.rs                    H.264 SPS/PPS/Slice (h264-reader)
│   └── h265.rs                    HEVC VPS/SPS/PPS (将来)
│
├── convert/ ──────────────── 色変換
│   ├── mod.rs                     pub struct NV12ToRgbaPass
│   ├── nv12_to_rgba.wgsl          WGSL コンピュートシェーダ
│   └── color_space.rs             BT.601 / BT.709 パラメータ
│
├── backend/ ──────────────── プラットフォーム固有デコーダ
│   ├── mod.rs                     Backend enum, 検出・選択ロジック
│   │
│   ├── apple.rs ──────────── macOS + iOS 共通
│   │   cfg(any(target_os = "macos", target_os = "ios"))
│   │   AVAssetReader → CVPixelBuffer → Metal blit
│   │
│   ├── d3d12_video.rs ────── Windows 優先パス
│   │   cfg(target_os = "windows")
│   │   ID3D12VideoDecodeCommandList → D3D12 Texture (D3D12 内完結)
│   │
│   ├── media_foundation.rs ─ Windows フォールバック
│   │   cfg(target_os = "windows")
│   │   IMFSourceReader (HW decode) → D3D11 → DXGI SharedHandle → D3D12
│   │
│   ├── vulkan_video.rs ───── Linux 優先パス
│   │   cfg(target_os = "linux")
│   │   ash VkVideoSession → VkImage → compute convert
│   │
│   ├── gst_vaapi.rs ─────── Linux GStreamer フォールバック
│   │   cfg(all(target_os = "linux", feature = "gstreamer"))
│   │   GStreamer decodebin3 → DMA-BUF → Vulkan import
│   │
│   ├── v4l2.rs ──────────── Linux V4L2 Stateless (SBC)
│   │   cfg(all(target_os = "linux", feature = "v4l2"))
│   │   ioctl → DMA-BUF → Vulkan import
│   │
│   ├── media_codec.rs ───── Android
│   │   cfg(target_os = "android")
│   │   AMediaCodec → AHardwareBuffer → Vulkan import
│   │
│   └── software.rs ─────── CPU フォールバック (全プラットフォーム)
│       image crate → RGBA bytes → queue.write_texture()
│
└── util/ ─────────────────── 共用ユーティリティ
    ├── mod.rs
    ├── ring_buffer.rs             DPB / フレームバッファリング用リングバッファ
    └── timestamp.rs               PTS/DTS 管理、フレームレート制御
```

---

## 5. ディレクトリ構成

```
crates/video-decoder/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs
│   │
│   ├── demux/
│   │   ├── mod.rs
│   │   ├── mp4.rs
│   │   └── matroska.rs
│   │
│   ├── nal/
│   │   ├── mod.rs
│   │   ├── h264.rs
│   │   └── h265.rs
│   │
│   ├── convert/
│   │   ├── mod.rs
│   │   ├── nv12_to_rgba.wgsl
│   │   └── color_space.rs
│   │
│   ├── backend/
│   │   ├── mod.rs
│   │   ├── apple.rs
│   │   ├── d3d12_video.rs
│   │   ├── media_foundation.rs
│   │   ├── vulkan_video.rs
│   │   ├── gst_vaapi.rs
│   │   ├── v4l2.rs
│   │   ├── media_codec.rs
│   │   └── software.rs
│   │
│   └── util/
│       ├── mod.rs
│       ├── ring_buffer.rs
│       └── timestamp.rs
│
├── shaders/
│   └── nv12_to_rgba.wgsl          (src/convert/ からインクルード)
│
├── tests/
│   ├── integration_open.rs        ファイルオープン + メタデータ取得
│   ├── integration_decode.rs      フレームデコード (SW fallback)
│   └── test_fixtures/
│       ├── test_h264_360p.mp4     テスト用動画 (小サイズ)
│       └── test_vp9_360p.webm
│
├── examples/
│   ├── decode_to_png.rs           動画 → 連番 PNG 出力 (HW不要)
│   └── wgpu_video_bg.rs           wgpu ウィンドウに動画背景 (E2E)
│
└── benches/
    └── decode_throughput.rs        デコードスループット計測
```

---

## 6. パブリック API 詳細

### 6.1 lib.rs — 型定義

```rust
//! # video-decoder
//!
//! プラットフォームネイティブ HW デコーダを用いて、
//! wgpu テクスチャに動画フレームをゼロコピーで書き込むライブラリ。
//!
//! ## 基本的な使い方
//!
//! ```rust,no_run
//! use video_decoder::{open, OutputTarget, SessionConfig, NativeHandle};
//!
//! // 1. wgpu テクスチャを作成
//! let texture = device.create_texture(&wgpu::TextureDescriptor {
//!     format: wgpu::TextureFormat::Rgba8UnormSrgb,
//!     usage: wgpu::TextureUsages::TEXTURE_BINDING
//!          | wgpu::TextureUsages::COPY_DST
//!          | wgpu::TextureUsages::STORAGE_BINDING,
//!     ..
//! });
//!
//! // 2. ネイティブハンドルを取得
//! let native = unsafe { get_native_texture_handle(&device, &texture) };
//!
//! // 3. VideoSession を開く
//! let output = OutputTarget {
//!     native_handle: native,
//!     format: PixelFormat::Rgba8Srgb,
//!     width: 1920,
//!     height: 1080,
//!     color_space: ColorSpace::Srgb,
//! };
//! let config = SessionConfig::default();
//! let mut session = open("video.mp4", output, config)?;
//!
//! // 4. 毎フレーム: デコードして GPU テクスチャに書き込み
//! match session.decode_frame(dt)? {
//!     FrameStatus::NewFrame => { /* texture が更新された、描画する */ }
//!     FrameStatus::Waiting  => { /* 前フレーム維持 */ }
//!     FrameStatus::EndOfStream => { /* 再生完了 */ }
//! }
//! ```

use std::time::Duration;

/// ──────────────────────────────────────────────
/// エラー型
/// ──────────────────────────────────────────────
#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    #[error("unsupported codec: {0}")]
    UnsupportedCodec(String),

    #[error("no compatible HW decoder found, falling back to software")]
    NoHwDecoder,

    #[error("demux error: {0}")]
    Demux(String),

    #[error("decode error: {0}")]
    Decode(String),

    #[error("GPU interop error: {0}")]
    GpuInterop(String),

    #[error("seek error: {0}")]
    Seek(String),

    #[error("output target format mismatch: expected {expected}, got {actual}")]
    FormatMismatch { expected: String, actual: String },

    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, VideoError>;

/// ──────────────────────────────────────────────
/// コーデック
/// ──────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
    H265,
    Vp9,
    Av1,
}

/// ──────────────────────────────────────────────
/// ピクセルフォーマット (出力テクスチャのフォーマット)
/// ──────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8Srgb,
    Rgba8Unorm,
    Bgra8Srgb,
    Bgra8Unorm,
}

/// ──────────────────────────────────────────────
/// 色空間
/// ──────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    /// BT.601 (SD)
    Bt601,
    /// BT.709 (HD) — デフォルト
    Bt709,
    /// sRGB (ウェブ標準)
    Srgb,
}

impl Default for ColorSpace {
    fn default() -> Self {
        Self::Bt709
    }
}

/// ──────────────────────────────────────────────
/// GPU ネイティブテクスチャハンドル
/// ──────────────────────────────────────────────
///
/// アプリケーションが wgpu::Texture から抽出した
/// バックエンド固有のポインタ。
///
/// # Safety
/// ポインタは VideoSession の生存期間中有効でなければならない。
/// VideoSession::drop() より先にテクスチャを破棄してはならない。
#[derive(Debug, Clone, Copy)]
pub enum NativeHandle {
    /// macOS / iOS: `id<MTLTexture>` (Objective-C オブジェクトポインタ)
    Metal {
        /// `id<MTLTexture>` as `*mut c_void`
        texture: *mut std::ffi::c_void,
        /// `id<MTLDevice>` as `*mut c_void`
        device: *mut std::ffi::c_void,
    },

    /// Windows (D3D12 Video — 優先): `ID3D12Resource*`
    D3d12 {
        /// `ID3D12Resource*` (output texture) as `*mut c_void`
        texture: *mut std::ffi::c_void,
        /// `ID3D12Device*` as `*mut c_void`
        device: *mut std::ffi::c_void,
        /// `ID3D12CommandQueue*` (DIRECT or VIDEO_DECODE) as `*mut c_void`
        command_queue: *mut std::ffi::c_void,
    },

    /// Windows (Media Foundation フォールバック): `ID3D11Texture2D*`
    D3d11 {
        /// `ID3D11Texture2D*` as `*mut c_void`
        texture: *mut std::ffi::c_void,
        /// `ID3D11Device*` as `*mut c_void`
        device: *mut std::ffi::c_void,
    },

    /// Linux / Android (Vulkan backend):
    /// VkImage + VkDevice (アプリの wgpu Vulkan デバイス)
    Vulkan {
        /// `VkImage` (u64)
        image: u64,
        /// `VkDevice` as `*mut c_void`
        device: *mut std::ffi::c_void,
        /// `VkPhysicalDevice` as `*mut c_void`
        physical_device: *mut std::ffi::c_void,
        /// `VkInstance` as `*mut c_void`
        instance: *mut std::ffi::c_void,
        /// `VkQueue` (video or graphics) as `*mut c_void`
        queue: *mut std::ffi::c_void,
        /// Queue family index
        queue_family_index: u32,
    },

    /// CPU フォールバック: wgpu::Queue を使って write_texture する
    Wgpu {
        /// wgpu は Send + Sync なので Arc<Queue> のポインタ
        /// 実際は `&wgpu::Queue` を transmute
        queue: *const std::ffi::c_void,
        /// wgpu::Texture の raw ID (wgpu-core の texture ID)
        texture_id: u64,
    },
}

// Safety: ネイティブハンドルは GPU リソースへの参照であり、
// GPU API 側でスレッドセーフ性が保証されている。
unsafe impl Send for NativeHandle {}
unsafe impl Sync for NativeHandle {}

/// ──────────────────────────────────────────────
/// 出力先テクスチャ情報
/// ──────────────────────────────────────────────
#[derive(Debug, Clone, Copy)]
pub struct OutputTarget {
    /// GPU ネイティブテクスチャハンドル
    pub native_handle: NativeHandle,
    /// テクスチャのピクセルフォーマット
    pub format: PixelFormat,
    /// テクスチャの幅 (ピクセル)
    pub width: u32,
    /// テクスチャの高さ (ピクセル)
    pub height: u32,
    /// 出力色空間
    pub color_space: ColorSpace,
}

/// ──────────────────────────────────────────────
/// セッション設定
/// ──────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// ループ再生するか
    pub looping: bool,
    /// バックエンドを強制指定 (None = 自動選択)
    pub preferred_backend: Option<Backend>,
    /// ソフトウェアフォールバックを許可するか
    pub allow_software_fallback: bool,
    /// デコードバッファサイズ (フレーム数)
    pub decode_buffer_size: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            looping: true,
            preferred_backend: None,
            allow_software_fallback: true,
            decode_buffer_size: 4,
        }
    }
}

/// ──────────────────────────────────────────────
/// バックエンド種別
/// ──────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// macOS / iOS: VideoToolbox via AVFoundation
    VideoToolbox,
    /// Windows: D3D12 Video API (優先)
    D3d12Video,
    /// Windows: Media Foundation (フォールバック — HW decode)
    MediaFoundation,
    /// Linux: Vulkan Video Extensions
    VulkanVideo,
    /// Linux: GStreamer + VA-API
    GStreamerVaapi,
    /// Linux: V4L2 Stateless
    V4l2,
    /// Android: MediaCodec
    MediaCodec,
    /// 全プラットフォーム: CPU ソフトウェアデコード
    Software,
}

/// ──────────────────────────────────────────────
/// 動画メタデータ
/// ──────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct VideoInfo {
    /// コーデック
    pub codec: Codec,
    /// 幅 (ピクセル)
    pub width: u32,
    /// 高さ (ピクセル)
    pub height: u32,
    /// 総再生時間
    pub duration: Duration,
    /// フレームレート (fps)
    pub fps: f64,
    /// 使用中のバックエンド
    pub backend: Backend,
    /// 色変換が必要か (NV12→RGBA)
    pub needs_color_conversion: bool,
}

/// ──────────────────────────────────────────────
/// フレームデコード結果
/// ──────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStatus {
    /// 新しいフレームがテクスチャに書き込まれた
    NewFrame,
    /// タイミング的にまだ次フレームではない (前フレーム維持)
    Waiting,
    /// ストリーム終端に到達 (looping=false の場合)
    EndOfStream,
}

/// ──────────────────────────────────────────────
/// VideoSession — メイン trait
/// ──────────────────────────────────────────────
///
/// 1つの動画ファイル ↔ 1つの出力テクスチャ のセッション。
///
/// # ライフサイクル
/// 1. `open()` でセッション作成
/// 2. 毎フレーム `decode_frame(dt)` 呼び出し
/// 3. `FrameStatus::NewFrame` が返ったら、テクスチャは更新済み
/// 4. Drop で全リソース解放
pub trait VideoSession: Send {
    /// 動画メタデータを取得
    fn info(&self) -> &VideoInfo;

    /// 現在の再生位置
    fn position(&self) -> Duration;

    /// 次のフレームをデコードして出力テクスチャに書き込む。
    ///
    /// # 引数
    /// - `dt`: 前回呼び出しからの経過時間
    ///
    /// # 戻り値
    /// - `NewFrame`: テクスチャが更新された
    /// - `Waiting`: まだ次フレームのタイミングではない
    /// - `EndOfStream`: 再生完了 (looping=false の場合のみ)
    fn decode_frame(&mut self, dt: Duration) -> Result<FrameStatus>;

    /// 指定時間にシーク
    fn seek(&mut self, position: Duration) -> Result<()>;

    /// ループ再生の on/off
    fn set_looping(&mut self, looping: bool);

    /// ループ再生中か
    fn is_looping(&self) -> bool;

    /// 一時停止
    fn pause(&mut self);

    /// 再開
    fn resume(&mut self);

    /// 一時停止中か
    fn is_paused(&self) -> bool;

    /// 使用中バックエンドを取得
    fn backend(&self) -> Backend;
}

/// ──────────────────────────────────────────────
/// エントリポイント
/// ──────────────────────────────────────────────
///
/// 動画ファイルを開き、最適なバックエンドを自動選択して
/// VideoSession を返す。
///
/// # バックエンド選択順序
///
/// ## macOS / iOS
/// 1. VideoToolbox (常に利用可能)
///
/// ## Windows
/// 1. D3D12 Video API (ランタイム検出: ID3D12VideoDevice)
/// 2. Media Foundation — HW decode (フォールバック、D3D11→D3D12 interop)
///
/// ## Linux
/// 1. Vulkan Video (ランタイム検出: VkQueueVideoDecodeKHR)
/// 2. GStreamer + VA-API (`feature = "gstreamer"` 有効時)
/// 3. V4L2 Stateless (`feature = "v4l2"` 有効時)
///
/// ## Android
/// 1. MediaCodec (常に利用可能)
///
/// ## 全プラットフォーム共通
/// - 上記全て失敗 + `allow_software_fallback=true` → Software
/// - `allow_software_fallback=false` → Err(VideoError::NoHwDecoder)
pub fn open(
    path: &str,
    output: OutputTarget,
    config: SessionConfig,
) -> Result<Box<dyn VideoSession>> {
    backend::create_session(path, output, config)
}
```

### 6.2 demux/mod.rs — Demuxer trait

```rust
use std::time::Duration;
use crate::{Codec, Result};

/// demux 済みのビデオパケット
pub struct VideoPacket {
    /// NAL unit データ (Annex B format: 00 00 00 01 + NAL)
    pub data: Vec<u8>,
    /// Presentation Timestamp
    pub pts: Duration,
    /// Decode Timestamp (B フレーム時に PTS と異なる)
    pub dts: Duration,
    /// キーフレームか
    pub is_keyframe: bool,
}

/// コーデック固有のパラメータセット
pub struct CodecParameters {
    pub codec: Codec,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: Duration,
    /// H.264: SPS + PPS bytes
    /// H.265: VPS + SPS + PPS bytes
    /// VP9/AV1: codec-specific header
    pub extra_data: Vec<u8>,
}

/// コンテナ demuxer trait
pub trait Demuxer: Send {
    /// ビデオトラックのパラメータを取得
    fn parameters(&self) -> &CodecParameters;

    /// 次のビデオパケットを読み取る
    /// None = EOF
    fn next_packet(&mut self) -> Result<Option<VideoPacket>>;

    /// 指定時間にシーク (最寄りのキーフレームに移動)
    fn seek(&mut self, position: Duration) -> Result<()>;
}

/// ファイルパスから Demuxer を作成
/// 拡張子で MP4/WebM を判定
pub fn create_demuxer(path: &str) -> Result<Box<dyn Demuxer>> {
    // ...
}
```

### 6.3 backend/mod.rs — バックエンド選択

```rust
use crate::{OutputTarget, SessionConfig, VideoSession, VideoError, Result, Backend, NativeHandle};

/// プラットフォーム + ランタイム検出でバックエンド自動選択
pub fn create_session(
    path: &str,
    output: OutputTarget,
    config: SessionConfig,
) -> Result<Box<dyn VideoSession>> {
    // preferred_backend が指定されていればそれを試行
    if let Some(backend) = config.preferred_backend {
        return create_with_backend(path, output, &config, backend);
    }

    // 自動選択
    let candidates = detect_backends(&output.native_handle);

    for backend in candidates {
        match create_with_backend(path, output, &config, backend) {
            Ok(session) => return Ok(session),
            Err(e) => {
                log::warn!("Backend {:?} failed: {}, trying next", backend, e);
                continue;
            }
        }
    }

    if config.allow_software_fallback {
        log::warn!("All HW backends failed, using software fallback");
        create_with_backend(path, output, &config, Backend::Software)
    } else {
        Err(VideoError::NoHwDecoder)
    }
}

/// NativeHandle の種別からプラットフォームを判定し、候補バックエンドを返す
fn detect_backends(handle: &NativeHandle) -> Vec<Backend> {
    match handle {
        // macOS / iOS
        NativeHandle::Metal { .. } => vec![Backend::VideoToolbox],

        // Windows (D3D12 — 優先)
        NativeHandle::D3d12 { device, .. } => {
            let mut backends = Vec::new();
            // D3D12 Video API ランタイム検出
            // ID3D12Device::QueryInterface(IID_ID3D12VideoDevice) 成功チェック
            if d3d12_video::is_supported(*device) {
                backends.push(Backend::D3d12Video);
            }
            // Media Foundation フォールバック (HW decode、D3D11 経由)
            backends.push(Backend::MediaFoundation);
            backends
        }

        // Windows (D3D11 レガシー — MF のみ)
        NativeHandle::D3d11 { .. } => vec![Backend::MediaFoundation],

        // Linux / Android
        NativeHandle::Vulkan { instance, physical_device, .. } => {
            let mut backends = Vec::new();

            // Linux: Vulkan Video ランタイム検出
            #[cfg(target_os = "linux")]
            {
                if vulkan_video::is_supported(*instance, *physical_device) {
                    backends.push(Backend::VulkanVideo);
                }

                #[cfg(feature = "gstreamer")]
                backends.push(Backend::GStreamerVaapi);

                #[cfg(feature = "v4l2")]
                backends.push(Backend::V4l2);
            }

            // Android
            #[cfg(target_os = "android")]
            backends.push(Backend::MediaCodec);

            backends
        }

        // CPU fallback
        NativeHandle::Wgpu { .. } => vec![Backend::Software],
    }
}

fn create_with_backend(
    path: &str,
    output: OutputTarget,
    config: &SessionConfig,
    backend: Backend,
) -> Result<Box<dyn VideoSession>> {
    match backend {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        Backend::VideoToolbox => {
            Ok(Box::new(apple::AppleVideoSession::new(path, output, config)?))
        }

        #[cfg(target_os = "windows")]
        Backend::D3d12Video => {
            Ok(Box::new(d3d12_video::D3d12VideoSession::new(path, output, config)?))
        }

        #[cfg(target_os = "windows")]
        Backend::MediaFoundation => {
            Ok(Box::new(media_foundation::MfVideoSession::new(path, output, config)?))
        }

        #[cfg(target_os = "linux")]
        Backend::VulkanVideo => {
            Ok(Box::new(vulkan_video::VkVideoSession::new(path, output, config)?))
        }

        #[cfg(all(target_os = "linux", feature = "gstreamer"))]
        Backend::GStreamerVaapi => {
            Ok(Box::new(gst_vaapi::GstVideoSession::new(path, output, config)?))
        }

        #[cfg(all(target_os = "linux", feature = "v4l2"))]
        Backend::V4l2 => {
            Ok(Box::new(v4l2::V4l2VideoSession::new(path, output, config)?))
        }

        #[cfg(target_os = "android")]
        Backend::MediaCodec => {
            Ok(Box::new(media_codec::McVideoSession::new(path, output, config)?))
        }

        Backend::Software => {
            Ok(Box::new(software::SwVideoSession::new(path, output, config)?))
        }

        _ => Err(VideoError::UnsupportedCodec(
            format!("Backend {:?} not available on this platform", backend)
        )),
    }
}
```

### 6.4 convert/mod.rs — NV12 色変換パス

```rust
/// NV12 → RGBA GPU 色変換パイプライン
///
/// macOS/iOS では不要 (BGRA 出力)。
/// Windows / Linux / Android で HW デコーダが NV12 出力の場合に使用。
///
/// # GPU リソース構成
/// - Y plane テクスチャ: R8Unorm, width × height
/// - UV plane テクスチャ: RG8Unorm, width/2 × height/2
/// - 出力テクスチャ: Rgba8Unorm, width × height (= OutputTarget)
/// - コンピュートパイプライン: @workgroup_size(8, 8)
///
/// # 使い方
/// ```rust,no_run
/// let pass = NV12ToRgbaPass::new(device, color_space, width, height);
/// pass.convert(encoder, y_view, uv_view, output_view);
/// ```
pub struct NV12ToRgbaPass {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    color_space: ColorSpace,
    width: u32,
    height: u32,
}

impl NV12ToRgbaPass {
    pub fn new(
        device: &wgpu::Device,
        color_space: ColorSpace,
        width: u32,
        height: u32,
    ) -> Self { /* ... */ }

    /// NV12 → RGBA 変換をコマンドエンコーダに記録
    pub fn convert(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        y_view: &wgpu::TextureView,
        uv_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
    ) { /* ... */ }
}
```

---

## 7. Cargo.toml

```toml
[package]
name = "video-decoder"
version = "0.1.0"
edition = "2021"
description = "Platform-native HW video decoder with zero-copy GPU texture output"
license = "MIT OR Apache-2.0"
categories = ["multimedia::video", "rendering::engine"]
keywords = ["video", "decoder", "wgpu", "gpu", "hardware-acceleration"]

[dependencies]
# エラー / ログ
anyhow = "1.0"
thiserror = "2.0"
log = "0.4"

# demux (pure Rust, 全プラットフォーム)
mp4parse = "0.17"
h264-reader = "0.7"

# GPU 色変換 (wgpu は全プラットフォームで使用)
wgpu = { version = "24.0", features = ["expose-ids"] }

# ──────────────────────────
# macOS / iOS
# ──────────────────────────
[target.'cfg(any(target_os = "macos", target_os = "ios"))'.dependencies]
objc2 = "0.6"
objc2-foundation = { version = "0.3", features = ["NSString", "NSURL", "NSError"] }
objc2-av-foundation = { version = "0.3", features = [
    "AVAssetReader",
    "AVAssetReaderOutput",
    "AVAssetReaderTrackOutput",
    "AVAsset",
    "AVURLAsset",
    "AVAssetTrack",
    "AVMediaFormat",
] }
objc2-core-media = { version = "0.3", features = [
    "CMSampleBuffer",
    "CMTime",
    "CMFormatDescription",
] }
objc2-core-video = { version = "0.3", features = [
    "CVPixelBuffer",
    "CVMetalTextureCache",
    "CVMetalTexture",
    "CVImageBuffer",
    "CVBuffer",
] }
objc2-metal = { version = "0.3", features = [
    "MTLTexture",
    "MTLDevice",
    "MTLCommandQueue",
    "MTLCommandBuffer",
    "MTLBlitCommandEncoder",
] }

# ──────────────────────────
# Windows
# ──────────────────────────
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    # D3D12 Video API (優先パス)
    "Win32_Graphics_Direct3D12",
    "Win32_Graphics_Direct3D12_Video",   # ID3D12VideoDevice, ID3D12VideoDecoder, etc.
    # Media Foundation (フォールバック)
    "Win32_Media_MediaFoundation",
    "Win32_System_Com",
    # D3D11 interop (MF フォールバック時)
    "Win32_Graphics_Direct3D11",
    "Win32_Graphics_Dxgi",
    "Win32_Graphics_Dxgi_Common",
] }

# demux (D3D12 Video バックエンド用 — Vulkan Video と共有)
mp4parse = "0.17"
h264-reader = "0.7"

# ──────────────────────────
# Linux
# ──────────────────────────
[target.'cfg(target_os = "linux")'.dependencies]
# Vulkan Video (優先パス — 常にコンパイル)
ash = { version = "0.38", default-features = false, features = ["std", "debug"] }

# GStreamer + VA-API (optional フォールバック)
gstreamer = { version = "0.23", optional = true }
gstreamer-app = { version = "0.23", optional = true }
gstreamer-video = { version = "0.23", optional = true }
gstreamer-allocators = { version = "0.23", optional = true }

# V4L2 Stateless (optional SBC 向け)
nix = { version = "0.29", features = ["ioctl", "mman", "fs"], optional = true }

# ──────────────────────────
# Android
# ──────────────────────────
[target.'cfg(target_os = "android")'.dependencies]
ndk = { version = "0.9", features = ["media", "hardware_buffer"] }
ash = { version = "0.38", default-features = false, features = ["std"] }

[features]
default = []
gstreamer = [
    "dep:gstreamer",
    "dep:gstreamer-app",
    "dep:gstreamer-video",
    "dep:gstreamer-allocators",
]
v4l2 = ["dep:nix"]

[dev-dependencies]
pollster = "0.4"
env_logger = "0.11"
image = "0.25"
wgpu = { version = "24.0", features = ["expose-ids"] }
```

---

## 8. 各バックエンド実装仕様

### 8.1 apple.rs — AppleVideoSession

**状態管理:**
```rust
pub struct AppleVideoSession {
    /// AVAssetReader (seek 時は再作成)
    reader: Retained<AVAssetReader>,
    /// AVAssetReaderTrackOutput
    track_output: Retained<AVAssetReaderTrackOutput>,
    /// CVMetalTextureCache (セッション生存期間中保持)
    texture_cache: CVMetalTextureCacheRef,
    /// 出力先 MTLTexture ポインタ (アプリ所有)
    output_mtl_texture: *mut objc2_metal::MTLTexture,
    /// Metal コマンドキュー (blit 用)
    command_queue: Retained<objc2_metal::MTLCommandQueue>,
    /// 再生状態
    playback: PlaybackState,
    /// メタデータ
    info: VideoInfo,
}
```

**decode_frame 実装方針:**
1. `track_output.copyNextSampleBuffer()` で CMSampleBuffer 取得
2. `CMSampleBufferGetImageBuffer()` で CVPixelBuffer 取得
3. `CVMetalTextureCacheCreateTextureFromImage()` で一時 MTLTexture にマップ
4. Metal blit コマンドで一時 MTLTexture → output_mtl_texture にコピー
5. コマンドバッファ commit (GPU 側で非同期実行)

**seek 実装方針:**
- AVAssetReader は seek 不可。新しい AVAssetReader を `timeRange` 指定で再作成
- texture_cache は再利用可能

### 8.2 d3d12_video.rs — D3d12VideoSession (Windows 優先パス)

**設計思想:**
D3D12 Video API は Vulkan Video と同じ低レベル設計（アプリ側 DPB 管理、コマンドリスト）。
demux / NAL パース / DPB 管理ロジックを Vulkan Video と共有できる。
D3D12 内で完結するため D3D11→D3D12 interop が不要。

**フロー:**
```
MP4 file
  → mp4parse (Rust) で demux → H.264 NAL units
    → h264-reader で SPS/PPS/Slice パース
      → ID3D12VideoDevice::CreateVideoDecoder
        → ID3D12VideoDecoderHeap (DPB)
          → ID3D12VideoDecodeCommandList::DecodeFrame
            → D3D12 Texture (NV12)
              → NV12→RGBA (WGSL compute or D3D12 Video Processor)
                → OutputTarget の D3D12 Texture (wgpu 所有)
```

**状態管理:**
```rust
pub struct D3d12VideoSession {
    /// demux + NAL parse (Vulkan Video と共通)
    demuxer: Box<dyn Demuxer>,
    /// ID3D12VideoDevice (D3D12 Video API エントリ)
    video_device: ID3D12VideoDevice,
    /// ID3D12VideoDecoder (デコードセッション状態)
    decoder: ID3D12VideoDecoder,
    /// ID3D12VideoDecoderHeap (解像度依存リソース + DPB ドライバ側状態)
    decoder_heap: ID3D12VideoDecoderHeap,
    /// DPB 管理 (参照フレーム用 D3D12 テクスチャ配列)
    dpb: Vec<D3d12DpbSlot>,
    /// デコード出力テクスチャ (NV12, D3D12)
    decode_output: ID3D12Resource,
    /// NV12→RGBA 変換パス (WGSL compute shader)
    nv12_pass: NV12ToRgbaPass,
    /// D3D12 コマンドアロケータ + コマンドリスト
    command_allocator: ID3D12CommandAllocator,
    command_list: ID3D12VideoDecodeCommandList,
    command_queue: ID3D12CommandQueue,
    /// D3D12 フェンス (GPU 同期)
    fence: ID3D12Fence,
    fence_value: u64,
    /// 再生状態
    playback: PlaybackState,
    info: VideoInfo,
}
```

**DPB スロット:**
```rust
struct D3d12DpbSlot {
    texture: ID3D12Resource,    // D3D12 テクスチャ (NV12)
    /// Picture Order Count
    poc: i32,
    in_use: bool,
}
```

**ランタイム検出:**
```rust
fn is_supported(device: *mut c_void) -> bool {
    // ID3D12Device::QueryInterface(IID_ID3D12VideoDevice) が成功するか
    // + CheckFeatureSupport(D3D12_FEATURE_VIDEO_DECODE_SUPPORT) で
    //   H.264 デコード対応を確認
    unsafe {
        let d3d12_device = device as *mut ID3D12Device;
        let hr = (*d3d12_device).QueryInterface(
            &IID_ID3D12VideoDevice,
            &mut video_device as *mut _ as *mut *mut c_void,
        );
        if hr.is_err() { return false; }

        let mut support = D3D12_FEATURE_DATA_VIDEO_DECODE_SUPPORT {
            Configuration: D3D12_VIDEO_DECODE_CONFIGURATION {
                DecodeProfile: D3D12_VIDEO_DECODE_PROFILE_H264,
                ..Default::default()
            },
            Width: 1920,
            Height: 1080,
            ..Default::default()
        };
        let hr = video_device.CheckFeatureSupport(
            D3D12_FEATURE_VIDEO_DECODE_SUPPORT,
            &mut support as *mut _ as *mut c_void,
            std::mem::size_of_val(&support) as u32,
        );
        hr.is_ok() && support.SupportFlags != D3D12_VIDEO_DECODE_SUPPORT_FLAG_NONE
    }
}
```

**decode_frame 実装方針:**
1. `demuxer.next_packet()` で NAL unit 取得 (Vulkan Video と共通ロジック)
2. h264-reader で Slice Header パース → 参照フレームリスト構築 (Vulkan Video と共通)
3. `command_list.DecodeFrame()` でデコード
   - 入力: `D3D12_VIDEO_DECODE_INPUT_STREAM_ARGUMENTS` (NAL data + 参照フレーム)
   - 出力: `D3D12_VIDEO_DECODE_OUTPUT_STREAM_ARGUMENTS` (decode_output テクスチャ, NV12)
4. `nv12_pass.convert()` で NV12 → RGBA (→ OutputTarget の D3D12 テクスチャ)
5. `command_queue.ExecuteCommandLists()` + fence signal で GPU 送信

**seek 実装方針:**
- `demuxer.seek()` で最寄りキーフレームに移動
- DPB リセット (全スロット解放)
- Decoder は再作成不要 (DecoderHeap は解像度変更時のみ再作成)

**Vulkan Video とのコード共有:**
- `demux/mp4.rs` — MP4 demux (共通)
- `nal/h264.rs` — SPS/PPS/Slice Header パース (共通)
- `util/ring_buffer.rs` — DPB スロット管理ロジック (POC ベース参照管理は抽象化可能)
- `convert/mod.rs` — NV12→RGBA WGSL compute shader (共通)

**D3D12 Video API のティア:**
- Tier 1: アプリが参照フレームを個別テクスチャで管理 → 本実装はこれを使用
- Tier 2: テクスチャ配列 + サブリソース → パフォーマンス向上、将来対応

### 8.3 media_foundation.rs — MfVideoSession (Windows フォールバック)

**使用条件:**
- D3D12 Video API が利用不可の場合 (古い GPU / ドライバ / Windows バージョン)
- `SessionConfig.preferred_backend = Some(Backend::MediaFoundation)` 指定時

**設計思想:**
MF は demux + decode を一括管理する高レベル API。
D3D12 Video と異なり DPB 管理は MF 内部で行われる。
出力は D3D11 テクスチャのため、D3D11→D3D12 interop が必要。

**状態管理:**
```rust
pub struct MfVideoSession {
    /// IMFSourceReader (D3D11 デバイスマネージャ付き、HW decode 有効)
    reader: IMFSourceReader,
    /// MF が使用する D3D11 デバイス
    d3d11_device: ID3D11Device,
    /// MF デコード出力テクスチャ (D3D11, NV12 or RGB32)
    /// → DXGI SharedHandle で D3D12 にアクセス可能にする
    staging_texture: ID3D11Texture2D,
    /// D3D12 側の shared テクスチャ (wgpu OutputTarget に blit する元)
    shared_d3d12_texture: ID3D12Resource,
    /// NV12→RGBA 変換 (MF Video Processor MFT or WGSL compute)
    nv12_pass: Option<NV12ToRgbaPass>,
    /// 再生状態
    playback: PlaybackState,
    info: VideoInfo,
}
```

**decode_frame 実装方針:**
1. `reader.ReadSample()` で IMFSample 取得 (MF が HW decode を内部実行)
2. `IMFDXGIBuffer::GetResource()` で ID3D11Texture2D 取得
3. MF の出力が NV12 の場合:
   - MF Video Processor MFT で NV12→RGBA 変換
   - または D3D11 テクスチャを DXGI SharedHandle 経由で D3D12 にインポート後 WGSL compute
4. MF の出力が RGB32 の場合:
   - `SetGUID(MF_MT_SUBTYPE, MFVideoFormat_RGB32)` で MF に変換させる (GPU 内変換)
   - D3D11 CopyResource → DXGI SharedHandle → D3D12

**D3D11→D3D12 interop:**
```rust
// D3D11 テクスチャを DXGI SharedHandle で D3D12 にインポート
let dxgi_resource: IDXGIResource1 = staging_texture.cast()?;
let shared_handle = dxgi_resource.CreateSharedHandle(
    None,                              // security attributes
    DXGI_SHARED_RESOURCE_READ,         // access
    None,                              // name
)?;
let d3d12_resource = d3d12_device.OpenSharedHandle(
    shared_handle,
    &IID_ID3D12Resource,
)?;
```

**MF HW decode の確認:**
- `MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS = 1` で HW decode を要求
- `MF_SOURCE_READER_D3D_MANAGER` で DXGI デバイスマネージャを設定
- MF が HW decode 不可の場合でも SW decode にフォールバック (MF 内部で自動)

**seek 実装方針:**
- `reader.SetCurrentPosition(MF_PROPERTY_TYPE_VT_I8, position)` で seek

### 8.4 vulkan_video.rs — VkVideoSession

**状態管理:**
```rust
pub struct VkVideoSession {
    /// demux + NAL parse (mp4parse + h264-reader)
    demuxer: Box<dyn Demuxer>,
    /// Vulkan Video Session
    video_session: vk::VideoSessionKHR,
    /// Video Session Parameters (SPS/PPS)
    session_params: vk::VideoSessionParametersKHR,
    /// DPB (Decoded Picture Buffer) スロット配列
    dpb: Vec<DpbSlot>,
    /// デコード結果 VkImage (NV12)
    decode_output: vk::Image,
    /// NV12→RGBA 変換パス
    nv12_pass: NV12ToRgbaPass,
    /// Vulkan ハンドル群
    vk_device: ash::Device,
    video_queue: vk::Queue,
    command_pool: vk::CommandPool,
    /// 再生状態
    playback: PlaybackState,
    info: VideoInfo,
}
```

**DPB (Decoded Picture Buffer) 管理:**
```rust
struct DpbSlot {
    image: vk::Image,
    image_view: vk::ImageView,
    memory: vk::DeviceMemory,
    /// Picture Order Count (H.264 の参照フレーム管理)
    poc: i32,
    /// 使用中フラグ
    in_use: bool,
}
```

- H.264 Level 4.1 (1080p) → 最大 DPB スロット数: 17
- デコード時に `VkVideoDecodeInfoKHR.pReferenceSlots` で参照フレームを指定
- h264-reader で Slice Header から POC / reference list を抽出して DPB 管理

**decode_frame 実装方針:**
1. `demuxer.next_packet()` で NAL unit 取得
2. h264-reader で Slice Header パース → 参照フレームリスト構築
3. `vkCmdDecodeVideoKHR()` でデコード (→ decode_output VkImage, NV12)
4. `nv12_pass.convert()` で NV12 → RGBA (→ OutputTarget の VkImage)

### 8.5 gst_vaapi.rs — GstVideoSession

**状態管理:**
```rust
#[cfg(feature = "gstreamer")]
pub struct GstVideoSession {
    pipeline: gst::Pipeline,
    appsink: gst_app::AppSink,
    /// DMA-BUF → Vulkan import 用リソース
    vk_device: ash::Device,
    /// NV12→RGBA 変換パス
    nv12_pass: NV12ToRgbaPass,
    /// 再生状態
    playback: PlaybackState,
    info: VideoInfo,
}
```

**GStreamer パイプライン構成:**
```
filesrc location={path}
  ! decodebin3
  ! video/x-raw(memory:DMABuf),format=NV12
  ! appsink name=sink sync=false emit-signals=false
```

- `decodebin3` が VA-API 有無を自動判定
- `sync=false` でリアルタイムクロック同期を無効化 (アプリ側で制御)
- DMA-BUF 非対応時は `video/x-raw,format=RGBA` にフォールバック

**decode_frame 実装方針:**
1. `appsink.try_pull_sample(timeout=0)` で非ブロッキング取得
2. DMA-BUF メモリの場合:
   - `gst_allocators::DmaBufMemory::fd()` で fd 取得
   - `VkImportMemoryFdInfoKHR` で一時 VkImage にインポート
   - `nv12_pass.convert()` で NV12 → RGBA
3. 通常メモリの場合:
   - CPU → GPU アップロード (`queue.write_texture()`)

### 8.6 v4l2.rs — V4l2VideoSession

**状態管理:**
```rust
#[cfg(feature = "v4l2")]
pub struct V4l2VideoSession {
    /// V4L2 デバイス fd
    fd: std::os::unix::io::RawFd,
    /// demux + NAL parse (Vulkan Video と共通)
    demuxer: Box<dyn Demuxer>,
    /// OUTPUT キュー (NAL unit を submit する側)
    output_buffers: Vec<V4l2Buffer>,
    /// CAPTURE キュー (デコード済みフレームを受け取る側)
    capture_buffers: Vec<V4l2Buffer>,
    /// NV12→RGBA 変換パス
    nv12_pass: NV12ToRgbaPass,
    /// Vulkan import 用
    vk_device: ash::Device,
    playback: PlaybackState,
    info: VideoInfo,
}
```

**V4L2 Stateless デバイス検出:**
```rust
fn find_v4l2_decoder() -> Option<String> {
    // /dev/video* を走査
    // VIDIOC_QUERYCAP で V4L2_CAP_VIDEO_M2M_MPLANE 確認
    // VIDIOC_ENUM_FMT で V4L2_PIX_FMT_H264_SLICE 対応確認
}
```

### 8.7 media_codec.rs — McVideoSession

**状態管理:**
```rust
pub struct McVideoSession {
    extractor: ndk::media::MediaExtractor,
    codec: ndk::media::MediaCodec,
    /// AHardwareBuffer → Vulkan import 用
    vk_device: ash::Device,
    nv12_pass: NV12ToRgbaPass,
    playback: PlaybackState,
    info: VideoInfo,
}
```

### 8.8 software.rs — SwVideoSession

**状態管理:**
```rust
pub struct SwVideoSession {
    demuxer: Box<dyn Demuxer>,
    /// CPU RGBA バッファ
    frame_buffer: Vec<u8>,
    /// wgpu Queue (write_texture 用)
    queue_ptr: *const std::ffi::c_void,
    playback: PlaybackState,
    info: VideoInfo,
}
```

- CPU でのデコードは `openh264` crate (BSD ライセンス) を使用
- デコード後 RGBA に変換し `queue.write_texture()` で GPU にアップロード

---

## 9. 状態遷移図

```
                    open()
                      │
                      ▼
              ┌───────────────┐
              │   Created     │
              │ (demux 完了,  │
              │  decoder 準備)│
              └───────┬───────┘
                      │ decode_frame()
                      ▼
              ┌───────────────┐
              │   Playing     │◄─────────── resume()
              │               │
              │ decode_frame() │
              │ → NewFrame    │
              │ → Waiting     │
              └───┬───┬───┬───┘
                  │   │   │
          pause() │   │   │ position >= duration
                  ▼   │   ▼
          ┌────────┐  │  ┌──────────────┐
          │ Paused │  │  │ EndOfStream  │
          └────────┘  │  │ (looping=f)  │
                      │  └──────┬───────┘
                      │         │ seek() or
                      │         │ set_looping(true) + decode_frame()
                      │         │
                      │  ┌──────▼───────┐
                      └──┤  Looping     │
                         │ (seek to 0,  │
                         │  continue)   │
                         └──────────────┘
                               │
                               │ (自動的に Playing に戻る)
                               ▼
                         ┌───────────┐
                         │  Playing  │
                         └───────────┘

    ※ どの状態からも Drop → リソース解放
```

---

## 10. スレッドモデル

```
┌─────────────────────────────┐
│  Application Thread (main)  │
│                             │
│  loop {                     │
│    session.decode_frame(dt) │──── ① 同期呼び出し
│    render(texture)          │     (decode は内部で非同期 GPU コマンド)
│  }                          │
└─────────────────────────────┘
              │
              │ ① が内部で発行するもの:
              │
              ├── [macOS] Metal command buffer (GPU 非同期)
              ├── [Win/D3D12] ID3D12VideoDecodeCommandList (GPU 非同期)
              ├── [Win/MF]  IMFSourceReader.ReadSample + D3D11 CopyResource (GPU 非同期)
              ├── [Linux/Vk] vkCmdDecodeVideoKHR + compute dispatch
              ├── [Linux/GSt] GStreamer は内部スレッドでデコード
              │               appsink.try_pull_sample() は non-blocking
              └── [Software] CPU decode は同期 (ブロッキング)
```

- `decode_frame()` は**メインスレッドから同期的に呼ぶ** API
- 内部で GPU コマンドを発行するが、GPU 実行自体は非同期
- GStreamer バックエンドのみ内部スレッドプールを使用 (GStreamer が管理)
- 将来的にデコードを別スレッドに分離する場合は `VideoSession: Send` で対応可能

---

## 11. wgpu からネイティブハンドルを取得する方法

アプリケーション側のヘルパーコード (本クレートには含めず、ドキュメントで提供):

```rust
/// wgpu::Device + wgpu::Texture からプラットフォーム固有のハンドルを取得する。
///
/// # Safety
/// - 返されたハンドルは texture と device が生存している間のみ有効
/// - wgpu の `expose-ids` feature が必要
pub unsafe fn get_native_handle(
    device: &wgpu::Device,
    texture: &wgpu::Texture,
) -> NativeHandle {
    // wgpu::hal API を使用してバックエンド固有のリソースにアクセス
    //
    // Metal:
    //   device.as_hal::<wgpu::hal::api::Metal, _, _>(|d| { d.raw_device() })
    //   texture.as_hal::<wgpu::hal::api::Metal, _, _>(|t| { t.raw_texture() })
    //
    // DX12:
    //   device.as_hal::<wgpu::hal::api::Dx12, _, _>(|d| { d.raw_device() })
    //   texture.as_hal::<wgpu::hal::api::Dx12, _, _>(|t| { t.raw_resource() })
    //
    // Vulkan:
    //   device.as_hal::<wgpu::hal::api::Vulkan, _, _>(|d| { ... })
    //   texture.as_hal::<wgpu::hal::api::Vulkan, _, _>(|t| { t.raw_handle() })
    todo!()
}
```
