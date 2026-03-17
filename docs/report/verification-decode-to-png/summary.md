# 動作確認: decode_to_png example — 作業報告

## 実行日時
2026-03-17 19:10-19:15 JST

## 手順

### 1. テスト動画ダウンロード
- Big Buck Bunny 360p 10秒 H.264 MP4 (968KB)
- URL: https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/360/Big_Buck_Bunny_360_10s_1MB.mp4
- 保存先: `tests/fixtures/big_buck_bunny_360p.mp4`

### 2. 初回実行 — エラー発生
```
Error: decode error: openh264 decode error: Native:4
```
原因: MP4 demuxer がサンプルデータを AVCC (length-prefixed) 形式のまま返していたが、
openh264 は Annex B (start-code: 00 00 00 01) 形式を期待。

### 3. 修正: AVCC → Annex B 変換追加
- `demux/mp4.rs` に `avcc_to_annex_b()` 関数を追加
- `annex_b_from_avcc_extra()` で avcC の SPS/PPS をキーフレーム先頭に挿入
- `next_packet()` 内で変換を実行

### 4. 再実行 — 成功
```
Video: 640x360, 30.0 fps, 10.0s, codec: H264
Wrote /tmp/vd_frames/frame_0000.png
...
Wrote /tmp/vd_frames/frame_0009.png
Decoded 10 frames to /tmp/vd_frames/
```

### 5. 出力確認
- 10 フレーム PNG (各 510-532KB, 640x360 RGBA)
- Big Buck Bunny の正しい映像フレーム (色・解像度・フレーム順が正常)

## 実行コマンド
```
curl -L -o tests/fixtures/big_buck_bunny_360p.mp4 "https://test-videos.co.uk/..."
ffprobe tests/fixtures/big_buck_bunny_360p.mp4  # h264, 640x360, 30fps, 10s
cargo run -p video-decoder --example decode_to_png -- tests/fixtures/big_buck_bunny_360p.mp4 /tmp/vd_frames
cargo test -p video-decoder   # 65 tests pass
cargo clippy -p video-decoder -- -D warnings  # clean
```

## トラブルシューティング
エラー: openh264 decode error (AVCC→Annex B 変換未実装)
→ demux/mp4.rs に avcc_to_annex_b() + annex_b_from_avcc_extra() を追加して解決
