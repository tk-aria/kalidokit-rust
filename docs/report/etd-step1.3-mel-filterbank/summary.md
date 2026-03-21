# Step 1.3: Mel フィルタバンク

## 実行日時
2026-03-21 11:28 JST

## 実装内容
- `MelConfig` struct (Whisper互換デフォルト)
- `hz_to_mel()` / `mel_to_hz()` — HTK式
- `mel_filterbank()` — 三角フィルタバンク生成 (80 × 201)

## テスト結果
7 tests passed (5 normal + 2 edge cases)

## 実行コマンド
```bash
cargo check -p etd  # OK
cargo test -p etd   # 16 tests total passed
```
