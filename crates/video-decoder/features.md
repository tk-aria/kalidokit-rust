# video-decoder crate — 実装タスク

> 各 Phase は順番に実装。各 Step 内のチェックボックスを完了順にチェックする。
> 300行以上になるファイルは分割候補として明記。
> `cargo fmt --check` は全 Phase の検証で毎回実行すること。
> 設計書: `docs/design/video-decoder-crate-design.md`, `docs/design/video-decoder-sow.md`

## Phase 依存関係

```
Phase 1 (クレート基盤 + 共通型)
  ↓
Phase 2 (Demux + NAL パーサ) ← Phase 1 に依存
  ↓
Phase 3 (NV12→RGBA 色変換 + PlaybackState) ← Phase 1 に依存
  ↓
Phase 4 (ソフトウェアデコーダ + バックエンド選択) ← Phase 1, 2, 3 に依存
  ↓
Phase 5 (macOS / iOS バックエンド) ← Phase 4 に依存
  ↓
Phase 6 (Windows バックエンド: D3D12 Video + MF) ← Phase 2, 3, 4 に依存
  ↓
Phase 7 (Linux バックエンド: Vulkan Video + GStreamer + V4L2) ← Phase 2, 3, 4 に依存
  ↓
Phase 8 (Android バックエンド) ← Phase 3, 4 に依存
  ↓
Phase 9 (E2E テスト・サンプル・ドキュメント・CI) ← Phase 4 以降随時
```

## ライブラリバージョン一覧

| クレート | バージョン | 用途 |
|---------|-----------|------|
| `wgpu` | 24.0 | GPU 色変換・テクスチャ書き込み (expose-ids feature) |
| `anyhow` | 1.0 | エラーハンドリング |
| `thiserror` | 2.0 | カスタムエラー型 |
| `log` | 0.4 | ログマクロ |
| `mp4parse` | 0.17 | MP4 コンテナ demux (pure Rust, Mozilla 製) |
| `h264-reader` | 0.7 | H.264 NAL unit パーサ (SPS/PPS/Slice Header) |
| `openh264` | 0.6 | CPU ソフトウェア H.264 デコーダ (BSD) |
| `objc2` | 0.6 | Obj-C FFI (macOS/iOS) |
| `objc2-foundation` | 0.3 | Foundation フレームワーク |
| `objc2-av-foundation` | 0.3 | AVFoundation (AVAssetReader 等) |
| `objc2-core-media` | 0.3 | CoreMedia (CMSampleBuffer 等) |
| `objc2-core-video` | 0.3 | CoreVideo (CVPixelBuffer, CVMetalTextureCache) |
| `objc2-metal` | 0.3 | Metal (MTLTexture, MTLCommandQueue) |
| `windows` | 0.58 | Win32 API (D3D12 Video, Media Foundation, DXGI) |
| `ash` | 0.38 | Vulkan FFI (Video Extensions, external memory) |
| `gstreamer` | 0.23 | GStreamer Rust bindings (optional) |
| `gstreamer-app` | 0.23 | AppSink (optional) |
| `gstreamer-video` | 0.23 | Video caps (optional) |
| `gstreamer-allocators` | 0.23 | DmaBufMemory (optional) |
| `nix` | 0.29 | ioctl / mman (V4L2, optional) |
| `ndk` | 0.9 | Android NDK (MediaCodec, AHardwareBuffer) |
| `pollster` | 0.4 | async→sync ブリッジ (dev) |
| `env_logger` | 0.11 | ロギング (dev) |
| `image` | 0.25 | 画像処理 (dev/テスト) |

---

## Phase 1: クレート基盤 + 共通型定義

**目的**: ワークスペースへの追加、クリーンアーキテクチャに基づくモジュール構造の構築、全パブリック型の定義

### Step 1.1: Cargo.toml + ワークスペース追加

- [x] **ルート Cargo.toml**: `members` に `"crates/video-decoder"` を追加 <!-- 2026-03-17 12:12 JST -->
- [x] **crates/video-decoder/Cargo.toml** を作成 <!-- 2026-03-17 12:12 JST -->

```toml
[package]
name = "video-decoder"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
thiserror = "2.0"
log = "0.4"
wgpu = { version = "24.0", features = ["expose-ids"] }

[dev-dependencies]
pollster = "0.4"
env_logger = "0.11"
image = "0.25"
```

- [x] `cargo check -p video-decoder` が通ることを確認 <!-- 2026-03-17 12:13 JST -->

### Step 1.2: モジュール構造の scaffold

- [x] 以下のディレクトリ・ファイルを空の mod 宣言付きで作成 <!-- 2026-03-17 12:15 JST -->

```
src/
├── lib.rs           # pub mod 宣言 + pub use re-exports
├── error.rs         # VideoError, Result type alias
├── types.rs         # Codec, PixelFormat, ColorSpace, FrameStatus, Backend, VideoInfo
├── handle.rs        # NativeHandle enum (Metal/D3d12/D3d11/Vulkan/Wgpu)
├── session.rs       # VideoSession trait, OutputTarget, SessionConfig
├── demux/
│   └── mod.rs       # pub trait Demuxer, VideoPacket, CodecParameters
├── nal/
│   └── mod.rs       # pub trait NalParser (将来拡張用)
├── convert/
│   └── mod.rs       # NV12ToRgbaPass (stub)
├── backend/
│   └── mod.rs       # create_session(), detect_backends() (stub)
└── util/
    └── mod.rs       # PlaybackState (stub)
```

- [x] `cargo check -p video-decoder` が通ることを確認 <!-- 2026-03-17 12:15 JST -->

### Step 1.3: エラー型 — `error.rs` (~50行)

- [x] `VideoError` enum を実装 (thiserror derive) <!-- 2026-03-17 12:15 JST -->

```rust
// 参考: docs/design/video-decoder-crate-design.md §6.1
#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    #[error("unsupported codec: {0}")]
    UnsupportedCodec(String),
    #[error("no compatible HW decoder found")]
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
```

- [x] **テスト: `error.rs`** <!-- 2026-03-17 12:16 JST -->
  - 正常系: 各バリアントの Display 出力が期待通り
  - 異常系: `anyhow::Error` → `VideoError::Other` 変換

### Step 1.4: 基本型 — `types.rs` (~80行)

- [x] `Codec`, `PixelFormat`, `ColorSpace`, `FrameStatus`, `Backend`, `VideoInfo` を定義 <!-- 2026-03-17 12:15 JST -->

```rust
// 参考: docs/design/video-decoder-crate-design.md §6.1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec { H264, H265, Vp9, Av1 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat { Rgba8Srgb, Rgba8Unorm, Bgra8Srgb, Bgra8Unorm }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSpace { Bt601, #[default] Bt709, Srgb }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStatus { NewFrame, Waiting, EndOfStream }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    VideoToolbox, D3d12Video, MediaFoundation,
    VulkanVideo, GStreamerVaapi, V4l2,
    MediaCodec, Software,
}
```

- [x] **テスト: `types.rs`** <!-- 2026-03-17 12:16 JST -->
  - 正常系: Default, Clone, PartialEq のテスト
  - 正常系: `ColorSpace::default()` == `Bt709`

### Step 1.5: NativeHandle — `handle.rs` (~90行)

- [x] `NativeHandle` enum 実装 (Metal, D3d12, D3d11, Vulkan, Wgpu) <!-- 2026-03-17 12:15 JST -->
- [x] `unsafe impl Send for NativeHandle`, `unsafe impl Sync for NativeHandle` <!-- 2026-03-17 12:15 JST -->

```rust
// 参考: docs/design/video-decoder-crate-design.md §6.1
#[derive(Debug, Clone, Copy)]
pub enum NativeHandle {
    Metal { texture: *mut std::ffi::c_void, device: *mut std::ffi::c_void },
    D3d12 {
        texture: *mut std::ffi::c_void,
        device: *mut std::ffi::c_void,
        command_queue: *mut std::ffi::c_void,
    },
    D3d11 { texture: *mut std::ffi::c_void, device: *mut std::ffi::c_void },
    Vulkan {
        image: u64,
        device: *mut std::ffi::c_void,
        physical_device: *mut std::ffi::c_void,
        instance: *mut std::ffi::c_void,
        queue: *mut std::ffi::c_void,
        queue_family_index: u32,
    },
    Wgpu { queue: *const std::ffi::c_void, texture_id: u64 },
}
unsafe impl Send for NativeHandle {}
unsafe impl Sync for NativeHandle {}
```

- [x] **テスト: `handle.rs`** <!-- 2026-03-17 12:16 JST -->
  - 正常系: NativeHandle::Wgpu の作成と Clone
  - 正常系: Send/Sync が成立することの static assert (`fn _assert_send<T: Send>() {}`)

### Step 1.6: セッション定義 — `session.rs` (~80行)

- [x] `OutputTarget` struct, `SessionConfig` struct, `VideoSession` trait 定義 <!-- 2026-03-17 12:15 JST -->

```rust
// 参考: docs/design/video-decoder-crate-design.md §6.1
pub struct OutputTarget {
    pub native_handle: NativeHandle,
    pub format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub color_space: ColorSpace,
}

pub struct SessionConfig {
    pub looping: bool,
    pub preferred_backend: Option<Backend>,
    pub allow_software_fallback: bool,
    pub decode_buffer_size: usize,
}
impl Default for SessionConfig { /* looping=true, None, true, 4 */ }

pub trait VideoSession: Send {
    fn info(&self) -> &VideoInfo;
    fn position(&self) -> std::time::Duration;
    fn decode_frame(&mut self, dt: std::time::Duration) -> Result<FrameStatus>;
    fn seek(&mut self, position: std::time::Duration) -> Result<()>;
    fn set_looping(&mut self, looping: bool);
    fn is_looping(&self) -> bool;
    fn pause(&mut self);
    fn resume(&mut self);
    fn is_paused(&self) -> bool;
    fn backend(&self) -> Backend;
}
```

- [x] **テスト: `session.rs`** <!-- 2026-03-17 12:16 JST -->
  - 正常系: `SessionConfig::default()` のフィールド検証
  - 正常系: `OutputTarget` の構築

### Step 1.7: lib.rs — パブリック re-exports + `open()` stub (~40行)

- [x] 全モジュールを `pub mod` で公開 <!-- 2026-03-17 12:15 JST -->
- [x] 主要型を `pub use` で re-export <!-- 2026-03-17 12:15 JST -->
- [x] `pub fn open(path, output, config) -> Result<Box<dyn VideoSession>>` の stub (→ `Err(NoHwDecoder)`) <!-- 2026-03-17 12:15 JST -->

- [x] **テスト: `lib.rs`** <!-- 2026-03-17 12:17 JST -->
  - 異常系: `open()` が存在しないファイルで `VideoError::FileNotFound` を返す
  - 異常系: stub 状態で `open()` が `VideoError::NoHwDecoder` を返す

### Step 1.8: Phase 1 検証

- [x] `cargo test -p video-decoder` — 全テスト pass (16 tests + 1 doctest) <!-- 2026-03-17 12:17 JST -->
- [x] `cargo clippy -p video-decoder -- -D warnings` — 警告なし <!-- 2026-03-17 12:17 JST -->
- [x] `cargo fmt -p video-decoder --check` — フォーマット OK <!-- 2026-03-17 12:17 JST -->
- [x] `cargo doc -p video-decoder --no-deps` — 警告なし <!-- 2026-03-17 12:17 JST -->
- [ ] テストカバレッジ 90% 以上を確認 (`cargo llvm-cov -p video-decoder`)、未カバー部分のテスト追加 — `cargo-llvm-cov` 未インストールのため保留
- [x] `cargo build -p video-decoder` が正常完了 <!-- 2026-03-17 12:17 JST -->
- [x] **動作確認**: `cargo test -p video-decoder` を実行し、全型定義が正しく構築でき、stub の `open()` が期待通りのエラーを返すことを確認する。目的の動作と異なる場合は修正を繰り返す <!-- 2026-03-17 12:18 JST -->

---

## Phase 2: Demux + NAL パーサ

**目的**: MP4 コンテナの demux と H.264 NAL unit の解析。Vulkan Video / D3D12 Video / V4L2 で共通使用。

### Step 2.1: Cargo.toml に demux 依存追加

- [ ] `mp4parse = "0.17"` と `h264-reader = "0.7"` を dependencies に追加
- [ ] `cargo check -p video-decoder` が通ることを確認

### Step 2.2: Demuxer trait — `demux/mod.rs` (~60行)

- [ ] `Demuxer` trait, `VideoPacket` struct, `CodecParameters` struct を定義
- [ ] `pub fn create_demuxer(path: &str) -> Result<Box<dyn Demuxer>>` ファクトリ関数

```rust
// 参考: docs/design/video-decoder-crate-design.md §6.2
pub struct VideoPacket {
    pub data: Vec<u8>,       // NAL unit (Annex B: 00 00 00 01 + NAL)
    pub pts: Duration,
    pub dts: Duration,
    pub is_keyframe: bool,
}

pub struct CodecParameters {
    pub codec: Codec,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: Duration,
    pub extra_data: Vec<u8>, // SPS + PPS bytes
}

pub trait Demuxer: Send {
    fn parameters(&self) -> &CodecParameters;
    fn next_packet(&mut self) -> Result<Option<VideoPacket>>;
    fn seek(&mut self, position: Duration) -> Result<()>;
}
```

### Step 2.3: MP4 Demuxer — `demux/mp4.rs` (~200行)

- [ ] `Mp4Demuxer` struct を実装: ファイル読み込み、ビデオトラック検出、H.264 パラメータセット抽出
- [ ] `Demuxer` trait impl: `parameters()`, `next_packet()` (サンプル→NAL unit 変換), `seek()`

```rust
// 参考: mp4parse API
// let mut file = std::fs::File::open(path)?;
// let mp4 = mp4parse::read_mp4(&mut file)?;
// let track = mp4.tracks.iter().find(|t| t.track_type == mp4parse::TrackType::Video);
// SPS/PPS は track.stsd.avcc.sequence_parameter_sets から取得
// サンプルは track.samples / chunk_offsets から読み取り
```

- [ ] **⚠ 300行超え見込み**: MP4 のサンプルテーブル解析が複雑な場合 `demux/mp4_samples.rs` に分割を検討

### Step 2.4: NAL パーサ — `nal/mod.rs` + `nal/h264.rs` (~150行)

- [ ] `nal/mod.rs`: `NalUnit` struct (type, data), `NalParser` trait (将来 H.265 拡張用)
- [ ] `nal/h264.rs`: h264-reader で SPS/PPS/Slice Header をパース

```rust
// 参考: h264-reader API
// use h264_reader::nal::Nal;
// use h264_reader::nal::sps::SeqParameterSet;
// use h264_reader::nal::pps::PicParameterSet;
//
// SPS から width/height/profile/level を抽出:
// let sps = SeqParameterSet::from_bytes(&sps_bytes)?;
// let (width, height) = sps.pixel_dimensions()?;
//
// Slice Header から frame_num, poc, reference list を抽出:
// (DPB 管理に必要 — Phase 6, 7 で使用)
```

- [ ] `pub struct H264Context` — SPS/PPS 保持 + Slice Header パース結果を返すメソッド

### Step 2.5: テスト — Demux + NAL

- [ ] **テスト用フィクスチャ**: `tests/fixtures/test_h264_360p.mp4` (10 秒, 360p, H.264 Baseline, < 500KB)
- [ ] **正常系テスト**:
  - `Mp4Demuxer::new()` で codec, width, height, fps, duration が正しい
  - `next_packet()` で全パケットが PTS 昇順で取得できる
  - 先頭パケットが `is_keyframe == true`
  - H264Context で SPS から width/height が正しく抽出される
- [ ] **異常系テスト**:
  - 存在しないファイル → `VideoError::FileNotFound`
  - 空ファイル → `VideoError::Demux`
  - 非 MP4 ファイル (テキスト) → `VideoError::Demux`
  - 音声のみ MP4 (ビデオトラックなし) → `VideoError::Demux`
  - `seek()` が duration 超過 → `VideoError::Seek`

### Step 2.6: Phase 2 検証

- [ ] `cargo test -p video-decoder` — 全テスト pass
- [ ] `cargo clippy -p video-decoder -- -D warnings` — 警告なし
- [ ] `cargo fmt -p video-decoder --check` — フォーマット OK
- [ ] テストカバレッジ 90% 以上を確認、未カバー部分のテスト追加
- [ ] `cargo build -p video-decoder` が正常完了
- [ ] **動作確認**: テストコードで `Mp4Demuxer` を使い `test_h264_360p.mp4` を demux し、全パケットが PTS 昇順で取得され、SPS から正しい解像度が抽出されることを確認する。目的の動作と異なる場合は修正を繰り返す

---

## Phase 3: NV12→RGBA 色変換 + PlaybackState

**目的**: GPU コンピュートシェーダによる NV12→RGBA 変換パイプラインと、フレームレート制御ロジック

### Step 3.1: WGSL シェーダ — `shaders/nv12_to_rgba.wgsl` (~30行)

- [ ] NV12 (Y plane + UV plane) → RGBA 変換コンピュートシェーダを作成

```wgsl
// 参考: docs/design/video-decoder-crate-design.md §6 (NV12 色変換シェーダ)
@group(0) @binding(0) var y_tex: texture_2d<f32>;
@group(0) @binding(1) var uv_tex: texture_2d<f32>;
@group(0) @binding(2) var out_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let y  = textureLoad(y_tex, gid.xy, 0).r;
    let uv = textureLoad(uv_tex, gid.xy / 2, 0).rg;
    let u = uv.x - 0.5;
    let v = uv.y - 0.5;
    // BT.709 coefficients
    let r = y + 1.5748 * v;
    let g = y - 0.1873 * u - 0.4681 * v;
    let b = y + 1.8556 * u;
    textureStore(out_tex, gid.xy, vec4<f32>(clamp(r, 0.0, 1.0), clamp(g, 0.0, 1.0), clamp(b, 0.0, 1.0), 1.0));
}
```

### Step 3.2: 色空間パラメータ — `convert/color_space.rs` (~40行)

- [ ] BT.601 / BT.709 の変換係数を定義

```rust
pub struct ColorMatrix {
    pub kr: f32, pub kg: f32, pub kb: f32, // Y coefficients
    pub rv: f32, pub gu: f32, pub gv: f32, pub bu: f32, // UV→RGB
}
pub fn bt709() -> ColorMatrix { /* ... */ }
pub fn bt601() -> ColorMatrix { /* ... */ }
```

### Step 3.3: NV12ToRgbaPass — `convert/mod.rs` (~150行)

- [ ] `NV12ToRgbaPass` struct: wgpu コンピュートパイプライン + bind group layout
- [ ] `new(device, color_space, width, height)`: シェーダモジュール・パイプライン作成
- [ ] `convert(device, encoder, y_view, uv_view, output_view)`: dispatch をエンコーダに記録

```rust
// 参考: docs/design/video-decoder-crate-design.md §6.4
pub struct NV12ToRgbaPass {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    width: u32,
    height: u32,
}

impl NV12ToRgbaPass {
    pub fn new(device: &wgpu::Device, color_space: ColorSpace, w: u32, h: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("nv12_to_rgba"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/nv12_to_rgba.wgsl").into()),
        });
        // bind_group_layout: 0=y_tex, 1=uv_tex, 2=out_tex
        // compute pipeline: workgroup_size(8, 8)
        // dispatch: ceil(w/8) x ceil(h/8)
        todo!()
    }
    pub fn convert(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder,
                   y_view: &wgpu::TextureView, uv_view: &wgpu::TextureView,
                   output_view: &wgpu::TextureView) { todo!() }
}
```

### Step 3.4: PlaybackState — `util/mod.rs` + `util/timestamp.rs` (~100行)

- [ ] `PlaybackState` struct: position, duration, fps, looping, paused, frame_interval
- [ ] `tick(dt) -> bool` — dt 加算、次フレームのタイミングかどうか判定
- [ ] `should_loop() -> bool` — ループ判定 + position リセット

```rust
// util/timestamp.rs
pub struct PlaybackState {
    pub position: Duration,
    pub duration: Duration,
    pub fps: f64,
    pub looping: bool,
    pub paused: bool,
    frame_interval: Duration,    // 1.0 / fps
    elapsed_since_frame: Duration,
}

impl PlaybackState {
    pub fn new(duration: Duration, fps: f64, looping: bool) -> Self { /* ... */ }
    /// dt を加算し、新しいフレームを取得すべきなら true を返す
    pub fn tick(&mut self, dt: Duration) -> bool { /* ... */ }
    /// ストリーム終端判定。looping なら position を 0 にリセットして true
    pub fn should_loop(&mut self) -> bool { /* ... */ }
}
```

### Step 3.5: `util/ring_buffer.rs` — DPB 用リングバッファ (~80行)

- [ ] `DpbManager<T>` — POC ベースの参照フレーム管理 (D3D12 Video / Vulkan Video 共通)

```rust
pub struct DpbManager<T> {
    slots: Vec<DpbSlot<T>>,
    max_slots: usize,
}
pub struct DpbSlot<T> {
    pub resource: T,
    pub poc: i32,
    pub in_use: bool,
}
impl<T> DpbManager<T> {
    pub fn new(max_slots: usize) -> Self { /* ... */ }
    pub fn allocate(&mut self, poc: i32) -> Option<&mut DpbSlot<T>> { /* ... */ }
    pub fn release(&mut self, poc: i32) { /* ... */ }
    pub fn get_references(&self, ref_list: &[i32]) -> Vec<&DpbSlot<T>> { /* ... */ }
    pub fn reset(&mut self) { /* ... */ }
}
```

### Step 3.6: テスト — 色変換 + PlaybackState + DPB

- [ ] **正常系テスト (PlaybackState)**:
  - 30fps 動画で `tick(33ms)` → true (1フレーム分経過)
  - 30fps 動画で `tick(16ms)` → false (半フレーム)
  - ループ: position が duration 超過後に 0 にリセット
  - pause 中は tick が常に false
- [ ] **異常系テスト (PlaybackState)**:
  - fps=0 → パニックしない (0除算対策)
  - duration=0 → 即座に EndOfStream
- [ ] **正常系テスト (DpbManager)**:
  - allocate → get_references で参照が取得できる
  - release 後は get_references に含まれない
  - reset で全スロットが解放される
- [ ] **異常系テスト (DpbManager)**:
  - max_slots 超過時に最古の未使用スロットが再利用される
  - 存在しない poc の release → no-op
- [ ] **正常系テスト (NV12ToRgbaPass)** — ヘッドレス環境のため未検証、stub テスト
  - `NV12ToRgbaPass::new()` がパニックしない (wgpu adapter 取得可能な場合)

### Step 3.7: Phase 3 検証

- [ ] `cargo test -p video-decoder` — 全テスト pass
- [ ] `cargo clippy -p video-decoder -- -D warnings` — 警告なし
- [ ] `cargo fmt -p video-decoder --check` — フォーマット OK
- [ ] テストカバレッジ 90% 以上を確認、未カバー部分のテスト追加
- [ ] `cargo build -p video-decoder` が正常完了
- [ ] **動作確認**: テストコードで `PlaybackState` を使い 30fps/60fps 動画のフレームタイミング制御、ループ再生、pause/resume が仕様通り動作することを確認する。`DpbManager` の allocate/release/reset が正しく参照管理することを確認する。目的の動作と異なる場合は修正を繰り返す

---

## Phase 4: ソフトウェアデコーダ + バックエンド選択

**目的**: CPU フォールバックデコーダの実装とバックエンド自動選択ロジック。全プラットフォームで動画再生が可能になる最小構成。

### Step 4.1: Cargo.toml にソフトウェアデコーダ依存追加

- [ ] `openh264 = "0.6"` を dependencies に追加

### Step 4.2: ソフトウェアデコーダ — `backend/software.rs` (~200行)

- [ ] `SwVideoSession` struct: demuxer, openh264 Decoder, frame_buffer (RGBA Vec), playback, info
- [ ] `VideoSession` trait impl

```rust
// 参考: openh264 API
// use openh264::decoder::Decoder;
// let mut decoder = Decoder::new()?;
// let yuv = decoder.decode(&nal_data)?;
// let rgb = yuv.to_rgb()?; // → Vec<u8> (RGB)
// RGB → RGBA 変換 (alpha=255 追加)
// queue.write_texture() で GPU テクスチャに書き込み

pub struct SwVideoSession {
    demuxer: Box<dyn Demuxer>,
    decoder: openh264::decoder::Decoder,
    frame_buffer: Vec<u8>,
    output: OutputTarget,
    playback: PlaybackState,
    info: VideoInfo,
}
```

- [ ] decode_frame: demuxer.next_packet() → openh264 decode → YUV→RGBA → write_texture
- [ ] seek: demuxer.seek() → decoder flush
- [ ] **⚠ 300行超え見込み**: YUV→RGBA 変換ロジックが大きい場合 `backend/sw_yuv_convert.rs` に分割

### Step 4.3: バックエンド選択 — `backend/mod.rs` (~120行)

- [ ] `create_session(path, output, config)`: NativeHandle 種別でバックエンド候補を決定、順に試行
- [ ] `detect_backends(handle)`: プラットフォーム + ランタイム検出で候補 Vec を返す
- [ ] `create_with_backend(path, output, config, backend)`: cfg マッチでバックエンド作成

```rust
// 参考: docs/design/video-decoder-crate-design.md §6.3
pub fn create_session(path: &str, output: OutputTarget, config: SessionConfig)
    -> Result<Box<dyn VideoSession>>
{
    // 1. ファイル存在チェック
    if !std::path::Path::new(path).exists() {
        return Err(VideoError::FileNotFound(path.to_string()));
    }
    // 2. preferred_backend 指定時はそれを試行
    // 3. detect_backends() で候補リスト取得
    // 4. 順に create_with_backend() を試行、失敗時は次へ
    // 5. 全失敗 + allow_software_fallback → Software
    // 6. 全失敗 + !allow_software_fallback → Err(NoHwDecoder)
}
```

### Step 4.4: lib.rs の `open()` を実装に接続

- [ ] `open()` が `backend::create_session()` を呼び出すように変更

### Step 4.5: テスト — ソフトウェアデコーダ

- [ ] **正常系テスト**:
  - `open("test.mp4", output_wgpu, config)` → SwVideoSession が返る
  - `info()` が正しい codec, width, height, fps, duration
  - `decode_frame(33ms)` → `FrameStatus::NewFrame`
  - 連続 10 フレームデコード → 全て NewFrame
  - `seek(Duration::from_secs(5))` → 次フレームの position が 5s 付近
  - ループ再生: duration 超過後に position が 0 に戻る
  - `pause()` → `decode_frame()` が `Waiting` を返す
  - `resume()` → `decode_frame()` が `NewFrame` を返す
- [ ] **異常系テスト**:
  - 破損 MP4 → `VideoError::Demux`
  - `allow_software_fallback = false` + Wgpu handle → `VideoError::NoHwDecoder`
  - EndOfStream 後の `decode_frame()` → `EndOfStream`

### Step 4.6: サンプル — `examples/decode_to_png.rs` (~80行)

- [ ] CLI: `cargo run -p video-decoder --example decode_to_png -- <input.mp4> <output_dir>`
- [ ] SW デコーダで先頭 10 フレームを PNG 出力 (HW 不要)

```rust
fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("usage: decode_to_png <input.mp4> <output_dir>");
    let out_dir = std::env::args().nth(2).unwrap_or("frames".to_string());
    // wgpu device 作成 (headless)
    // Texture 作成 (RGBA8, COPY_SRC)
    // open() → SwVideoSession
    // 10 フレーム decode → readback → image::save_buffer() で PNG 出力
    Ok(())
}
```

### Step 4.7: Phase 4 検証

- [ ] `cargo test -p video-decoder` — 全テスト pass
- [ ] `cargo clippy -p video-decoder -- -D warnings` — 警告なし
- [ ] `cargo fmt -p video-decoder --check` — フォーマット OK
- [ ] テストカバレッジ 90% 以上を確認、未カバー部分のテスト追加
- [ ] `cargo build -p video-decoder` が正常完了
- [ ] **動作確認**: `cargo run -p video-decoder --example decode_to_png -- test.mp4 frames/` を実行し、出力された PNG ファイルが正しい映像フレーム (色・解像度・フレーム順) であることを目視確認する。seek 後の PNG が正しい時刻のフレームであることを確認する。目的の動作と異なる場合は修正を繰り返す

---

## Phase 5: macOS / iOS バックエンド (VideoToolbox)

**目的**: macOS/iOS で AVFoundation + CVMetalTextureCache による HW ゼロコピーデコード

### Step 5.1: Cargo.toml に macOS/iOS 依存追加

- [ ] `cfg(any(target_os = "macos", target_os = "ios"))` ブロックに objc2 系クレートを追加 (バージョンは §ライブラリバージョン一覧参照)

### Step 5.2: AppleVideoSession — `backend/apple.rs` (~280行)

- [ ] `AppleVideoSession` struct: reader, track_output, texture_cache, output_mtl_texture, command_queue, playback, info
- [ ] `new(path, output, config)`:
  1. AVURLAsset → AVAssetReader → AVAssetReaderTrackOutput 作成
  2. CVMetalTextureCacheCreate で texture_cache 作成
  3. VideoInfo (duration, fps, codec) 取得
- [ ] `VideoSession` trait impl

```rust
// 参考: docs/design/video-decoder-crate-design.md §8.1
// AVAssetReader API flow:
// let url = NSURL::fileURLWithPath(&NSString::from_str(path));
// let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
// let track = asset.tracksWithMediaType(AVMediaTypeVideo).firstObject()?;
// let output = AVAssetReaderTrackOutput::alloc()
//     .initWithTrack_outputSettings(&track, output_settings);
// let reader = AVAssetReader::alloc().initWithAsset_error(&asset)?;
// reader.addOutput(&output);
// reader.startReading();
```

- [ ] decode_frame:
  1. `track_output.copyNextSampleBuffer()` → CMSampleBuffer
  2. `CMSampleBufferGetImageBuffer()` → CVPixelBuffer
  3. `CVMetalTextureCacheCreateTextureFromImage()` → 一時 MTLTexture
  4. Metal blit → OutputTarget の MTLTexture にコピー
  5. CommandBuffer commit

- [ ] **⚠ 300行超え見込み**: CVMetalTextureCache 周りが長い場合 `backend/apple_metal.rs` に Metal blit ロジックを分離

### Step 5.3: seek 実装

- [ ] AVAssetReader は seek 不可 → timeRange 指定で再作成
- [ ] texture_cache は再利用

### Step 5.4: Drop 実装 — リソース解放

- [ ] AVAssetReader, CVMetalTextureCache, MTLCommandQueue を Drop で解放
- [ ] Instruments でリーク検出なしを確認

### Step 5.5: backend/mod.rs に Apple バックエンド接続

- [ ] `detect_backends`: `NativeHandle::Metal` → `[VideoToolbox]`
- [ ] `create_with_backend`: `Backend::VideoToolbox` → `AppleVideoSession::new()`

### Step 5.6: テスト — macOS バックエンド

- [ ] **正常系テスト (macOS 実機)**:
  - `open()` で VideoToolbox バックエンドが選択される
  - `info().backend == Backend::VideoToolbox`
  - 10 フレーム連続 decode → 全て NewFrame
  - ループ再生が途切れず動作
  - seek(5s) → 次フレーム PTS が 5s 付近
- [ ] **異常系テスト**:
  - 非対応コーデック (VP9 MP4) → SW フォールバック
  - 不正な Metal texture ポインタ → `VideoError::GpuInterop`
- [ ] **ヘッドレス CI**: macOS バックエンドテストは GPU 必須のため `#[ignore]` 付与、ローカル実行

### Step 5.7: Phase 5 検証

- [ ] `cargo test -p video-decoder` — 全テスト pass (ignore 除外)
- [ ] `cargo clippy -p video-decoder -- -D warnings` — 警告なし
- [ ] `cargo fmt -p video-decoder --check` — フォーマット OK
- [ ] テストカバレッジ 90% 以上を確認、未カバー部分のテスト追加
- [ ] `cargo build -p video-decoder` が正常完了
- [ ] **動作確認**: macOS で `cargo run -p video-decoder --example wgpu_video_bg -- test.mp4` を実行し、ウィンドウに動画が 60fps で滑らかに背景再生されることを目視確認する。ループ再生が途切れなく動作し、seek 操作後に正しいフレームから再開されることを確認する。Activity Monitor / Instruments で Metal リソースリークがないことを確認する。目的の動作と異なる場合は修正を繰り返す

---

## Phase 6: Windows バックエンド (D3D12 Video + Media Foundation)

**目的**: D3D12 Video API (優先) と Media Foundation HW decode (フォールバック)

### Step 6.1: Cargo.toml に Windows 依存追加

- [ ] `cfg(target_os = "windows")` ブロックに `windows = "0.58"` + features 追加 (バージョンは §ライブラリバージョン一覧参照)

### Step 6.2: D3D12 Video — `backend/d3d12_video.rs` (~280行)

- [ ] `D3d12VideoSession` struct: demuxer, video_device, decoder, decoder_heap, dpb (DpbManager), decode_output, nv12_pass, command_allocator, command_list, command_queue, fence, fence_value, playback, info
- [ ] ランタイム検出: `is_supported(device)` — QueryInterface + CheckFeatureSupport

```rust
// 参考: docs/design/video-decoder-crate-design.md §8.2
// ID3D12Device::QueryInterface(IID_ID3D12VideoDevice) → ID3D12VideoDevice
// CheckFeatureSupport(D3D12_FEATURE_VIDEO_DECODE_SUPPORT, H264 profile, 1920x1080)
// CreateVideoDecoder(D3D12_VIDEO_DECODER_DESC { profile: H264, ... })
// CreateVideoDecoderHeap(D3D12_VIDEO_DECODER_HEAP_DESC { ... })
```

- [ ] decode_frame:
  1. demuxer.next_packet() + h264-reader パース (Phase 2 共通)
  2. DPB 参照リスト構築 (DpbManager 使用)
  3. command_list.DecodeFrame()
  4. nv12_pass.convert() (Phase 3 共通)
  5. ExecuteCommandLists + fence signal
- [ ] **⚠ 300行超え見込み**: DPB 管理 + デコードコマンド構築を `backend/d3d12_decode_cmd.rs` に分離

### Step 6.3: Media Foundation フォールバック — `backend/media_foundation.rs` (~250行)

- [ ] `MfVideoSession` struct: reader, d3d11_device, staging_texture, shared_d3d12_texture, nv12_pass, playback, info
- [ ] MF 初期化: MFCreateSourceReaderFromURL + D3D11 デバイスマネージャ + HW decode 有効化

```rust
// 参考: docs/design/video-decoder-crate-design.md §8.3
// MFStartup(MF_VERSION)?;
// let attributes = MFCreateAttributes(3)?;
// attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &dxgi_manager)?;
// attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
// let reader = MFCreateSourceReaderFromURL(path, &attributes)?;
```

- [ ] D3D11→D3D12 interop: DXGI SharedHandle
- [ ] COM 初期化/解放: CoInitializeEx / CoUninitialize

### Step 6.4: backend/mod.rs に Windows バックエンド接続

- [ ] `detect_backends`:
  - `NativeHandle::D3d12` → `is_supported()` ? `[D3d12Video, MediaFoundation]` : `[MediaFoundation]`
  - `NativeHandle::D3d11` → `[MediaFoundation]`
- [ ] `create_with_backend`: D3d12Video → `D3d12VideoSession`, MediaFoundation → `MfVideoSession`

### Step 6.5: テスト — Windows バックエンド

- [ ] **正常系テスト (D3D12 Video)**:
  - `is_supported()` が true/false を正しく返す
  - 対応環境で D3D12 Video が選択される
  - 10 フレーム decode → 全て NewFrame
  - seek + ループが動作
- [ ] **正常系テスト (Media Foundation)**:
  - D3D12 Video 非対応時に MF が選択される
  - HW decode が有効 (ログ確認)
  - DXGI SharedHandle interop が動作
- [ ] **異常系テスト**:
  - 不正な D3D12 device ポインタ → `VideoError::GpuInterop`
  - 破損 MP4 → `VideoError::Demux`
  - COM 未初期化環境 → 適切なエラー
- [ ] **ヘッドレス CI**: Windows バックエンドテストは GPU 必須のため `#[ignore]` 付与

### Step 6.6: Phase 6 検証

- [ ] `cargo test -p video-decoder` — 全テスト pass (ignore 除外)
- [ ] `cargo clippy -p video-decoder -- -D warnings` — 警告なし
- [ ] `cargo fmt -p video-decoder --check` — フォーマット OK
- [ ] テストカバレッジ 90% 以上を確認、未カバー部分のテスト追加
- [ ] `cargo build -p video-decoder` が正常完了 (Windows ターゲット)
- [ ] **動作確認**: Windows 10/11 で `cargo run -p video-decoder --example wgpu_video_bg -- test.mp4` を実行し、D3D12 Video バックエンドで動画が 60fps 再生されることを確認する。D3D12 Video 非対応環境では MF フォールバックで再生されることを確認する。ログ出力で `Using D3d12Video decoder` / `Using MediaFoundation decoder` が表示されることを確認する。NVIDIA / AMD / Intel GPU それぞれで動作確認する。目的の動作と異なる場合は修正を繰り返す

---

## Phase 7: Linux バックエンド (Vulkan Video + GStreamer + V4L2)

**目的**: Vulkan Video (優先), GStreamer VA-API (cfg), V4L2 Stateless (cfg) の 3 段フォールバック

### Step 7.1: Cargo.toml に Linux 依存追加

- [ ] `cfg(target_os = "linux")` ブロックに `ash = "0.38"` (常時), gstreamer 系 (optional), nix (optional)
- [ ] features: `gstreamer`, `v4l2`

### Step 7.2: Vulkan Video — `backend/vulkan_video.rs` (~280行)

- [ ] `VkVideoSession` struct: demuxer, video_session, session_params, dpb (DpbManager), decode_output, nv12_pass, vk_device, video_queue, command_pool, playback, info
- [ ] ランタイム検出: `is_supported(instance, physical_device)` — VkQueueFlags::VIDEO_DECODE_KHR チェック

```rust
// 参考: docs/design/video-decoder-crate-design.md §8.4
// ash で Video Decode キュー対応を確認
// let queue_families = instance.get_physical_device_queue_family_properties(phys);
// queue_families.iter().any(|qf| qf.queue_flags.contains(vk::QueueFlags::VIDEO_DECODE_KHR))
//
// Video Session 作成:
// vk::VideoDecodeH264ProfileInfoKHR { std_profile_idc: HIGH, ... }
// vk::VideoSessionCreateInfoKHR { queue_family_index, picture_format: NV12, ... }
// vkCreateVideoSessionKHR(device, &create_info)
```

- [ ] decode_frame: demuxer.next_packet() → h264-reader → vkCmdDecodeVideoKHR → nv12_pass.convert()
- [ ] DPB 管理: DpbManager<vk::Image> (Phase 3 の共通ロジック使用)
- [ ] **⚠ 300行超え見込み**: Video Session 初期化を `backend/vk_video_init.rs` に分離

### Step 7.3: GStreamer VA-API — `backend/gst_vaapi.rs` (~200行, cfg feature)

- [ ] `#[cfg(feature = "gstreamer")]`
- [ ] `GstVideoSession` struct: pipeline, appsink, vk_device, nv12_pass, playback, info
- [ ] パイプライン: `filesrc ! decodebin3 ! video/x-raw(memory:DMABuf),format=NV12 ! appsink`

```rust
// 参考: docs/design/video-decoder-crate-design.md §8.5
// gst::init()?;
// let pipeline = gst::parse::launch(&format!(
//     "filesrc location={path} ! decodebin3 ! \
//      video/x-raw(memory:DMABuf),format=NV12 ! appsink name=sink sync=false"
// ))?;
// appsink.try_pull_sample(timeout=0) で non-blocking 取得
// DMA-BUF fd → VkImportMemoryFdInfoKHR → temp VkImage → nv12_pass
```

### Step 7.4: V4L2 Stateless — `backend/v4l2.rs` (~200行, cfg feature)

- [ ] `#[cfg(feature = "v4l2")]`
- [ ] `V4l2VideoSession` struct: fd, demuxer, output_buffers, capture_buffers, nv12_pass, vk_device, playback, info
- [ ] デバイス検出: `/dev/video*` スキャン + VIDIOC_QUERYCAP + VIDIOC_ENUM_FMT

```rust
// 参考: docs/design/video-decoder-crate-design.md §8.6
// ioctl(fd, VIDIOC_QUERYCAP) → V4L2_CAP_VIDEO_M2M_MPLANE
// ioctl(fd, VIDIOC_ENUM_FMT) → V4L2_PIX_FMT_H264_SLICE
// NAL submit: VIDIOC_QBUF → VIDIOC_DQBUF → VIDIOC_EXPBUF (DMA-BUF fd)
```

### Step 7.5: backend/mod.rs に Linux バックエンド接続

- [ ] `detect_backends`:
  - `NativeHandle::Vulkan` + Linux → Vulkan Video (ランタイム) + GStreamer (cfg) + V4L2 (cfg)
- [ ] `create_with_backend`: 各バックエンドへの dispatch

### Step 7.6: テスト — Linux バックエンド

- [ ] **正常系テスト (Vulkan Video)**:
  - `is_supported()` が正しく検出
  - 対応ドライバで decode → NewFrame
- [ ] **正常系テスト (GStreamer)**:
  - `cargo test --features gstreamer` でテスト pass
  - DMA-BUF パスと CPU フォールバックの両方
- [ ] **異常系テスト**:
  - Vulkan Video 非対応ドライバ → GStreamer にフォールバック
  - GStreamer 未インストール (`--no-default-features`) → compile から除外確認
  - V4L2 デバイスなし → Software フォールバック
- [ ] **cfg 排除テスト**:
  - `cargo build -p video-decoder` (features なし) → gstreamer/nix 依存ゼロ確認
  - `cargo build -p video-decoder --features gstreamer` → ビルド成功
  - `cargo build -p video-decoder --features v4l2` → ビルド成功

### Step 7.7: Phase 7 検証

- [ ] `cargo test -p video-decoder` — 全テスト pass
- [ ] `cargo test -p video-decoder --features gstreamer` — GStreamer テスト pass
- [ ] `cargo clippy -p video-decoder -- -D warnings` — 警告なし
- [ ] `cargo fmt -p video-decoder --check` — フォーマット OK
- [ ] テストカバレッジ 90% 以上を確認、未カバー部分のテスト追加
- [ ] `cargo build -p video-decoder` が正常完了 (Linux ターゲット)
- [ ] **動作確認**: Linux で `cargo run -p video-decoder --example wgpu_video_bg -- test.mp4` を実行し、Vulkan Video 対応ドライバでは Vulkan Video バックエンドで再生されることを確認する。Vulkan Video 非対応環境では `--features gstreamer` で GStreamer VA-API にフォールバックすることを確認する。ログ出力で使用中バックエンドを確認する。`vainfo` コマンドで VA-API HW デコードが有効であることを確認する。目的の動作と異なる場合は修正を繰り返す

---

## Phase 8: Android バックエンド (MediaCodec)

**目的**: Android NDK の MediaCodec + AHardwareBuffer → Vulkan ゼロコピーデコード

### Step 8.1: Cargo.toml に Android 依存追加

- [ ] `cfg(target_os = "android")` ブロックに `ndk = "0.9"` + `ash = "0.38"` を追加

### Step 8.2: MediaCodec — `backend/media_codec.rs` (~250行)

- [ ] `McVideoSession` struct: extractor, codec, vk_device, nv12_pass, playback, info
- [ ] 初期化: AMediaExtractor → AMediaCodec (バッファモード)

```rust
// 参考: docs/design/video-decoder-crate-design.md §8.7
// let extractor = AMediaExtractor::new()?;
// extractor.set_data_source(path)?;
// let codec = AMediaCodec::create_decoder_by_type("video/avc")?;
// codec.configure(&format, None, 0)?;
// codec.start()?;
```

- [ ] decode_frame: input submit → output dequeue → AHardwareBuffer → VkImportAndroidHardwareBufferInfoANDROID → nv12_pass

### Step 8.3: backend/mod.rs に Android バックエンド接続

- [ ] `detect_backends`: `NativeHandle::Vulkan` + Android → `[MediaCodec]`
- [ ] `create_with_backend`: `Backend::MediaCodec` → `McVideoSession::new()`

### Step 8.4: テスト — Android バックエンド

- [ ] **正常系テスト**:
  - AMediaExtractor でトラック情報取得
  - AMediaCodec でデコード → AHardwareBuffer 取得
  - Vulkan import → nv12_pass → RGBA テクスチャ
- [ ] **異常系テスト**:
  - 非対応コーデック → SW フォールバック
  - AHardwareBuffer 取得失敗 → `VideoError::GpuInterop`
- [ ] **クロスビルド確認**: `cargo build -p video-decoder --target aarch64-linux-android`

### Step 8.5: Phase 8 検証

- [ ] `cargo check -p video-decoder --target aarch64-linux-android` — 型チェック pass
- [ ] `cargo clippy -p video-decoder -- -D warnings` — 警告なし
- [ ] `cargo fmt -p video-decoder --check` — フォーマット OK
- [ ] テストカバレッジ 90% 以上を確認 (ホスト環境テスト分)、未カバー部分のテスト追加
- [ ] `cargo build -p video-decoder --target aarch64-linux-android` が正常完了
- [ ] **動作確認**: Android 実機 (API 26+) またはエミュレータでアプリケーションを起動し、MediaCodec バックエンドで MP4 が再生されることを確認する。AHardwareBuffer → Vulkan import パスが動作し、映像が正しく表示されることを確認する。`adb logcat` で使用中バックエンドとデコード性能のログを確認する。目的の動作と異なる場合は修正を繰り返す

---

## Phase 9: E2E テスト・サンプル・ドキュメント・CI

**目的**: 全プラットフォーム横断の E2E テスト、サンプルコード、rustdoc、CI 統合

### Step 9.1: E2E サンプル — `examples/wgpu_video_bg.rs` (~150行)

- [ ] wgpu ウィンドウに動画背景を表示する完全なサンプル
- [ ] winit イベントループ + wgpu 初期化 + video-decoder 統合

```rust
// 1. winit Window + wgpu Device/Queue/Surface 作成
// 2. RGBA8 Texture 作成 (TEXTURE_BINDING | COPY_DST | STORAGE_BINDING)
// 3. NativeHandle 取得 (unsafe get_native_texture_handle)
// 4. video_decoder::open("video.mp4", output, config)?
// 5. render loop:
//    session.decode_frame(dt)?;
//    fullscreen quad で texture を画面に描画
```

### Step 9.2: 結合テスト — `tests/integration_open.rs` (~60行)

- [ ] **正常系**: MP4 を open → info() のフィールド検証 (codec, width, height, fps, duration)
- [ ] **異常系**: 不正パス、非動画ファイル、音声のみ MP4

### Step 9.3: 結合テスト — `tests/integration_decode.rs` (~100行)

- [ ] **正常系**: SW デコーダで 10 フレーム decode → 全て NewFrame
- [ ] **正常系**: seek → decode → position 検証
- [ ] **正常系**: ループ再生 → EndOfStream にならず position リセット
- [ ] **異常系**: pause 中の decode → Waiting

### Step 9.4: ベンチマーク — `benches/decode_throughput.rs` (~50行)

- [ ] 1080p MP4 の SW デコードスループット計測
- [ ] `cargo bench -p video-decoder` で実行可能

### Step 9.5: rustdoc

- [ ] 全 `pub` 型・関数に doc comment を追加
- [ ] `cargo doc -p video-decoder --no-deps` が警告なし
- [ ] lib.rs の crate-level doc にクイックスタートコード例を含める

### Step 9.6: Phase 9 検証

- [ ] `cargo test -p video-decoder` — 全テスト pass (結合テスト含む)
- [ ] `cargo test -p video-decoder --features gstreamer` — GStreamer テスト含む
- [ ] `cargo clippy -p video-decoder -- -D warnings` — 警告なし
- [ ] `cargo fmt -p video-decoder --check` — フォーマット OK
- [ ] `cargo doc -p video-decoder --no-deps` — 警告なし
- [ ] `cargo bench -p video-decoder` — ベンチ動作
- [ ] テストカバレッジ 90% 以上を最終確認 (`cargo llvm-cov -p video-decoder`)、未カバー部分のテスト追加
- [ ] `cargo build -p video-decoder` が全対応ターゲットで正常完了
- [ ] **動作確認 (最終)**: 各プラットフォーム (macOS, Windows, Linux) で `cargo run -p video-decoder --example wgpu_video_bg -- test.mp4` を実行し、以下を全て確認する。目的の動作と異なる場合は修正を繰り返す:
  - 動画が 60fps で滑らかに再生される
  - ループ再生が途切れなく動作する
  - seek 操作が正しいフレームに移動する
  - pause / resume が正しく動作する
  - ログに使用中バックエンド名が表示される
  - 各プラットフォームで適切な HW デコーダが自動選択される
  - HW 非対応環境で SW フォールバックが動作する
  - `examples/decode_to_png` で出力された PNG が正しい映像フレームである
