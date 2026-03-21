# Phase 1: wgpu 24→29 アップグレード — 作業報告

## 実行日時
2026-03-21 11:25-11:40 JST

## 完了タスク
- wgpu 24.0 → 29.0 にアップグレード
- 7 ファイルの API 差分を修正 (22 エラー)
- 全テスト pass (renderer 16, vrm 33, solver 39, video-decoder 68)
- clippy 0 warnings (mascot.rs dead code に #![allow] 追加)
- Release ビルド成功、30 fps で安定動作 (wgpu 24 時の 23 fps から改善)

## 修正した wgpu API 変更
| 変更 | 修正箇所 |
|------|----------|
| Instance::new() | context.rs |
| request_adapter() → Result | context.rs |
| request_device(desc, None) → request_device(desc) | context.rs |
| Maintain::Wait → PollType::wait_indefinitely() | context.rs, scene.rs |
| push_constant_ranges → 削除, immediate_size: 0 追加 | pipeline.rs, scene.rs, debug_overlay.rs, convert/mod.rs |
| multiview: None → multiview_mask: None | pipeline.rs, scene.rs, debug_overlay.rs |
| depth_slice: None 追加 | scene.rs, debug_overlay.rs |
| depth_write_enabled: bool → Option<bool> | pipeline.rs |
| mipmap_filter → MipmapFilterMode | texture.rs |
| get_current_texture() → match SurfaceTextureStatus | scene.rs, update.rs |
