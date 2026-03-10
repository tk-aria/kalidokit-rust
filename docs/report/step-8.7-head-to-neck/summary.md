# Step 8.7: Head → Neck 適用先の修正

## 完了日時
2026-03-10 13:02 JST

## 変更ファイル
- `crates/app/src/update.rs` — `HumanoidBoneName::Head` → `HumanoidBoneName::Neck` に変更

## 実行コマンド
```bash
cargo check --workspace        # OK
```

## 実装内容
1. face solver の head rotation を Neck ボーンに適用 (testbed の `rigRotation("Neck", ...)` に合わせる)
2. dampener=0.7, lerp=0.3 は既に cfg.neck を使用していたため変更なし
