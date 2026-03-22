# wgpu バージョン制約レポート

## 現状

- **workspace wgpu**: 28.0
- **経緯**: wgpu 24 → 29 にアップグレード後、dear-imgui-wgpu 0.10 との互換性問題により 29 → 28 にダウングレード

## ボトルネック

`dear-imgui-wgpu 0.10` が wgpu 28 に固定依存しており、wgpu 29 と互換性がない。

```
dear-imgui-rs 0.10 ecosystem:
  dear-imgui-wgpu 0.10 → wgpu "28.0"  ← ボトルネック
  dear-imgui-winit 0.10 → winit "0.30"
  dear-imnodes 0.10
```

## wgpu 28 → 29 の破壊的変更

| wgpu 28 | wgpu 29 | 影響 |
|---------|---------|------|
| `PipelineLayoutDescriptor { immediate_size }` | `push_constant_ranges` に変更 | パイプラインレイアウト作成 |
| `multiview_mask: Option<...>` | `multiview: Option<...>` | レンダーパイプライン記述子 |
| `RenderPassColorAttachment { depth_slice }` | フィールド削除 | カラーアタッチメント |
| `surface.get_current_texture()` → `Result` | `CurrentSurfaceTexture` 型に変更 | サーフェステクスチャ取得 |
| `bind_group_layouts: Option` unwrapping | 直接参照 | バインドグループ |
| `depth_write_enabled: bool` | `Option<bool>` に変更 | デプスステンシル |

## 影響を受けるクレート

| クレート | wgpu 使用 | 備考 |
|---------|----------|------|
| `renderer` | 直接 | 3D シーン描画、パイプライン |
| `imgui-renderer` | `dear-imgui-wgpu` 経由 | ★ ボトルネック |
| `lua-imgui` | なし | コマンドバッファ方式に移行済み |
| `video-decoder` | 直接 | テクスチャ出力 |
| `virtual-camera` | なし | POSIX shm 経由 |

## 解決策

### 案1: upstream 対応待ち (推奨)

[dear-imgui-rs](https://github.com/Latias94/dear-imgui-rs) が wgpu 29 対応版をリリースするのを待つ。

- **メリット**: メンテナンスコストゼロ
- **デメリット**: 時期不明

### 案2: dear-imgui-wgpu フォーク

`dear-imgui-wgpu` をフォークし、wgpu 29 API に自前で対応する。

- **メリット**: 即座に wgpu 29 に移行可能
- **デメリット**: フォークのメンテナンスコスト、upstream とのマージ作業
- **作業量**: API 差分は限定的（上記テーブルの6箇所程度）

### 案3: imgui-renderer を自前レンダラーに置換

`dear-imgui-wgpu` を使わず、ImGui の DrawData を直接 wgpu で描画する自前レンダラーを実装する。

- **メリット**: wgpu バージョンに依存しなくなる
- **デメリット**: 実装コスト大（シェーダー、頂点バッファ、テクスチャアトラス管理）
- **参考**: `lua-imgui` の旧 `renderer.rs` (411行) に簡易実装が存在

## 推奨アクション

1. **短期**: wgpu 28 のまま維持（現状で問題なく動作）
2. **中期**: dear-imgui-rs の GitHub リリースを監視し、wgpu 29 対応版が出たら即座にアップグレード
3. **長期（必要時）**: 案2（フォーク）で対応。作業量は1日以内と推定

## 関連リンク

- [dear-imgui-rs リポジトリ](https://github.com/Latias94/dear-imgui-rs)
- [wgpu 29.0 Changelog](https://github.com/gfx-rs/wgpu/releases)
