# Phase 2: Demux + NAL パーサ — 作業報告

## 実行日時
2026-03-17 12:19-12:30 JST

## 完了タスク

### Step 2.1: Cargo.toml に demux 依存追加
- `mp4parse = "0.17"` と `h264-reader = "0.7"` を追加
- `cargo check -p video-decoder` 成功

### Step 2.2: Demuxer trait (demux/mod.rs)
- Demuxer trait, VideoPacket, CodecParameters は Phase 1 で定義済み
- `create_demuxer()` ファクトリ関数を追加 (.mp4/.m4v/.mov 対応)

### Step 2.3: MP4 Demuxer (demux/mp4.rs, ~280行)
- `Mp4Demuxer` struct: mp4parse で moov 解析 + 別の seekable fd でサンプルデータ読み取り
- stbl ボックス (stco, stsz, stsc, stss, stts, ctts) からサンプルテーブルをフラットに構築
- next_packet(): ファイル offset + サイズでサンプルデータ読み取り、NAL length prefix → Annex B 変換
- seek(): sync sample テーブルから最寄りのキーフレームを検索

### Step 2.4: NAL パーサ (nal/h264.rs)
- H264Context struct: AvcDecoderConfigurationRecord からSPS/PPS 抽出
- from_avcc() で avcC バイト列をパース
- pixel_dimensions() で SPS から width/height 取得

### Step 2.5: テスト
- 異常系テスト: 非対応拡張子、空ファイル、テキストファイル → エラー
- 正常系テスト: テストフィクスチャ (MP4ファイル) 未追加のため保留

### Step 2.6: Phase 2 検証
- 20 tests + 1 doctest all pass
- clippy 0 warnings, fmt OK

## 実行コマンド
```
# Cargo.toml に mp4parse + h264-reader 追加
cargo check -p video-decoder  # 成功
# subagent で mp4.rs, h264.rs 実装
cargo test -p video-decoder   # 20 tests pass
cargo clippy -p video-decoder -- -D warnings  # OK
cargo fmt -p video-decoder
```

## 未完了項目
- テスト用フィクスチャ (test_h264_360p.mp4) の作成
- フィクスチャ依存の正常系テスト
- テストカバレッジ計測 (cargo-llvm-cov 未インストール)
