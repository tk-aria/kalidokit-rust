# Step 8.4: Hip position 適用

## 完了日時
2026-03-10 12:58 JST

## 変更ファイル
- `crates/vrm/src/bone.rs` — `set_position()`, `set_position_interpolated()`, `prev_positions` 追加、`compute_joint_matrices()` でボーン position 反映
- `crates/app/src/update.rs` — `let _ = hip_pos;` → `set_position_interpolated()` に変更

## 実行コマンド
```bash
cargo check --workspace        # OK
cargo test -p vrm              # 29 tests passed
```

## 実装内容
1. `prev_positions: HashMap<HumanoidBoneName, Vec3>` フィールド追加
2. `set_position(name, Vec3)` メソッド追加
3. `set_position_interpolated(name, target, dampener, lerp_amount)` メソッド追加
4. `compute_joint_matrices()`: `bone_positions` ルックアップ追加、translation をボーン位置で上書き
5. `apply_rig_to_model()`: hip_pos を `set_position_interpolated(Hips, hip_pos, 1.0, 0.07)` で適用
6. テスト: `set_position_applied_in_joint_matrices` 追加
