# 仮想カメラ・オーディオ設計ドキュメント

## 目的

Google Meet botにおいて、任意の映像・音声コンテンツをリアルタイムで配信できるようにする。

### 背景

現在のmeet-bot-rsでは、Chromeの`--use-fake-device-for-media-stream`フラグにより、デフォルトのテストパターン（緑色の回転映像）が表示される。これは開発・テスト用であり、実用的なユースケースには不十分。

### 解決したい課題

1. **カスタム映像配信**: 任意のVulkan/OpenGLアプリケーションの出力をMeet参加者に配信したい
2. **低レイテンシ**: リアルタイム性が求められるユースケース（ライブデモ、インタラクティブコンテンツ）に対応
3. **音声同期**: 映像と同期したカスタム音声の配信
4. **Docker環境対応**: コンテナ内での仮想デバイス利用

### ユースケース

| ユースケース | 説明 | 要求レイテンシ |
|-------------|------|---------------|
| 3Dアプリデモ | Vulkan/GLアプリの画面をリアルタイム共有 | <50ms |
| バーチャルアバター | リアルタイムレンダリングされたアバター映像 | <30ms |
| ゲーム配信 | ゲーム画面のライブストリーミング | <100ms |
| プレゼンテーション | スライド + 動的コンテンツ | <200ms |
| AI生成コンテンツ | リアルタイムで生成される映像/音声 | 用途による |

### 技術的ゴール

- **映像**: Vulkan/OpenGLレンダリング結果 → Chrome WebRTC → Google Meet
- **音声**: アプリケーション生成音声 → Chrome WebRTC → Google Meet
- **レイテンシ目標**: <30ms (GPU → Meet配信まで)
- **解像度**: 1280x720 @ 30fps (設定可能)

## 概要

Chromeの`--use-fake-device-for-media-stream`で使用されるデフォルトのテストパターンを、仮想カメラデバイス（v4l2loopback）または PipeWire ストリーム経由でカスタム映像に置き換える。

## アーキテクチャ比較

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        レイテンシ比較                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│ 方式                          │ レイテンシ      │ CPU負荷  │ 実装難易度    │
├───────────────────────────────┼─────────────────┼──────────┼───────────────┤
│ 1. DMA-BUF直接共有            │ <5ms            │ 最小     │ 高           │
│ 2. GStreamer + GPU            │ 10-30ms         │ 低       │ 中           │
│ 3. wf-recorder/ffmpeg         │ 30-100ms        │ 中       │ 低           │
│ 4. 直接レンダリング(アプリ実装) │ <5ms            │ 最小     │ 高           │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 方式1: DMA-BUF直接共有 (Zero-Copy)

### 概要
GPUメモリを直接共有し、コピーなしで映像を転送する最高効率の方式。

### アーキテクチャ

```
┌──────────────────┐     DMA-BUF fd      ┌──────────────────┐
│  Vulkan/GL App   │────────────────────▶│    PipeWire      │
│  (レンダリング)   │    (zero-copy)      │  (Virtual Cam)   │
└──────────────────┘                     └────────┬─────────┘
                                                  │
                                                  ▼
                                         ┌──────────────────┐
                                         │ v4l2loopback     │
                                         │ /dev/video10     │
                                         └────────┬─────────┘
                                                  │
                                                  ▼
                                         ┌──────────────────┐
                                         │     Chrome       │
                                         │ (--use-file-for- │
                                         │  video-capture)  │
                                         └──────────────────┘
```

### 実装例 (Rust + ash)

```rust
use ash::vk;

// DMA-BUF exportable memory allocation
fn create_exportable_image(
    device: &ash::Device,
    width: u32,
    height: u32,
) -> (vk::Image, vk::DeviceMemory, i32) {
    // External memory extension
    let external_memory_info = vk::ExternalMemoryImageCreateInfo::builder()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
        .build();

    let image_info = vk::ImageCreateInfo::builder()
        .push_next(&mut external_memory_info)
        .image_type(vk::ImageType::TYPE_2D)
        .format(vk::Format::B8G8R8A8_UNORM)
        .extent(vk::Extent3D { width, height, depth: 1 })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::LINEAR)  // DMA-BUF requires linear
        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
        .build();

    let image = unsafe { device.create_image(&image_info, None).unwrap() };

    // Allocate exportable memory
    let export_info = vk::ExportMemoryAllocateInfo::builder()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
        .build();

    let alloc_info = vk::MemoryAllocateInfo::builder()
        .push_next(&mut export_info)
        .allocation_size(requirements.size)
        .memory_type_index(memory_type_index)
        .build();

    let memory = unsafe { device.allocate_memory(&alloc_info, None).unwrap() };

    // Get DMA-BUF file descriptor
    let fd_info = vk::MemoryGetFdInfoKHR::builder()
        .memory(memory)
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
        .build();

    let fd = unsafe { external_memory_fd.get_memory_fd(&fd_info).unwrap() };

    (image, memory, fd)
}
```

### PipeWire連携

```rust
// libpipewire-rs を使用
use pipewire::{Context, MainLoop, stream::Stream};

fn create_pipewire_video_stream(dma_buf_fd: i32) {
    let mainloop = MainLoop::new().unwrap();
    let context = Context::new(&mainloop).unwrap();
    let core = context.connect(None).unwrap();

    let stream = Stream::new(
        &core,
        "virtual-camera",
        pipewire::properties! {
            *pipewire::keys::MEDIA_TYPE => "Video",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Camera",
        },
    ).unwrap();

    // DMA-BUF buffer setup
    let params = spa::pod::Pod::new_bytes(&[
        spa::format::video::Format::BGRA,
        spa::format::video::size(1280, 720),
        spa::format::video::framerate(30, 1),
    ]);

    stream.connect(
        pipewire::stream::Direction::Output,
        None,
        pipewire::stream::StreamFlags::MAP_BUFFERS
            | pipewire::stream::StreamFlags::ALLOC_BUFFERS,
        &[params],
    ).unwrap();
}
```

### 利点
- 最小レイテンシ (<5ms)
- CPU負荷なし（GPUメモリ直接共有）
- 高解像度でも効率的

### 欠点
- GPU/ドライバー依存（Mesa DRI, NVIDIA要対応）
- 実装が複雑
- Dockerコンテナ内では追加設定が必要

---

## 方式2: GStreamer + GPU

### 概要
GStreamerパイプラインを使用し、GPU accelerated encodingで映像を転送。

### アーキテクチャ

```
┌──────────────────┐                     ┌──────────────────┐
│  Vulkan/GL App   │──▶ GPU Texture ──▶ │   GStreamer      │
│  (レンダリング)   │                     │   Pipeline       │
└──────────────────┘                     └────────┬─────────┘
                                                  │
                           ┌──────────────────────┴──────────┐
                           ▼                                 ▼
                  ┌──────────────────┐            ┌──────────────────┐
                  │   v4l2loopback   │            │    PipeWire      │
                  │   /dev/video10   │            │   (pw-record)    │
                  └────────┬─────────┘            └────────┬─────────┘
                           │                               │
                           └───────────┬───────────────────┘
                                       ▼
                              ┌──────────────────┐
                              │     Chrome       │
                              └──────────────────┘
```

### パイプライン例

```bash
# VAAPI (Intel/AMD GPU)
gst-launch-1.0 \
    ximagesrc display=:99 use-damage=false ! \
    video/x-raw,framerate=30/1 ! \
    vaapipostproc ! \
    vaapih264enc ! \
    h264parse ! \
    avdec_h264 ! \
    videoconvert ! \
    v4l2sink device=/dev/video10

# NVIDIA NVENC
gst-launch-1.0 \
    ximagesrc display=:99 ! \
    video/x-raw,framerate=30/1 ! \
    nvh264enc ! \
    h264parse ! \
    avdec_h264 ! \
    videoconvert ! \
    v4l2sink device=/dev/video10
```

### Rust実装 (gstreamer-rs)

```rust
use gstreamer as gst;
use gst::prelude::*;

fn create_video_pipeline() -> gst::Pipeline {
    gst::init().unwrap();

    let pipeline = gst::Pipeline::new(Some("virtual-cam-pipeline"));

    // Elements
    let src = gst::ElementFactory::make("ximagesrc")
        .property("display-name", ":99")
        .property("use-damage", false)
        .build().unwrap();

    let capsfilter = gst::ElementFactory::make("capsfilter")
        .property("caps", gst::Caps::builder("video/x-raw")
            .field("framerate", gst::Fraction::new(30, 1))
            .build())
        .build().unwrap();

    let convert = gst::ElementFactory::make("videoconvert").build().unwrap();

    let sink = gst::ElementFactory::make("v4l2sink")
        .property("device", "/dev/video10")
        .build().unwrap();

    pipeline.add_many(&[&src, &capsfilter, &convert, &sink]).unwrap();
    gst::Element::link_many(&[&src, &capsfilter, &convert, &sink]).unwrap();

    pipeline
}
```

### 利点
- 柔軟なパイプライン構成
- GPU encoding対応
- 既存ツールとの連携が容易

### 欠点
- GStreamerの依存関係
- 中程度のレイテンシ (10-30ms)
- パイプライン設定の複雑さ

---

## 方式3: wf-recorder / ffmpeg キャプチャ

### 概要
画面キャプチャツールを使用する最もシンプルな方式。

### アーキテクチャ

```
┌──────────────────┐                     ┌──────────────────┐
│  Vulkan/GL App   │                     │   wf-recorder    │
│  (Xvfb :99上で   │──▶ X11/Wayland ──▶ │   or ffmpeg      │
│   レンダリング)   │    Compositor       │                  │
└──────────────────┘                     └────────┬─────────┘
                                                  │
                                                  ▼
                                         ┌──────────────────┐
                                         │   v4l2loopback   │
                                         │   /dev/video10   │
                                         └────────┬─────────┘
                                                  │
                                                  ▼
                                         ┌──────────────────┐
                                         │     Chrome       │
                                         └──────────────────┘
```

### 実装例

```bash
# ffmpeg (X11)
ffmpeg -f x11grab -framerate 30 -video_size 1280x720 -i :99 \
    -f v4l2 -pix_fmt yuv420p /dev/video10

# wf-recorder (Wayland + PipeWire)
wf-recorder -g "0,0 1280x720" -f - | \
    ffmpeg -i - -f v4l2 -pix_fmt yuv420p /dev/video10

# PipeWire direct
pw-record --target=0 - | \
    ffmpeg -i - -f v4l2 -pix_fmt yuv420p /dev/video10
```

### Docker entrypoint.sh への追加

```bash
# v4l2loopback setup (ホスト側で実行)
# sudo modprobe v4l2loopback video_nr=10 card_label="VirtualCam"

# Start screen capture to virtual camera
ffmpeg -f x11grab -framerate 30 -video_size ${SCREEN_WIDTH}x${SCREEN_HEIGHT} \
    -i $DISPLAY -f v4l2 -pix_fmt yuv420p /dev/video10 &
FFMPEG_PID=$!
```

### 利点
- 実装が最も簡単
- 追加の依存関係が少ない
- デバッグが容易

### 欠点
- 最も高いレイテンシ (30-100ms)
- CPU負荷が中程度
- 画面全体のキャプチャのみ（特定ウィンドウの場合は追加設定必要）

---

## 方式4: 直接レンダリング (アプリ側実装) ⭐推奨

### 概要
Vulkan/OpenGLアプリケーション内で直接v4l2loopbackまたはPipeWireに出力する方式。
ユーザーの想定する主要アプローチ。

### アーキテクチャ

```
┌─────────────────────────────────────────────────────────────────┐
│                     Vulkan/GL Application                       │
│  ┌──────────────┐                                               │
│  │ Render Pass  │                                               │
│  │  (Scene)     │                                               │
│  └──────┬───────┘                                               │
│         │                                                       │
│         ▼                                                       │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐    │
│  │ Framebuffer  │────▶│  Readback    │────▶│  Output      │    │
│  │ (GPU Memory) │     │  (Optional)  │     │  Module      │    │
│  └──────────────┘     └──────────────┘     └──────┬───────┘    │
│                                                    │            │
└────────────────────────────────────────────────────┼────────────┘
                                                     │
                    ┌────────────────────────────────┼────────────┐
                    │                                │            │
                    ▼                                ▼            ▼
           ┌──────────────┐               ┌──────────────┐ ┌──────────────┐
           │ v4l2loopback │               │   PipeWire   │ │  DMA-BUF     │
           │ /dev/video10 │               │   Stream     │ │  (方式1へ)   │
           └──────┬───────┘               └──────┬───────┘ └──────────────┘
                  │                              │
                  └──────────────┬───────────────┘
                                 ▼
                        ┌──────────────┐
                        │    Chrome    │
                        │  (WebRTC)    │
                        └──────────────┘
```

### Rust実装 (Vulkan + v4l2)

```rust
use ash::vk;
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use v4l::prelude::*;
use v4l::video::Output;

pub struct VirtualCameraOutput {
    device: Device,
    width: u32,
    height: u32,
    staging_buffer: vk::Buffer,
    staging_memory: vk::DeviceMemory,
}

impl VirtualCameraOutput {
    pub fn new(
        device: &ash::Device,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        width: u32,
        height: u32,
        v4l_device_path: &str,
    ) -> Self {
        // Open v4l2loopback device
        let v4l_device = Device::with_path(v4l_device_path).unwrap();

        // Set format
        let mut fmt = v4l_device.format().unwrap();
        fmt.width = width;
        fmt.height = height;
        fmt.fourcc = v4l::FourCC::new(b"YUYV");
        v4l_device.set_format(&fmt).unwrap();

        // Create staging buffer for GPU->CPU transfer
        let buffer_size = (width * height * 4) as vk::DeviceSize;
        let buffer_info = vk::BufferCreateInfo::builder()
            .size(buffer_size)
            .usage(vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .build();

        let staging_buffer = unsafe { device.create_buffer(&buffer_info, None).unwrap() };

        // Allocate host-visible memory
        let mem_requirements = unsafe { device.get_buffer_memory_requirements(staging_buffer) };
        let mem_properties = unsafe {
            instance.get_physical_device_memory_properties(physical_device)
        };

        let memory_type_index = find_memory_type(
            mem_requirements.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            &mem_properties,
        );

        let alloc_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(mem_requirements.size)
            .memory_type_index(memory_type_index)
            .build();

        let staging_memory = unsafe { device.allocate_memory(&alloc_info, None).unwrap() };
        unsafe { device.bind_buffer_memory(staging_buffer, staging_memory, 0).unwrap() };

        Self {
            device: v4l_device,
            width,
            height,
            staging_buffer,
            staging_memory,
        }
    }

    /// Copy framebuffer to v4l2loopback
    pub fn output_frame(
        &self,
        vk_device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        source_image: vk::Image,
    ) {
        // Copy image to staging buffer
        let region = vk::BufferImageCopy::builder()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D {
                width: self.width,
                height: self.height,
                depth: 1,
            })
            .build();

        unsafe {
            vk_device.cmd_copy_image_to_buffer(
                command_buffer,
                source_image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                self.staging_buffer,
                &[region],
            );
        }

        // Map memory and write to v4l2
        unsafe {
            let ptr = vk_device.map_memory(
                self.staging_memory,
                0,
                vk::WHOLE_SIZE,
                vk::MemoryMapFlags::empty(),
            ).unwrap() as *const u8;

            let data = std::slice::from_raw_parts(
                ptr,
                (self.width * self.height * 4) as usize
            );

            // Convert BGRA to YUYV and write
            let yuyv_data = bgra_to_yuyv(data, self.width, self.height);
            self.device.write(&yuyv_data).unwrap();

            vk_device.unmap_memory(self.staging_memory);
        }
    }
}

/// BGRA to YUYV color conversion
fn bgra_to_yuyv(bgra: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut yuyv = Vec::with_capacity((width * height * 2) as usize);

    for y in 0..height {
        for x in (0..width).step_by(2) {
            let i1 = ((y * width + x) * 4) as usize;
            let i2 = ((y * width + x + 1) * 4) as usize;

            let (b1, g1, r1) = (bgra[i1], bgra[i1 + 1], bgra[i1 + 2]);
            let (b2, g2, r2) = (bgra[i2], bgra[i2 + 1], bgra[i2 + 2]);

            // RGB to YUV conversion
            let y1 = ((66 * r1 as i32 + 129 * g1 as i32 + 25 * b1 as i32 + 128) >> 8) + 16;
            let y2 = ((66 * r2 as i32 + 129 * g2 as i32 + 25 * b2 as i32 + 128) >> 8) + 16;
            let u = ((-38 * r1 as i32 - 74 * g1 as i32 + 112 * b1 as i32 + 128) >> 8) + 128;
            let v = ((112 * r1 as i32 - 94 * g1 as i32 - 18 * b1 as i32 + 128) >> 8) + 128;

            yuyv.push(y1.clamp(0, 255) as u8);
            yuyv.push(u.clamp(0, 255) as u8);
            yuyv.push(y2.clamp(0, 255) as u8);
            yuyv.push(v.clamp(0, 255) as u8);
        }
    }

    yuyv
}
```

### OpenGL版実装

```rust
use gl;
use std::os::raw::c_void;

pub struct GLVirtualCamera {
    pbo: [gl::types::GLuint; 2],  // Double-buffered PBO
    current_pbo: usize,
    width: u32,
    height: u32,
    v4l_device: v4l::Device,
}

impl GLVirtualCamera {
    pub fn new(width: u32, height: u32, device_path: &str) -> Self {
        let mut pbos = [0u32; 2];
        unsafe {
            gl::GenBuffers(2, pbos.as_mut_ptr());

            let buffer_size = (width * height * 4) as isize;
            for pbo in &pbos {
                gl::BindBuffer(gl::PIXEL_PACK_BUFFER, *pbo);
                gl::BufferData(
                    gl::PIXEL_PACK_BUFFER,
                    buffer_size,
                    std::ptr::null(),
                    gl::STREAM_READ,
                );
            }
            gl::BindBuffer(gl::PIXEL_PACK_BUFFER, 0);
        }

        let v4l_device = v4l::Device::with_path(device_path).unwrap();

        Self {
            pbo: pbos,
            current_pbo: 0,
            width,
            height,
            v4l_device,
        }
    }

    pub fn capture_and_output(&mut self, framebuffer: gl::types::GLuint) {
        unsafe {
            // Bind FBO and read pixels to PBO
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, framebuffer);
            gl::BindBuffer(gl::PIXEL_PACK_BUFFER, self.pbo[self.current_pbo]);

            gl::ReadPixels(
                0, 0,
                self.width as i32, self.height as i32,
                gl::BGRA,
                gl::UNSIGNED_BYTE,
                std::ptr::null_mut(),
            );

            // Read from previous PBO (async transfer)
            let prev_pbo = 1 - self.current_pbo;
            gl::BindBuffer(gl::PIXEL_PACK_BUFFER, self.pbo[prev_pbo]);

            let ptr = gl::MapBuffer(gl::PIXEL_PACK_BUFFER, gl::READ_ONLY) as *const u8;
            if !ptr.is_null() {
                let data = std::slice::from_raw_parts(
                    ptr,
                    (self.width * self.height * 4) as usize,
                );

                let yuyv = bgra_to_yuyv(data, self.width, self.height);
                self.v4l_device.write(&yuyv).ok();

                gl::UnmapBuffer(gl::PIXEL_PACK_BUFFER);
            }

            gl::BindBuffer(gl::PIXEL_PACK_BUFFER, 0);
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, 0);

            self.current_pbo = prev_pbo;
        }
    }
}
```

### PipeWire直接出力版

```rust
use pipewire::{
    Context, MainLoop,
    spa::{pod::Pod, utils::dict::DictRef},
    stream::{Stream, StreamFlags},
};

pub struct PipeWireOutput {
    stream: Stream,
    width: u32,
    height: u32,
}

impl PipeWireOutput {
    pub fn new(width: u32, height: u32) -> Self {
        let mainloop = MainLoop::new().expect("Failed to create main loop");
        let context = Context::new(&mainloop).expect("Failed to create context");
        let core = context.connect(None).expect("Failed to connect to PipeWire");

        let stream = Stream::new(
            &core,
            "rust-virtual-camera",
            pipewire::properties! {
                *pipewire::keys::MEDIA_TYPE => "Video",
                *pipewire::keys::MEDIA_CATEGORY => "Capture",
                *pipewire::keys::MEDIA_ROLE => "Camera",
            },
        ).expect("Failed to create stream");

        // Configure video format
        let params = [
            // SPA_FORMAT_VIDEO_format
            Pod::new_object(
                spa::SPA_TYPE_OBJECT_Format,
                spa::SPA_PARAM_EnumFormat,
                &[
                    spa::format::media_type(spa::SPA_MEDIA_TYPE_video),
                    spa::format::media_subtype(spa::SPA_MEDIA_SUBTYPE_raw),
                    spa::format::video_format(spa::SPA_VIDEO_FORMAT_BGRx),
                    spa::format::video_size(width, height),
                    spa::format::video_framerate(30, 1),
                ],
            ),
        ];

        stream.connect(
            pipewire::stream::Direction::Output,
            None,
            StreamFlags::MAP_BUFFERS | StreamFlags::DRIVER,
            &params,
        ).expect("Failed to connect stream");

        Self { stream, width, height }
    }

    pub fn output_frame(&self, bgra_data: &[u8]) {
        if let Some(mut buffer) = self.stream.dequeue_buffer() {
            let datas = buffer.datas_mut();
            if let Some(data) = datas.get_mut(0) {
                let dst = data.data().unwrap();
                dst[..bgra_data.len()].copy_from_slice(bgra_data);
                data.chunk_mut().size = bgra_data.len() as u32;
            }
        }
    }
}
```

### 利点
- 最小レイテンシ（GPU readback次第で<5ms可能）
- 完全な制御が可能
- 他プロセスへの依存なし
- Double-buffered PBOで非同期転送

### 欠点
- アプリケーションへの組み込みが必要
- GPU readbackがボトルネックになる可能性
- プラットフォーム固有のコード

---

## 仮想オーディオ設計

### PipeWire Virtual Sink

```bash
# 仮想Sinkの作成
pactl load-module module-null-sink sink_name=VirtualMic sink_properties=device.description=VirtualMic

# または PipeWire native
pw-cli create-node adapter factory.name=support.null-audio-sink \
    media.class=Audio/Sink \
    node.name=VirtualMic \
    audio.position="FL,FR"
```

### Rust実装

```rust
use pipewire::stream::{Stream, StreamFlags};

pub struct VirtualAudioOutput {
    stream: Stream,
    sample_rate: u32,
    channels: u32,
}

impl VirtualAudioOutput {
    pub fn new(sample_rate: u32, channels: u32) -> Self {
        let mainloop = MainLoop::new().unwrap();
        let context = Context::new(&mainloop).unwrap();
        let core = context.connect(None).unwrap();

        let stream = Stream::new(
            &core,
            "rust-virtual-audio",
            pipewire::properties! {
                *pipewire::keys::MEDIA_TYPE => "Audio",
                *pipewire::keys::MEDIA_CATEGORY => "Capture",
                *pipewire::keys::MEDIA_ROLE => "Communication",
            },
        ).unwrap();

        let params = [
            Pod::new_object(
                spa::SPA_TYPE_OBJECT_Format,
                spa::SPA_PARAM_EnumFormat,
                &[
                    spa::format::media_type(spa::SPA_MEDIA_TYPE_audio),
                    spa::format::media_subtype(spa::SPA_MEDIA_SUBTYPE_raw),
                    spa::format::audio_format(spa::SPA_AUDIO_FORMAT_F32),
                    spa::format::audio_rate(sample_rate),
                    spa::format::audio_channels(channels),
                ],
            ),
        ];

        stream.connect(
            pipewire::stream::Direction::Output,
            None,
            StreamFlags::MAP_BUFFERS | StreamFlags::DRIVER,
            &params,
        ).unwrap();

        Self { stream, sample_rate, channels }
    }

    pub fn write_samples(&self, samples: &[f32]) {
        if let Some(mut buffer) = self.stream.dequeue_buffer() {
            let datas = buffer.datas_mut();
            if let Some(data) = datas.get_mut(0) {
                let dst = data.data().unwrap();
                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        samples.as_ptr() as *const u8,
                        samples.len() * 4,
                    )
                };
                dst[..bytes.len()].copy_from_slice(bytes);
            }
        }
    }
}
```

---

## Docker統合設計

### Dockerfile追加設定

```dockerfile
# v4l2loopback support (kernel module is on host)
# PipeWire support
RUN apt-get update && apt-get install -y \
    pipewire \
    pipewire-audio-client-libraries \
    libpipewire-0.3-dev \
    v4l-utils \
    && rm -rf /var/lib/apt/lists/*

# For GPU access
ENV NVIDIA_VISIBLE_DEVICES=all
ENV NVIDIA_DRIVER_CAPABILITIES=graphics,video,compute
```

### docker-compose.yml

```yaml
services:
  meet-bot-rs:
    # ... existing config ...
    devices:
      - /dev/video10:/dev/video10  # v4l2loopback
      - /dev/dri:/dev/dri          # GPU (DRI)
    volumes:
      - /run/user/1000/pipewire-0:/run/user/1000/pipewire-0  # PipeWire socket
    environment:
      - XDG_RUNTIME_DIR=/run/user/1000
      - PIPEWIRE_RUNTIME_DIR=/run/user/1000
```

### ホスト側セットアップ

```bash
# v4l2loopback kernel module
sudo modprobe v4l2loopback video_nr=10 card_label="VirtualCam" exclusive_caps=1

# Make it persistent
echo "v4l2loopback" | sudo tee /etc/modules-load.d/v4l2loopback.conf
echo "options v4l2loopback video_nr=10 card_label=VirtualCam exclusive_caps=1" | \
    sudo tee /etc/modprobe.d/v4l2loopback.conf
```

---

## 推奨実装順序

1. **Phase 1**: 方式3 (ffmpeg) で概念実証
   - 最も簡単に動作確認可能
   - Dockerfileに ffmpeg + v4l2loopback サポート追加

2. **Phase 2**: 方式4 (直接レンダリング) 基本実装
   - Vulkan/GLアプリからv4l2loopbackへの直接出力
   - staging buffer + readback パイプライン

3. **Phase 3**: 方式1 (DMA-BUF) 最適化
   - zero-copy実装
   - 最小レイテンシ達成

---

## 参考リンク

- [v4l2loopback](https://github.com/umlaeute/v4l2loopback)
- [PipeWire Documentation](https://docs.pipewire.org/)
- [Vulkan External Memory](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VK_KHR_external_memory.html)
- [pipewire-rs](https://gitlab.freedesktop.org/pipewire/pipewire-rs)
- [GStreamer Rust Bindings](https://gitlab.freedesktop.org/gstreamer/gstreamer-rs)
