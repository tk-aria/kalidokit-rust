# Step 1.2: 音声前処理 audio.rs

## 実行日時
2026-03-21 11:28 JST

## 実装内容
- `i16_to_f32()` — i16 PCM → f32 [-1.0, 1.0]
- `truncate_or_pad()` — 8秒にトランケート/先頭ゼロパディング

## テスト結果
9 tests passed (7 normal + 2 edge cases)

## 実行コマンド
```bash
cargo check -p etd  # OK
cargo test -p etd   # 9 tests passed
```
