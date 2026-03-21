# Step 2.1: ONNX モデル取得

## 実行日時
2026-03-21 11:37 JST

## 実行コマンド
```bash
curl -L -o assets/models/smart_turn_v3.onnx \
  "https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/main/smart-turn-v3.2-gpu.onnx"

# 入力/出力形状確認
python3 -c "import onnxruntime as ort; ..."
```

## 確認結果
- モデルサイズ: 31MB (FP32)
- Input: `input_features` shape (batch, 80, 800) float32
- Output: `logits` shape (batch, 1) float32
- ルート .gitignore で *.onnx 除外済み → git 管理外
