# Step 1.1 Troubleshooting

## エラー: wgpu `expose-ids` feature not found

### エラー内容
```
error: failed to select a version for `wgpu`.
package `video-decoder` depends on `wgpu` with feature `expose-ids` but `wgpu` does not have that feature.
```

### 原因
設計書 (`video-decoder-crate-design.md`) の Cargo.toml 例では `wgpu = { version = "24.0", features = ["expose-ids"] }` を記載していたが、wgpu 24.0 には `expose-ids` feature が存在しない。この feature は wgpu の古いバージョン (0.x 系) に存在していた。

### 解決
`wgpu = { workspace = true }` に変更し、ワークスペースの wgpu 設定 (`wgpu = "24.0"`) を継承するよう修正。

### 影響
設計書の Cargo.toml 例は参考として残すが、実装では workspace 継承を使用する。将来 wgpu HAL interop が必要になった場合は、適切な feature flag を調査して追加する。
