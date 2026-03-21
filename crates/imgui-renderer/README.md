# imgui-renderer

Dear ImGui integration for wgpu applications, built on top of [imgui-rs](https://github.com/imgui-rs/imgui-rs).

`ImGuiRenderer` wraps `imgui`, `imgui-wgpu`, and `imgui-winit-support` into a single type with a three-method API: **handle_event**, **frame**, and **render**.

## Features

- **Docking** — ImGui docking support enabled by default (`ConfigFlags::DOCKING_ENABLE`)
- **Overlay mode** — Renders with `LoadOp::Load` so ImGui draws on top of existing content
- **HiDPI support** — Automatic font scaling based on the window's scale factor
- **Minimal API** — Three core methods to integrate into any wgpu + winit application
- **Full access** — Escape hatches to the underlying `imgui::Context` and `imgui_wgpu::Renderer`
- **Custom render pass** — `render_into_pass()` for embedding ImGui into your own render pass

## Requirements

- **Rust** 1.70+
- **C++ compiler** — Required to build [cimgui](https://github.com/cimgui/cimgui) (the C bindings for Dear ImGui that imgui-rs depends on)
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
  - Linux: `build-essential` or equivalent
  - Windows: MSVC (Visual Studio Build Tools)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
imgui-renderer = { path = "crates/imgui-renderer" }
```

Key dependencies (managed by the crate):

| Crate | Version |
|---|---|
| `imgui` | 0.12 (with `docking` feature) |
| `imgui-wgpu` | 0.28 |
| `imgui-winit-support` | 0.13 |

## Quick Start

```rust
use imgui_renderer::ImGuiRenderer;

// 1. Create the renderer (after wgpu device/queue and winit window are ready)
let mut imgui = ImGuiRenderer::new(
    &device,
    &queue,
    wgpu::TextureFormat::Bgra8UnormSrgb,
    &window,
).unwrap();

// 2. In your event handler — forward events to ImGui
imgui.handle_event(&window, window_id, &event);

// 3. Each frame — build UI and render
imgui.frame(&window, |ui| {
    ui.window("Hello").build(|| {
        ui.text("Hello from ImGui!");
    });
});
imgui.render(&device, &queue, &texture_view);
```

## API Reference

### `ImGuiRenderer::new(device, queue, format, window) -> Result<Self>`

Creates a new renderer. Initializes the ImGui context with docking enabled, configures HiDPI font scaling, and sets up the wgpu renderer.

### `handle_event(&mut self, window, window_id, event)`

Forwards a `WindowEvent` to ImGui for input handling. Call this for every event in your `ApplicationHandler::window_event`.

### `handle_non_window_event(&mut self, window, event)`

Forwards non-window events (e.g. `Event::AboutToWait`) to ImGui. Typically called from `about_to_wait`.

### `frame(&mut self, window, f: FnOnce(&imgui::Ui))`

Builds the ImGui frame. The closure receives a `&imgui::Ui` to define your UI. Automatically tracks delta time between frames.

### `frame_with_dt(&mut self, window, dt: Duration, f: FnOnce(&imgui::Ui))`

Same as `frame` but uses an explicit delta time instead of measuring it automatically.

### `render(&mut self, device, queue, view)`

Renders ImGui draw data onto the given texture view. Creates its own render pass with `LoadOp::Load` (overlay mode).

### `render_into_pass(&mut self, queue, device, rpass)`

Renders ImGui draw data into an existing render pass. Use this when you need full control over the render pass configuration.

### Accessors

| Method | Description |
|---|---|
| `context() -> &Context` | Read-only access to the imgui `Context` |
| `context_mut() -> &mut Context` | Mutable access to the imgui `Context` |
| `renderer() -> &Renderer` | Read-only access to the imgui-wgpu `Renderer` |
| `renderer_mut() -> &mut Renderer` | Mutable access to the imgui-wgpu `Renderer` |
| `reload_font_texture(device, queue)` | Rebuild the font atlas after adding/removing fonts |
| `want_capture_mouse() -> bool` | Whether ImGui wants mouse input (skip your own mouse handling) |
| `want_capture_keyboard() -> bool` | Whether ImGui wants keyboard input (skip your own key handling) |

### Re-exports

The crate re-exports its core dependencies for convenience:

```rust
use imgui_renderer::imgui;              // imgui-rs
use imgui_renderer::imgui_wgpu;         // imgui-wgpu renderer
use imgui_renderer::imgui_winit_support; // winit platform backend
```

## Examples

### Standalone Demo Window

Opens a window with the ImGui demo window and an FPS counter.

```bash
cargo run -p imgui-renderer --example standalone
```

### Overlay

Demonstrates ImGui rendering on top of a colored background, with controls for background color and panel transparency.

```bash
cargo run -p imgui-renderer --example overlay
```

## Integration with Existing wgpu Apps

To add ImGui to an existing wgpu + winit application:

1. **Create** the `ImGuiRenderer` after your wgpu device and window are initialized:

```rust
let mut imgui = ImGuiRenderer::new(&device, &queue, surface_format, &window)?;
```

2. **Forward events** in your `ApplicationHandler`:

```rust
fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
    self.imgui.handle_event(&self.window, id, &event);

    // Check if ImGui wants input before processing your own
    if !self.imgui.want_capture_mouse() {
        // Handle your own mouse input
    }
}

fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
    self.window.request_redraw();
    self.imgui.handle_non_window_event(&self.window, &Event::<()>::AboutToWait);
}
```

3. **Render** after your main scene:

```rust
// Your scene rendering (clear, draw meshes, etc.)
// ...

// ImGui on top
imgui.frame(&window, |ui| {
    // Your UI here
});
imgui.render(&device, &queue, &view);

frame.present();
```

## License

Same license as the parent kalidokit-rust project.
