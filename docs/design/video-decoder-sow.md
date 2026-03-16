# video-decoder crate — Statement of Work (SoW)

> **プロジェクト名**: video-decoder
> **バージョン**: 0.1.0
> **作成日**: 2026-03-17
> **目的**: wgpu アプリケーションに動画フレームの HW デコード → GPU テクスチャ直接書き込みを提供する独立ライブラリクレート

---

## 1. スコープ

### 1.1 In Scope (対象)

| カテゴリ | 内容 |
|----------|------|
| コンテナ demux | MP4 (H.264/HEVC) 対応。pure Rust (mp4parse) |
| NAL パース | H.264 SPS/PPS/Slice Header パース (h264-reader) |
| HW デコード | macOS/iOS (VideoToolbox), Windows (Media Foundation), Linux (Vulkan Video), Android (MediaCodec) |
| HW フォールバック | Linux: GStreamer + VA-API (cfg), V4L2 Stateless (cfg) |
| SW フォールバック | 全プラットフォーム CPU ソフトウェアデコード |
| GPU テクスチャ出力 | アプリ所有の GPU テクスチャにフレームを直接書き込み |
| 色変換 | NV12 → RGBA WGSL コンピュートシェーダ |
| 再生制御 | play, pause, seek, loop |
| パブリック API | `VideoSession` trait, `open()` エントリポイント |

### 1.2 Out of Scope (対象外)

| カテゴリ | 理由 |
|----------|------|
| 音声デコード | 本クレートはビデオのみ。音声は別クレートで対応 |
| エンコード | デコードのみ。録画/ストリーミングは別途 |
| ネットワークストリーム (HLS/DASH/RTMP) | ファイル入力のみ。ストリーミングは将来拡張 |
| WebM/MKV demux | Phase 1 は MP4 のみ。matroska 対応は将来 |
| H.265/VP9/AV1 デコード | Phase 1 は H.264 のみ。コーデック追加は段階的 |
| wgpu テクスチャの生成・管理 | アプリケーション側の責務 |
| UI / 再生コントロール | アプリケーション側の責務 |

---

## 2. 成果物

| # | 成果物 | 形式 | 説明 |
|---|--------|------|------|
| D1 | `crates/video-decoder/` | Rust crate | ライブラリ本体 |
| D2 | パブリック API ドキュメント | `cargo doc` | 全 pub 型・関数の rustdoc |
| D3 | 設計ドキュメント | `docs/design/video-decoder-crate-design.md` | E-R 図、シーケンス図、モジュール構成 |
| D4 | 本 SoW | `docs/design/video-decoder-sow.md` | 本ドキュメント |
| D5 | 結合テスト | `tests/` | SW フォールバックの E2E テスト |
| D6 | サンプルコード | `examples/` | wgpu 動画背景サンプル |

---

## 3. ワークパッケージ (WP)

### WP-1: 共通基盤

**目的:** `VideoSession` trait、demux、色変換パイプラインの構築

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| 1.1 | クレート scaffolding | Cargo.toml、lib.rs、モジュール構造の作成。workspace に追加 | `crates/video-decoder/Cargo.toml`, `src/lib.rs` | `cargo check -p video-decoder` が全ターゲットで通る |
| 1.2 | 型定義 | `VideoSession` trait, `OutputTarget`, `NativeHandle`, `VideoError`, `FrameStatus`, `SessionConfig`, `VideoInfo`, `Backend`, `Codec`, `PixelFormat`, `ColorSpace` | `src/lib.rs` | 設計書 §6.1 の全型が定義され、rustdoc が生成される |
| 1.3 | MP4 demuxer | `mp4parse` で MP4 → H.264 NAL unit ストリーム。`Demuxer` trait + `Mp4Demuxer` 実装 | `src/demux/mod.rs`, `src/demux/mp4.rs` | テスト用 MP4 (test_h264_360p.mp4) の全パケットが正しい PTS 順で取得できる |
| 1.4 | H.264 NAL パーサ | `h264-reader` で SPS/PPS/Slice Header を解析し、`CodecParameters` を構築 | `src/nal/mod.rs`, `src/nal/h264.rs` | SPS から width/height/profile/level が正しく抽出できる |
| 1.5 | NV12→RGBA 変換 | WGSL コンピュートシェーダ + `NV12ToRgbaPass` 構造体 | `src/convert/mod.rs`, `src/convert/nv12_to_rgba.wgsl`, `src/convert/color_space.rs` | 既知の NV12 テストデータが正しい RGBA に変換される (BT.709) |
| 1.6 | バックエンド選択 | `backend::create_session()` の分岐ロジック。`NativeHandle` 種別から自動選択 | `src/backend/mod.rs` | 各プラットフォームで正しいバックエンドが選択される |
| 1.7 | PlaybackState | 再生位置管理、フレームレート制御、ループ・seek のタイミング制御 | `src/util/timestamp.rs` | 30fps 動画で dt=16ms 入力時に正しいフレーム境界で NewFrame/Waiting を返す |
| 1.8 | ソフトウェアデコーダ | CPU デコード (`openh264`) → RGBA → `queue.write_texture()` | `src/backend/software.rs` | HW 非対応環境でも動画が再生できる (低解像度) |

**WP-1 完了基準:**
- `cargo check -p video-decoder` が通る
- `cargo test -p video-decoder` でソフトウェアデコーダの結合テストが通る
- `examples/decode_to_png.rs` が test_h264_360p.mp4 の先頭 10 フレームを PNG 出力する

---

### WP-2: macOS / iOS バックエンド (VideoToolbox)

**目的:** macOS/iOS で VideoToolbox → Metal ゼロコピーデコード

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| 2.1 | AVAssetReader 初期化 | objc2-av-foundation で AVAssetReader + TrackOutput 作成。duration/fps/codec 取得 | `src/backend/apple.rs` L1-100 | `session.info()` が正しいメタデータを返す |
| 2.2 | フレーム読み取り | `copyNextSampleBuffer()` → CVPixelBuffer 取得 | `src/backend/apple.rs` L100-180 | CVPixelBuffer が non-null で正しいサイズ |
| 2.3 | CVMetalTextureCache | CVPixelBuffer → MTLTexture マッピング (IOSurface ゼロコピー) | `src/backend/apple.rs` L180-240 | MTLTexture の pixelFormat = BGRA8Unorm、サイズが一致 |
| 2.4 | Metal blit | 一時 MTLTexture → OutputTarget の MTLTexture に GPU コピー | `src/backend/apple.rs` L240-300 | blit 後にテクスチャの内容が更新されている (readback で確認) |
| 2.5 | seek 実装 | AVAssetReader を timeRange 指定で再作成 | `src/backend/apple.rs` L300-360 | seek(5s) 後の次フレームの PTS が 5s 付近 |
| 2.6 | リソース解放 | Drop impl で AVAssetReader, CVMetalTextureCache, command queue を解放 | `src/backend/apple.rs` | Instruments で Metal リソースリークがないことを確認 |

**WP-2 完了基準:**
- macOS で `examples/wgpu_video_bg.rs` が MP4 を背景として 60fps 再生
- `decode_frame()` が平均 < 2ms (1080p, M1 以降)
- ループ再生が途切れなく動作

---

### WP-3: Windows バックエンド (Media Foundation)

**目的:** Windows で Media Foundation → D3D11/D3D12 interop デコード

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| 3.1 | MF 初期化 | `MFCreateSourceReaderFromURL` + D3D11 デバイスマネージャ設定 | `src/backend/media_foundation.rs` L1-120 | SourceReader が D3D11 HW アクセラレーション有効で作成される |
| 3.2 | フレーム読み取り | `ReadSample()` → IMFDXGIBuffer → ID3D11Texture2D | `src/backend/media_foundation.rs` L120-200 | テクスチャが DXGI_FORMAT_NV12 で正しいサイズ |
| 3.3 | NV12→RGBA 変換 | MF Video Processor MFT または WGSL compute shader で変換 | `src/backend/media_foundation.rs` L200-280 | 変換後の RGBA が視覚的に正しい |
| 3.4 | D3D11→D3D12 interop | DXGI shared handle で wgpu D3D12 テクスチャにコピー | `src/backend/media_foundation.rs` L280-360 | wgpu テクスチャが描画に使用できる |
| 3.5 | seek 実装 | `IMFSourceReader::SetCurrentPosition()` | `src/backend/media_foundation.rs` L360-400 | seek 後の PTS が正しい |
| 3.6 | COM 初期化/解放 | `CoInitializeEx` / `CoUninitialize`、IMF* オブジェクトの Release | `src/backend/media_foundation.rs` | COM リソースリークなし |

**WP-3 完了基準:**
- Windows 10/11 で MP4 が HW デコード + 60fps 再生
- NVIDIA / AMD / Intel GPU で動作確認

---

### WP-4: Linux バックエンド — Vulkan Video (優先パス)

**目的:** Vulkan Video Extensions で外部依存ゼロのデコード

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| 4.1 | ランタイム検出 | `VkQueueFlags::VIDEO_DECODE_KHR` チェック。非対応時はスキップ | `src/backend/vulkan_video.rs` L1-60 | 非対応ドライバでパニックしない |
| 4.2 | Video Session 作成 | `vkCreateVideoSessionKHR` + H.264 プロファイル設定 | `src/backend/vulkan_video.rs` L60-160 | Session が正しいパラメータで作成される |
| 4.3 | Session Parameters | SPS/PPS を `vkCreateVideoSessionParametersKHR` に渡す | `src/backend/vulkan_video.rs` L160-220 | h264-reader で抽出した SPS/PPS が正しく設定される |
| 4.4 | DPB 管理 | DPB スロット配列の確保、POC ベースの参照フレーム管理 | `src/backend/vulkan_video.rs` L220-340 | I/P/B フレームの参照関係が正しく解決される |
| 4.5 | デコードコマンド | `vkCmdDecodeVideoKHR` でフレームデコード | `src/backend/vulkan_video.rs` L340-420 | デコード出力 VkImage (NV12) が正しい画像データ |
| 4.6 | NV12→RGBA | WP-1.5 の `NV12ToRgbaPass` で変換 → OutputTarget に書き込み | `src/backend/vulkan_video.rs` L420-460 | 最終 RGBA テクスチャが正しい |
| 4.7 | seek | demuxer seek → DPB リセット → 次のキーフレームからデコード再開 | `src/backend/vulkan_video.rs` L460-520 | seek 後にアーティファクトなく再生継続 |

**WP-4 完了基準:**
- NVIDIA (535+) または Mesa 23.1+ で MP4 がデコードされる
- `detect_backends()` → `[VulkanVideo, ...]` が正しく返る
- 非対応ドライバでは自動的に次のバックエンドにフォールバック

---

### WP-5: Linux バックエンド — GStreamer VA-API (フォールバック)

**目的:** GStreamer 経由の VA-API デコード (cfg feature で有効化)

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| 5.1 | GStreamer パイプライン | `filesrc ! decodebin3 ! appsink` (DMA-BUF 出力) | `src/backend/gst_vaapi.rs` L1-80 | パイプラインが PLAYING 状態に遷移 |
| 5.2 | DMA-BUF 取得 | `gst_allocators::DmaBufMemory::fd()` | `src/backend/gst_vaapi.rs` L80-130 | 有効な fd が返る |
| 5.3 | Vulkan import | `VkImportMemoryFdInfoKHR` → temp VkImage | `src/backend/gst_vaapi.rs` L130-200 | import した VkImage が NV12 データとして読める |
| 5.4 | NV12→RGBA | NV12ToRgbaPass 経由で OutputTarget に書き込み | `src/backend/gst_vaapi.rs` L200-240 | RGBA 出力が正しい |
| 5.5 | seek | GStreamer seek event | `src/backend/gst_vaapi.rs` L240-280 | seek 後に正しいフレームが取得される |
| 5.6 | cfg 排除確認 | `--no-default-features` でコンパイル時に gst 関連コードが除外される | Cargo.toml | `feature = "gstreamer"` なしでビルド成功、gstreamer クレートの痕跡なし |

**WP-5 完了基準:**
- `cargo build -p video-decoder --features gstreamer` でビルド成功
- `cargo build -p video-decoder` (features なし) で GStreamer 依存がゼロ
- VA-API 対応 GPU で HW デコードされることを `vainfo` + ログで確認

---

### WP-6: Linux バックエンド — V4L2 Stateless (SBC)

**目的:** Raspberry Pi / Rockchip 等 SBC 向け V4L2 デコード

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| 6.1 | デバイス検出 | `/dev/video*` スキャン、`VIDIOC_QUERYCAP` で M2M + Stateless 確認 | `src/backend/v4l2.rs` L1-60 | 対応デバイスが見つかるか None |
| 6.2 | バッファ setup | `VIDIOC_REQBUFS` + `VIDIOC_QUERYBUF` (OUTPUT/CAPTURE キュー) | `src/backend/v4l2.rs` L60-140 | DMA-BUF export 可能なバッファが確保される |
| 6.3 | デコード | NAL submit (`VIDIOC_QBUF`) → `VIDIOC_DQBUF` でデコード済み取得 | `src/backend/v4l2.rs` L140-240 | デコード済みフレームが NV12 で正しい |
| 6.4 | DMA-BUF export | `VIDIOC_EXPBUF` → fd → Vulkan import | `src/backend/v4l2.rs` L240-300 | GStreamer VA-API と同じ Vulkan import パスが動作 |
| 6.5 | cfg 排除確認 | `feature = "v4l2"` なしでコンパイルから除外 | Cargo.toml | nix crate 依存がゼロ |

**WP-6 完了基準:**
- Raspberry Pi 4/5 で MP4 がデコードされる
- `cargo build -p video-decoder --features v4l2` でビルド成功

---

### WP-7: Android バックエンド (MediaCodec)

**目的:** Android NDK の MediaCodec → AHardwareBuffer → Vulkan デコード

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| 7.1 | MediaExtractor 初期化 | `AMediaExtractor_setDataSourceFd()` でファイルオープン | `src/backend/media_codec.rs` L1-60 | トラック情報が正しく取得される |
| 7.2 | MediaCodec 設定 | `AMediaCodec_configure()` (surface なし、バッファモード) | `src/backend/media_codec.rs` L60-120 | codec が started 状態 |
| 7.3 | デコードループ | input buffer submit → output buffer dequeue → AHardwareBuffer | `src/backend/media_codec.rs` L120-220 | AHardwareBuffer が non-null |
| 7.4 | Vulkan import | `VkImportAndroidHardwareBufferInfoANDROID` → VkImage | `src/backend/media_codec.rs` L220-300 | import した VkImage が正しいフォーマット |
| 7.5 | NV12→RGBA | NV12ToRgbaPass 経由 | `src/backend/media_codec.rs` L300-340 | RGBA 出力が正しい |
| 7.6 | リソース解放 | `AMediaCodec_delete`, `AMediaExtractor_delete` | `src/backend/media_codec.rs` | リソースリークなし |

**WP-7 完了基準:**
- Android 8.0+ (API 26) のデバイスで MP4 がデコードされる
- `cargo build -p video-decoder --target aarch64-linux-android` でビルド成功

---

### WP-8: テスト・サンプル・ドキュメント

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| 8.1 | テスト用動画 | 360p H.264 MP4 (10秒程度, CC0 ライセンス) | `tests/test_fixtures/test_h264_360p.mp4` | ファイルサイズ < 500KB |
| 8.2 | 結合テスト (SW) | ソフトウェアデコーダで open → decode → verify | `tests/integration_decode.rs` | `cargo test -p video-decoder` が CI で通る |
| 8.3 | 結合テスト (open) | メタデータ取得テスト | `tests/integration_open.rs` | codec, width, height, fps, duration が正しい |
| 8.4 | サンプル: decode_to_png | 動画 → 連番 PNG (HW 不要) | `examples/decode_to_png.rs` | 実行して正しい PNG が出力される |
| 8.5 | サンプル: wgpu_video_bg | wgpu ウィンドウに動画背景 | `examples/wgpu_video_bg.rs` | ウィンドウに動画が表示される |
| 8.6 | rustdoc | 全 pub API に doc comment | 全 src ファイル | `cargo doc -p video-decoder --no-deps` が警告なし |
| 8.7 | ベンチマーク | 1080p MP4 のデコードスループット | `benches/decode_throughput.rs` | `cargo bench -p video-decoder` が動作 |

---

## 4. WP 依存関係

```
WP-1 (共通基盤)
  ├──→ WP-2 (macOS/iOS)      ← WP-1.2, 1.6 に依存
  ├──→ WP-3 (Windows)        ← WP-1.2, 1.5, 1.6 に依存
  ├──→ WP-4 (Linux VkVideo)  ← WP-1.2, 1.3, 1.4, 1.5, 1.6, 1.7 に依存
  ├──→ WP-5 (Linux GStreamer) ← WP-1.2, 1.5, 1.6 に依存
  ├──→ WP-6 (Linux V4L2)     ← WP-1.2, 1.3, 1.4, 1.5, 1.6 に依存
  ├──→ WP-7 (Android)        ← WP-1.2, 1.5, 1.6 に依存
  └──→ WP-8 (テスト/ドキュメント) ← WP-1 完了後から随時

  WP-4, WP-6 は demux/NAL パース (WP-1.3, 1.4) を共有
  WP-4, WP-5, WP-6 は DMA-BUF→Vulkan import コードを共有
  WP-3, WP-4, WP-5, WP-6, WP-7 は NV12→RGBA (WP-1.5) を共有
```

## 5. 並行実施可能な WP

以下の WP は WP-1 完了後に並行して実施可能:

```
             WP-1 (共通基盤)
               │
    ┌──────────┼──────────┬──────────┬──────────┐
    ▼          ▼          ▼          ▼          ▼
  WP-2      WP-3      WP-4       WP-7      WP-8
 (macOS)   (Win)    (Linux/Vk)  (Android) (テスト)
                       │
                  ┌────┤
                  ▼    ▼
                WP-5  WP-6
               (GSt)  (V4L2)
```

- WP-2, WP-3, WP-4, WP-7 は互いに独立、並行可能
- WP-5, WP-6 は WP-4 の Vulkan import コードに依存するため、WP-4 の後

---

## 6. 品質基準

| 項目 | 基準 |
|------|------|
| コンパイル | `cargo check -p video-decoder` が全ターゲットで通る |
| lint | `cargo clippy -p video-decoder -- -D warnings` が通る |
| フォーマット | `cargo fmt -p video-decoder --check` が通る |
| テスト | `cargo test -p video-decoder` が通る (SW fallback テスト) |
| ドキュメント | `cargo doc -p video-decoder --no-deps` が警告なし |
| cfg 分離 | 各 feature flag の有無でコンパイルが通る (`--no-default-features`, `--all-features`) |
| リソースリーク | 各バックエンドの Drop が全ネイティブリソースを解放 |
| パニック安全 | `decode_frame()` は Result を返す。パニックしない |
| スレッド安全 | `VideoSession: Send` が成立 |

---

## 7. 技術的リスクと対策

| # | リスク | 影響 | 対策 |
|---|--------|------|------|
| R1 | wgpu HAL API の breaking change | ネイティブテクスチャ interop が壊れる | HAL 版とCPU upload 版の両パスを維持。wgpu バージョンを固定 |
| R2 | Vulkan Video ドライバのバグ | デコード結果が不正 | GStreamer VA-API フォールバックで回避。ドライバ最小バージョンを明記 |
| R3 | objc2-av-foundation の API カバレッジ不足 | AVAssetReader の一部 API がバインディングにない | `objc2::msg_send!` マクロで直接 Obj-C メッセージ送信にフォールバック |
| R4 | D3D11→D3D12 shared handle の互換性 | 一部 GPU/ドライバで shared handle が動かない | MF の RGB32 出力 + CPU コピーのフォールバック |
| R5 | ndk media crate のカバレッジ不足 | AMediaCodec の一部 API がバインディングにない | `ndk-sys` 経由で C API を直接 FFI |
| R6 | H.264 の複雑なプロファイル | B フレーム参照・MBAFF 等で DPB 管理が複雑 | Phase 1 は Baseline/Main Profile に限定。High Profile は段階的 |
| R7 | GStreamer バージョン差異 | ディストリ間で GStreamer プラグインの有無が異なる | ランタイムでパイプライン構築失敗を検出しフォールバック |

---

## 8. 将来拡張 (本 SoW のスコープ外)

| 拡張 | 概要 | 前提 |
|------|------|------|
| WebM/MKV demux | matroska コンテナ対応 | WP-1 完了 |
| H.265 (HEVC) | NAL パーサ + Decoder 拡張 | WP-1.4 の h265.rs |
| VP9 / AV1 | Vulkan Video `VK_KHR_video_decode_av1` 等 | WP-4 完了 |
| 音声同期再生 | 動画 + 音声の同期 (PTS ベース) | 別クレート |
| ネットワーク入力 | HLS/DASH/RTMP | demuxer 拡張 |
| エンコード | GPU エンコード (Vulkan Video Encode) | 別クレート |
| WASM | WebCodecs API | wgpu webgpu バックエンド対応後 |
