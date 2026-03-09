# SpringBone VrmModel Integration - 作業報告

## タスク
features.md line 1160: `[ ] VrmModel への統合`

## 実施内容

### 1. VrmModel にフィールド追加
- `crates/vrm/src/model.rs`: `spring_bone_groups: Vec<SpringBoneGroup>` フィールド追加

### 2. Loader でパース
- `crates/vrm/src/loader.rs`: `SpringBoneGroup::from_vrm_json(&vrm_json)?` を呼び出し、VRM JSON の `secondaryAnimation.boneGroups` をパース。存在しない場合は空 Vec。

### 3. Update ループで呼び出し
- `crates/app/src/update.rs`: `apply_rig_to_model` の後、GPU バッファ更新の前に `group.update(delta_time, glam::Vec3::ZERO)` を各グループに対して呼び出し

## 実行コマンド
- `cargo check --workspace` — コンパイル成功

## 結果
`[x]` に更新。SpringBone がデッドコードではなくなり、VrmModel に統合完了。
