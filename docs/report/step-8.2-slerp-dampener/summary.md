# Step 8.2: slerp / dampener 補間の適用

## 完了日時
2026-03-10 12:54 JST

## 変更ファイル
- `crates/vrm/src/bone.rs` — `prev_rotations` フィールド追加、`set_rotation_interpolated()` メソッド追加、テスト追加
- `crates/app/src/state.rs` — `AppState` に `rig_config: RigConfig` フィールド追加
- `crates/app/src/init.rs` — `init_all()` で `rig_config: RigConfig::default()` 初期化
- `crates/app/src/update.rs` — `apply_rig_to_model()` 全 `set_rotation()` を `set_rotation_interpolated()` に置換
- `crates/app/src/rig_config.rs` — `#![allow(dead_code)]` 削除

## 実行コマンド
```bash
cargo check --workspace        # OK (RigConfig 未使用フィールド warning のみ)
cargo test -p vrm              # 28 tests passed
cargo test -p solver           # 23 tests passed
cargo test -p renderer         # 10 tests passed (tracker は ort-sys 制約で除外)
```

## 実装内容
1. `HumanoidBones::prev_rotations: HashMap<HumanoidBoneName, Quat>` 追加
2. `set_rotation_interpolated(name, target, dampener, lerp_amount)`:
   - dampener: IDENTITY → target を slerp(dampener) で減衰
   - lerp_amount: 前フレーム → dampened を slerp(lerp_amount) で補間
3. dampener 値を testbed と完全一致:
   - Neck=0.7, Hips=0.7, Chest=0.25, Spine=0.45, Limbs=1.0 (全て lerp=0.3)
4. テスト: `slerp_interpolation_produces_intermediate_value` 追加
