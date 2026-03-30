# Step 21.8 Troubleshooting: ETD 有効化時の問題

## 問題 1: ETD probability が固定値

### 症状
mel 前処理を Python WhisperFeatureExtractor と数値一致させた (max diff < 1e-5) にもかかわらず、ETD の probability が 0.68〜0.72 の狭い範囲に固定。発話内容・長さに関わらず変動しない。

### 調査済み
- mel スペクトログラムの正規化: `(x + 4.0) / 4.0` (Whisper 互換) ✓
- mel filterbank: slaney scale + slaney normalization ✓
- fmin=0, fmax=8000 ✓
- FFT 精度: f64 ✓
- center padding: reflect pad + last frame drop ✓
- Hann 窓: periodic ✓

### 推定原因
- smart-turn v3 モデルが現在の音声環境 (内蔵マイク + denoise) に対してキャリブレーションされていない
- モデルの学習データとの分布不一致 (domain mismatch)

### 次のアクション
- Python で同じ音声を smart-turn v3 に直接入力して probability を確認
- Rust と Python の ONNX 推論出力を比較 (前処理は一致済みなので推論結果も一致するはず)

## 問題 2: Whisper Metal crash

### 症状
ETD 有効時に `whisper_full_with_state: failed to encode` + `ggml_metal_free: deallocating` でアプリがクラッシュ。

### 推定原因
- ETD (ONNX Runtime) と Whisper (Metal/ggml) が同時に GPU リソースを使用
- Metal のメモリ管理が競合

### 対処
- ETD を無効化して回避中
