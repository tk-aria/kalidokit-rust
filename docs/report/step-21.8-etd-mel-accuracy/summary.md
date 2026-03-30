# Step 21.8: ETD mel accuracy — Python WhisperFeatureExtractor 数値一致検証

## 概要
Rust ETD の mel スペクトログラム前処理を Python の WhisperFeatureExtractor と数値比較し、一致を検証。

## 発見・修正した 3 つのバグ

### 1. Center padding の欠落
- **Python**: `np.pad(waveform, (n_fft//2, n_fft//2), mode='reflect')` で反射パディング後に STFT
- **Rust (旧)**: パディングなしで直接 STFT
- **修正**: `reflect_pad()` 関数を追加、STFT 前に n_fft/2=200 サンプルの反射パディングを適用

### 2. Hann 窓の種類 (symmetric vs periodic)
- **Python**: `window_function(N, "hann", periodic=True)` → `cos(2*pi*n/N)` (periodic)
- **Rust (旧)**: `cos(2*pi*n/(N-1))` (symmetric)
- **修正**: periodic Hann に変更

### 3. f32 vs f64 精度
- **Python**: numpy は FFT を float64 で実行
- **Rust (旧)**: f32 で FFT → mel filterbank 適用
- **修正**: FFT を f64 (`FftPlanner::<f64>`) で実行、mel filterbank も f64 で構築

## テスト結果

| テストケース | Max absolute diff |
|---|---|
| Silence (8s) | 0.000000 |
| 440Hz Sine (8s) | 0.000004 |
| Speech-like (200+400Hz, 2s) | 0.000006 |

全テスト tolerance 1e-3 で合格。実際の差異は 1e-5 以下。

## 実行コマンド
```bash
# Python テストフィクスチャ生成
python3 -m venv /tmp/etd-venv
/tmp/etd-venv/bin/pip install transformers numpy
/tmp/etd-venv/bin/python3 scripts/generate_fixtures.py

# Rust テスト実行
cargo test -p etd -- mel_accuracy --nocapture

# 全テスト
cargo test -p etd
```

## 変更ファイル
- `crates/etd/src/mel.rs` — reflect_pad(), f64 filterbank, f64 mel 計算
- `crates/etd/src/stft.rs` — periodic Hann, f64 FFT
- `crates/etd/tests/mel_accuracy.rs` — 3 テストケース追加
- `.gitignore` — `*.npy`, `*.raw` 追加

## 完了時刻
2026-03-30T19:45:00+09:00
