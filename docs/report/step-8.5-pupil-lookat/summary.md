# Step 8.5: Pupil (瞳孔) + LookAt 適用

## 完了日時
2026-03-10 13:02 JST

## 変更ファイル
- `crates/vrm/src/model.rs` — `VrmModel` に `look_at: Option<LookAtApplyer>` 追加
- `crates/vrm/src/loader.rs` — `LookAtApplyer::from_vrm_json()` でパース、エラー時 None
- `crates/app/src/state.rs` — `RigState` に `prev_look_target: Vec2` 追加
- `crates/app/src/update.rs` — `apply_rig_to_model()` に pupil tracking 追加 (lerp=0.4、LeftEye/RightEye ボーン適用)
- `crates/vrm/src/look_at.rs` — テスト追加

## 実行コマンド
```bash
cargo check --workspace        # OK
cargo test -p vrm              # 30 tests passed
```

## 実装内容
1. VrmModel に LookAtApplyer フィールド追加 + loader でパース
2. prev_look_target で前フレーム値を保持
3. pupil Vec2 → lerp(0.4) 補間 → EulerAngles(yaw, pitch) * 30度 → LookAt::apply() → Quat
4. LeftEye / RightEye ボーンに set_rotation_interpolated() で適用
5. テスト: nonzero_pupil_produces_non_identity_rotation
