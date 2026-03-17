# Phase 3: NV12→RGBA 色変換 + PlaybackState — 作業報告

## 実行日時
2026-03-17 12:30-12:37 JST

## 完了タスク

### Step 3.1: WGSL シェーダ
- `shaders/nv12_to_rgba.wgsl` 作成 (BT.709 coefficients, @workgroup_size(8,8))

### Step 3.2: 色空間パラメータ
- `convert/color_space.rs` — ColorMatrix struct + bt709() / bt601()

### Step 3.3: NV12ToRgbaPass
- `convert/mod.rs` — wgpu ComputePipeline + BindGroupLayout (Y tex, UV tex, RGBA storage)
- new() + convert() 実装

### Step 3.4: PlaybackState
- `util/timestamp.rs` — tick(), check_end_of_stream(), pause/resume/seek
- 11 unit tests (30fps timing, loop, pause, fps=0, duration=0)

### Step 3.5: DpbManager
- `util/ring_buffer.rs` — allocate/release/eviction/reset/get_reference_indices
- 7 unit tests (alloc cycle, references, reset, eviction, no-op release)

### Step 3.6: テスト
- PlaybackState: 11 tests (正常系 + 異常系)
- DpbManager: 7 tests (正常系 + 異常系)
- NV12ToRgbaPass: ヘッドレス環境のためスキップ

### Step 3.7: Phase 3 検証
- 38 tests + 1 doctest all pass
- clippy 0 warnings, fmt OK, doc OK

## 実行コマンド
```
# subagent で全ファイル実装
cargo check -p video-decoder
cargo test -p video-decoder     # 38 tests pass
cargo clippy -p video-decoder -- -D warnings
cargo fmt -p video-decoder
```
