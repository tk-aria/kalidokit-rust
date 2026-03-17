# Phase 4: ソフトウェアデコーダ + バックエンド選択 — 作業報告

## 実行日時
2026-03-17 12:37-12:44 JST

## 完了タスク

### Step 4.1: Cargo.toml にソフトウェアデコーダ依存追加
- `openh264 = "0.6"` (0.6.6) 追加、source feature でバンドル

### Step 4.2: ソフトウェアデコーダ (backend/software.rs, ~200行)
- SwVideoSession: demuxer + openh264 Decoder + RGBA frame buffer
- decode_frame: pause check → tick → end-of-stream check → demux → decode → write_rgba8
- seek: demuxer.seek() + decoder 再作成
- ループ再生: seek(0) + decoder 再作成
- frame_rgba() アクセサで CPU 側 RGBA バッファ取得

### Step 4.3: バックエンド選択 (backend/mod.rs)
- create_session(): ファイル存在チェック → preferred_backend → auto-detect → SW fallback
- detect_backends(): NativeHandle 種別ごとに候補リスト生成
- HW バックエンドは NoHwDecoder を返す (未実装)

### Step 4.4: lib.rs の open() 接続
- Phase 1 で既に接続済み

### Step 4.5: テスト
- 異常系: 破損 MP4 → Demux エラー、allow_software_fallback=false → NoHwDecoder
- 正常系: テストフィクスチャ (MP4) 未追加のため保留

### Step 4.7: Phase 4 検証
- 42 tests + 1 doctest all pass
- clippy 0 warnings, fmt OK

## 実行コマンド
```
# Cargo.toml に openh264 追加
cargo check -p video-decoder
# subagent で software.rs, backend/mod.rs 実装
cargo test -p video-decoder     # 42 tests pass
cargo clippy -p video-decoder -- -D warnings
cargo fmt -p video-decoder
```

## 未完了項目
- テスト用 MP4 フィクスチャ作成
- decode_to_png example 実装
- 正常系テスト (フィクスチャ依存)
- E2E 動作確認
