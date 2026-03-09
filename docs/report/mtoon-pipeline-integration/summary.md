# MToon Pipeline Integration - 作業報告

## タスク
features.md line 1196: `[ ] レンダーパイプラインへの統合`

## 実施内容

### 1. skinning.wgsl に MToon シェーディング統合
- MaterialUniform に shade_color, rim_color, mtoon_params (shade_shift, shade_toony, rim_power, rim_lift) 追加
- VertexOutput に world_pos 追加（リムライト計算用）
- フラグメントシェーダー: smoothstep による2段階トゥーンシェーディング + Fresnel リムライト

### 2. Rust 側の MaterialUniform 更新
- `scene.rs`: MaterialUniform に shade_color, rim_color, mtoon_params 追加
- `scene.rs`: MeshMaterialInput にMToonパラメータ追加（デフォルト値付き）

### 3. VRM MToon 拡張パース
- `loader.rs`: `apply_vrm_mtoon_properties()` 追加。VRM 0.x JSON の materialProperties から _ShadeColor, _ShadeShift, _ShadeToony, _RimColor 等を抽出
- `model.rs`: Material 構造体に MToon フィールド追加

### 4. App 統合
- `init.rs`: VRM Material → MeshMaterialInput の MToon パラメータパススルー

## 実行コマンド
- `cargo check --workspace` — コンパイル成功

## 結果
`[x]` に更新。MToon シェーダーがパイプラインに統合完了。
