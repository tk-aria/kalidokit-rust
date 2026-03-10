# Step 8.6: Face blink 補間 + stabilizeBlink

## 完了日時
2026-03-10 13:02 JST

## 変更ファイル
- `crates/app/src/state.rs` — `RigState` に `prev_blink_l: f32`, `prev_blink_r: f32` 追加
- `crates/app/src/update.rs` — blink 処理を改善: lerp(0.5) 補間 + stabilizeBlink 呼び出し + 左右同値

## 実行コマンド
```bash
cargo check --workspace        # OK
cargo test -p solver -p vrm -p renderer  # 63 tests passed
```

## 実装内容
1. prev_blink_l/r で前フレームの BlinkL/R 値を保持
2. `(1.0 - eye.l).clamp(0.0, 1.0)` → `lerp(prev, current, 0.5)` で補間
3. `solver::face::stabilize_blink()` を blink 設定前に呼び出し
4. BlinkL = BlinkR = stabilized.l (testbed と同じ左右同値)
5. テスト: blink_values_are_interpolated
