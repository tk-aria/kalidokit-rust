# Step 1.4: STFT 実装

## 実行日時
2026-03-21 11:31 JST

## 実装内容
- `hann_window()` — Hann 窓関数生成
- `stft_power()` — rustfft を使った STFT パワースペクトル計算

## テスト結果
7 tests passed (5 normal + 2 edge cases)

## 実行コマンド
```bash
cargo check -p etd  # OK
cargo test -p etd   # 23 tests passed
```
