# デスクトップマスコット機能 — 設計書

## 1. 概要

VRM モデルをデスクトップ上にオーバーレイ表示する「デスクトップマスコット」機能。
ウィンドウ背景を透過し、タイトルバーを非表示にして、モデルがデスクトップ上に直接いるように見せる。

## 2. 必要な機能

| 機能 | 説明 |
|------|------|
| **ウィンドウ透過** | 背景が透明、VRM モデルだけが見える |
| **タイトルバー非表示** | `decorations(false)` でフレームなし |
| **最前面表示** | `WindowLevel::AlwaysOnTop` で常に最前面 |
| **ドラッグ移動** | ウィンドウの任意の場所をクリック＆ドラッグで移動 |
| **クリックスルー** | モデル以外の透明部分はクリックが背面ウィンドウに通過 (将来) |
| **リサイズ** | マウスホイールでウィンドウサイズ変更 |
| **モード切替** | 通常ウィンドウ ↔ マスコットモードをキーで切替 |

## 3. プラットフォーム別対応状況

### winit 0.30.9 サポート

| 機能 | Windows | macOS | Linux (X11) | Linux (Wayland) |
|------|---------|-------|-------------|-----------------|
| `with_transparent(true)` | ✅ | ✅ | ✅ | ✅ |
| `with_decorations(false)` | ✅ | ✅ | ✅ | ✅ |
| `WindowLevel::AlwaysOnTop` | ✅ | ✅ | ✅ | ❌ ヒントのみ |
| ドラッグ移動 (drag_window) | ✅ | ✅ | ✅ | ✅ |

### wgpu 24.0 透過レンダリング

| 項目 | 現状 | 必要な変更 |
|------|------|-----------|
| Surface format | `Bgra8UnormSrgb` (alpha あり) | `alpha_mode: CompositeAlphaMode::PreMultiplied` に変更 |
| Clear color | `a: 1.0` (不透明) | `a: 0.0` (透明) に変更 |
| Blend state | `ALPHA_BLENDING` | マスコットモード時は `PREMULTIPLIED_ALPHA_BLENDING` |

### プラットフォーム固有の注意点

**macOS:**
- `NSWindow.backgroundColor = NSColor.clearColor` が必要 (winit が `with_transparent(true)` で自動設定)
- `NSWindow.isOpaque = false` が必要 (同上)
- Metal surface の `alpha_mode` を `PostMultiplied` or `PreMultiplied` にする必要あり

**Windows:**
- `WS_EX_LAYERED` + `SetLayeredWindowAttributes` は DWM 合成が有効なら不要
- winit `with_transparent(true)` で DWM が透過を処理
- D3D12 surface の `alpha_mode` を `PreMultiplied` にする

**Linux (X11):**
- コンポジタ (picom, compton, mutter 等) が必要
- ARGB ビジュアルの選択は winit が処理
- Vulkan surface の `alpha_mode` を `PreMultiplied` にする

**Linux (Wayland):**
- `AlwaysOnTop` が非対応 (layer-shell プロトコルが必要だが winit 未対応)
- 透過自体は動作

## 4. アーキテクチャ

```
┌─────────────────────────────────────────────────┐
│  MascotMode (crates/app/src/mascot.rs)          │
│                                                  │
│  pub struct MascotState {                        │
│      enabled: bool,         // マスコットモード  │
│      dragging: bool,        // ドラッグ中        │
│      drag_origin: PhysicalPosition<f64>,         │
│      original_size: LogicalSize<u32>,            │
│  }                                               │
│                                                  │
│  impl MascotState {                              │
│      fn enter(window) → 透過+装飾なし+最前面     │
│      fn leave(window) → 通常ウィンドウに復帰     │
│      fn handle_mouse_down(window, position)      │
│      fn handle_mouse_move(window, position)      │
│      fn handle_mouse_up()                        │
│  }                                               │
└──────────────┬──────────────────────────────────┘
               │
    ┌──────────┼──────────────┐
    ▼          ▼              ▼
┌────────┐ ┌─────────┐ ┌──────────────┐
│app.rs  │ │scene.rs │ │context.rs    │
│KeyM    │ │clear_a=0│ │alpha_mode    │
│toggle  │ │bg_video │ │transparent   │
│drag    │ │=None    │ │reconfigure   │
└────────┘ └─────────┘ └──────────────┘
```

## 5. 変更対象ファイル

| ファイル | 変更内容 |
|----------|----------|
| `crates/app/src/mascot.rs` | **新規**: MascotState struct + enter/leave/drag ロジック |
| `crates/app/src/app.rs` | KeyM でマスコットモード切替、マウスイベントでドラッグ |
| `crates/app/src/state.rs` | `mascot: MascotState` フィールド追加 |
| `crates/app/src/init.rs` | MascotState 初期化 |
| `crates/renderer/src/context.rs` | `set_transparent(bool)` — alpha_mode 切替 + surface 再設定 |
| `crates/renderer/src/scene.rs` | clear_color.a を 0.0 に切替可能にする |
| `crates/app/src/user_prefs.rs` | `mascot_mode: bool` 永続化 |

## 6. 実装フロー

### マスコットモード ON (`KeyM` 押下):

```
1. window.set_decorations(false)          — タイトルバー非表示
2. window.set_window_level(AlwaysOnTop)   — 最前面
3. render_ctx.set_transparent(true)       — surface alpha_mode 変更
4. scene.set_clear_color(a: 0.0)          — 背景透過
5. scene.remove_background_video()        — 動画背景無効化
6. scene.bg_image = None                  — 静止画背景無効化
7. window.set_inner_size(512, 512)        — マスコットサイズに縮小
```

### マスコットモード OFF (`KeyM` 再押下):

```
1. window.set_decorations(true)           — タイトルバー復帰
2. window.set_window_level(Normal)        — 通常レベル
3. render_ctx.set_transparent(false)      — surface 通常モード
4. scene.set_clear_color(a: 1.0)          — 背景不透明に戻す
5. 背景設定を復元                          — image_path から再設定
6. window.set_inner_size(1280, 720)       — 通常サイズに復帰
```

### ドラッグ移動:

```
MouseDown (Left) in mascot mode:
  → dragging = true
  → drag_origin = cursor_position

MouseMove while dragging:
  → window.set_outer_position(
      window_pos + (cursor_position - drag_origin)
    )

MouseUp:
  → dragging = false
```

## 7. RenderContext の透過対応

```rust
// context.rs に追加
impl RenderContext {
    /// Switch the surface between opaque and transparent modes.
    pub fn set_transparent(&mut self, transparent: bool) {
        self.config.alpha_mode = if transparent {
            // PreMultiplied: alpha=0 の部分がデスクトップが透けて見える
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            wgpu::CompositeAlphaMode::Opaque
        };
        self.surface.configure(&self.device, &self.config);
    }
}
```

**注意**: `CompositeAlphaMode::PreMultiplied` が全プラットフォームでサポートされるとは限らない。
`get_default_config()` が返す capabilities を確認して、サポートされるモードを選択する必要がある。

```rust
// より安全な実装
pub fn set_transparent(&mut self, transparent: bool) {
    if transparent {
        // 利用可能な alpha_mode から透過対応のものを選択
        let caps = self.surface.get_capabilities(&self.adapter);
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else {
            log::warn!("No transparent alpha mode available, falling back to Opaque");
            wgpu::CompositeAlphaMode::Opaque
        };
        self.config.alpha_mode = alpha_mode;
    } else {
        self.config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
    }
    self.surface.configure(&self.device, &self.config);
}
```

## 8. シェーダの注意点

マスコットモード (PreMultiplied alpha) では、フラグメントシェーダの出力を **プリマルチプライド** にする必要がある:

```
// 通常 (Opaque): output.rgb = color.rgb, output.a = 1.0
// PreMultiplied: output.rgb = color.rgb * color.a, output.a = color.a
```

MToon シェーダが既に `ALPHA_BLENDING` を使っているので、出力がプリマルチプライドかどうか確認が必要。
wgpu の `PREMULTIPLIED_ALPHA_BLENDING` に切り替えるか、最終パスで変換する。

**最も簡単なアプローチ**: シェーダ出力はそのまま (non-premultiplied)、`CompositeAlphaMode::PostMultiplied` を使う。
PostMultiplied は「フラグメントの alpha をそのまま使ってコンポジットする」モードで、
現在のシェーダ出力と互換性がある。

## 9. 将来拡張

| 機能 | 説明 | 複雑度 |
|------|------|--------|
| **クリックスルー** | 透明部分のクリックイベントを背面ウィンドウに透過。OS 固有 API が必要 (Windows: `WS_EX_TRANSPARENT`, macOS: `NSWindow.ignoresMouseEvents`) | 高 |
| **システムトレイ** | タスクバー/メニューバーから操作 | 中 |
| **物理演算** | マスコットが画面端に反応、揺れる | 中 |
| **複数モニター** | マスコットを別モニターに移動 | 低 |
| **右クリックメニュー** | コンテキストメニューで設定変更 | 中 |
