# GpuTexture Scene/Pipeline Integration - 作業報告

## タスク
features.md line 708: `[ ] Scene/パイプラインへの統合`

## 実施内容

### 1. VRM マテリアル/テクスチャローディング
- `crates/vrm/src/model.rs`: `Material` 構造体追加 (`base_color`, `base_color_texture`)、`VrmModel` に `materials` フィールド追加、`MeshData` に `material_index` 追加
- `crates/vrm/src/loader.rs`: glTF マテリアル解析 (PBR baseColorFactor, baseColorTexture)、GLB 埋め込み画像ロード
- `crates/vrm/Cargo.toml`: `image` ワークスペース依存追加

### 2. GPU マテリアルデータ
- `crates/renderer/src/scene.rs`:
  - `MaterialUniform` (repr(C), Pod) 追加
  - `GpuMaterial` 構造体 (bind_group) 追加
  - `MeshMaterialInput` 公開構造体追加
  - `Scene::new()` に `queue` と `mesh_materials` パラメータ追加
  - マテリアル bind group layout (group 3): uniform buffer + texture + sampler
  - メッシュごとに GpuTexture 作成 (fallback: default_white)
  - `Scene::render()` でメッシュごとにマテリアル bind group をバインド

### 3. シェーダー更新
- `assets/shaders/skinning.wgsl`: MaterialUniform, テクスチャ/サンプラーバインディング追加、フラグメントシェーダーでテクスチャサンプリング + base_color 乗算

### 4. App 統合
- `crates/app/src/init.rs`: `MeshMaterialInput` を VRM マテリアルから構築、更新された `Scene::new()` シグネチャに対応

## 実行コマンド
- `cargo check --workspace` — コンパイル成功
- `cargo test -p renderer -p vrm -p solver` — 全71テスト通過

## 結果
`[x]` に更新。GpuTexture が Scene/パイプラインに統合完了。
