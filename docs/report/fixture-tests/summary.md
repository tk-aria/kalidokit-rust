# Fixture-dependent Tests — 作業報告

## 実行日時
2026-03-17 21:13-21:20 JST

## 追加テスト (10 tests)

### demux/mp4.rs (4 tests)
- open_big_buck_bunny_metadata: codec, dimensions, fps, duration
- packets_are_dts_ordered: DTS monotonicity
- first_packet_is_keyframe: sync sample check
- seek_resets_position: seek + keyframe landing

### nal/h264.rs (1 test)
- h264_context_from_fixture: avcC parse → width/height

### backend/software.rs (2 tests)
- sw_decode_10_frames: decode + frame_rgba check
- sw_pause_resume: pause/resume lifecycle

### backend/apple.rs (1 test)
- apple_decode_10_frames: VideoToolbox decode + frame_rgba

### integration tests (2 tests)
- open_valid_mp4_returns_session
- decode_10_frames_sw

## 結果
- 75 tests total (was 65), all pass
- clippy clean, fmt clean

## 実行コマンド
```
# subagent で 10 テスト追加
cargo test -p video-decoder     # 75 tests pass
cargo clippy -p video-decoder -- -D warnings
cargo fmt -p video-decoder
```
