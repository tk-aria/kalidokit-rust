# Step 21.7 Troubleshooting: Whisper abort_callback + Metal crash

## 問題: abort_callback が Metal crash を引き起こす

### 症状
- `set_abort_callback_safe` を有効にすると `whisper_full_with_state: failed to encode` でクラッシュ
- state を毎回作成しても、使い回しても crash は同じ
- abort_callback を無効にすると正常動作

### 検証結果

| 構成 | state | abort_callback | 結果 |
|---|---|---|---|
| 旧 (毎回create) | 毎回 create_state() | 有効 | crash (failed to encode) |
| 方式3 (使い回し) | RefCell で保持 | 有効 | crash (failed to encode) |
| 方式3 (使い回し) | RefCell で保持 | **無効** | **安定動作** |

### 原因
whisper-rs 0.16 の `set_abort_callback_safe` が Metal backend の encode ステップ中に Rust クロージャを呼び出すことで Metal の内部状態が破壊される。whisper.cpp 側の問題の可能性。

### 方式3 (state 使い回し) の効果
- Metal init: 1 回のみ (起動時)
- Metal free: 0 回 (アプリ終了まで)
- crash なし (abort_callback 無効時)
- 期待: レイテンシ改善 (Metal init/free のオーバーヘッド削減)

### 対応
- abort_callback は無効のまま
- state 使い回しは有効 (安定性 + パフォーマンス向上)
- ハートビートは推論前後のタイムスタンプ更新で代替
