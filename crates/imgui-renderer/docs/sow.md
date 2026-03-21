# imgui-renderer — Statement of Work (SoW)

> **プロジェクト名**: imgui-renderer
> **バージョン**: 0.1.0
> **作成日**: 2026-03-20
> **前提**: wgpu 24 → 29 アップグレード (WP-A) 完了後に WP-B 以降を実施

---

## 1. スコープ

### In Scope

| カテゴリ | 内容 |
|----------|------|
| wgpu アップグレード | workspace 全体を wgpu 24 → 29 に移行 |
| ImGui 統合 | dear-imgui-rs + dear-imgui-wgpu + dear-imgui-winit |
| ラッパー API | `ImGuiRenderer` (new, handle_event, frame, render) |
| アプリ統合 | kalidokit-rust にデバッグ/設定 UI を追加 |
| 拡張 | ImPlot, ImGuizmo, ImNodes の統合基盤 |

### Out of Scope

| カテゴリ | 理由 |
|----------|------|
| egui 対応 | C++ 拡張エコシステムが使えない |
| カスタムウィジェット開発 | 標準 + 拡張で十分 |
| Web (WASM) 対応 | マルチビューポートが WASM 未対応 |

---

## 2. 成果物

| # | 成果物 | 形式 |
|---|--------|------|
| D1 | wgpu 29 アップグレード済みワークスペース | Cargo.toml + ソース修正 |
| D2 | `crates/imgui-renderer/` | Rust crate |
| D3 | ImGui 単体デモ example | `examples/standalone.rs` |
| D4 | 3D オーバーレイ example | `examples/overlay.rs` |
| D5 | kalidokit-rust アプリ統合 | デバッグ UI + 設定パネル |
| D6 | RFC + SoW ドキュメント | 本ドキュメント |

---

## 3. ワークパッケージ

### WP-A: wgpu 24 → 29 アップグレード

**目的**: ワークスペース全体の wgpu 依存を 29.0 に更新し、全クレートのビルドとテストを通す

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| A.1 | workspace wgpu バージョン更新 | `Cargo.toml` の `wgpu = "29.0"` に変更 | `Cargo.toml` | `cargo check` が依存解決でエラーにならない |
| A.2 | renderer クレート修正 | `Instance::new()`, `Surface::configure()`, `RenderPassDescriptor`, `BufferAddress` 等の API 差分修正 | `crates/renderer/src/*.rs` | `cargo check -p renderer` pass |
| A.3 | video-decoder クレート修正 | `TextureDescriptor`, `write_texture`, `ComputePassDescriptor` 等の差分修正 | `crates/video-decoder/src/*.rs` | `cargo check -p video-decoder` pass |
| A.4 | app クレート修正 | Surface acquire/present の API 差分修正 | `crates/app/src/*.rs` | `cargo check -p kalidokit-rust` pass |
| A.5 | 全ワークスペースビルド確認 | `cargo check --workspace` | — | 全クレートで pass |
| A.6 | テスト pass 確認 | `cargo test -p renderer -p vrm -p solver -p video-decoder` | — | 全テスト pass |
| A.7 | 動作確認 | VRM + 動画背景が release ビルドで動作 | — | 既存機能のリグレッションなし |

**WP-A 完了基準:**
- `cargo check --workspace` pass
- 既存テスト全 pass
- アプリが release ビルドで既存通り動作

---

### WP-B: imgui-renderer クレート作成

**目的**: dear-imgui-rs + dear-imgui-wgpu をラップし、3 メソッドで ImGui が使えるようにする

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| B.1 | クレート scaffold | Cargo.toml, lib.rs 骨格、ワークスペースに追加 | `crates/imgui-renderer/Cargo.toml` | `cargo check -p imgui-renderer` pass |
| B.2 | ImGuiRenderer::new() | Context 作成, Docking 有効化, WinitPlatform 初期化, wgpu Renderer 初期化 | `src/lib.rs` | フォントアトラステクスチャが生成される |
| B.3 | ImGuiRenderer::handle_event() | winit WindowEvent → ImGui IO 変換 | `src/lib.rs` | マウス/キーボード入力が ImGui に反映 |
| B.4 | ImGuiRenderer::frame() | delta time 更新, prepare_frame, UI クロージャ呼び出し, prepare_render | `src/lib.rs` | DrawList が生成される |
| B.5 | ImGuiRenderer::render() | DrawData → wgpu RenderPass (LoadOp::Load で既存シーン上に重ねる) | `src/lib.rs` | ImGui UI が画面に描画される |
| B.6 | context_mut() アクセサ | 高度な設定 (フォント追加, スタイル変更) 用 | `src/lib.rs` | Context に直接アクセス可能 |

**WP-B 完了基準:**
- `cargo check -p imgui-renderer` pass
- `cargo clippy -p imgui-renderer -- -D warnings` pass
- `cargo doc -p imgui-renderer --no-deps` 警告なし

---

### WP-C: Example + 動作確認

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| C.1 | standalone example | winit ウィンドウ + wgpu 初期化 + ImGui デモウィンドウ | `examples/standalone.rs` | ImGui Demo Window が表示され操作可能 |
| C.2 | overlay example | 背景色付き 3D シーン + ImGui オーバーレイ | `examples/overlay.rs` | 3D シーンの上に ImGui が半透明で重なる |
| C.3 | Docking 確認 | ウィンドウのドッキング/タブ化 | `examples/standalone.rs` | ドッキング操作が機能する |
| C.4 | 動作確認 | macOS release ビルドで 60fps 動作 | — | FPS 低下なし |

**WP-C 完了基準:**
- 2 つの example が macOS で動作
- Docking が機能
- release ビルドで安定動作

---

### WP-D: kalidokit-rust アプリ統合

| # | タスク | 詳細 | 成果ファイル | 受入基準 |
|---|--------|------|-------------|----------|
| D.1 | 依存追加 | `crates/app/Cargo.toml` に `imgui-renderer` 追加 | `Cargo.toml` | `cargo check -p kalidokit-rust` pass |
| D.2 | イベントフック | `app.rs` の `window_event` で `imgui.handle_event()` 呼び出し | `app.rs` | ImGui がマウス/キーを受け取る |
| D.3 | フレームフック | `update.rs` で `imgui.frame()` → UI 構築 | `update.rs` | UI が毎フレーム更新される |
| D.4 | レンダーフック | `update.rs` の 3D 描画後に `imgui.render()` | `update.rs` | VRM の上に ImGui が表示される |
| D.5 | デバッグ UI | FPS, VAD 状態, トラッカー結果, 動画背景情報 | `app/src/debug_ui.rs` | リアルタイム情報表示 |
| D.6 | 設定 UI | threshold, mascot size, always_on_top, lighting | `app/src/settings_ui.rs` | スライダー/チェックボックスで設定変更可能 |
| D.7 | UI 表示トグル | `F1` キーで ImGui 表示/非表示切替 | `app.rs` | F1 で toggle |

**WP-D 完了基準:**
- アプリ内でデバッグ UI + 設定 UI が動作
- F1 で表示切替
- 設定変更がリアルタイムに反映
- FPS への影響が最小限 (< 5%)

---

### WP-E: 拡張統合 (オプション)

| # | タスク | 詳細 | 受入基準 |
|---|--------|------|----------|
| E.1 | ImPlot | VAD 確率のリアルタイム折れ線グラフ | グラフがスムーズに更新される |
| E.2 | ImGuizmo | VRM モデルの位置/回転をマウスで操作 | ギズモ表示 + ドラッグで変換 |
| E.3 | ImNodes | トラッカーパイプライン (Camera → Face → Solver) の可視化 | ノード + エッジが描画される |

---

## 4. WP 依存関係

```
WP-A (wgpu 29 アップグレード)
  ↓
WP-B (imgui-renderer クレート)
  ↓
WP-C (Example + 動作確認)
  ↓
WP-D (kalidokit-rust 統合)
  ↓
WP-E (拡張, オプション)
```

全 WP は直列依存。WP-A が最もリスクが高い。

---

## 5. 品質基準

| 項目 | 基準 |
|------|------|
| ビルド | `cargo check --workspace` pass |
| テスト | 既存テスト全 pass + imgui-renderer テスト |
| Lint | `cargo clippy --workspace -- -D warnings` pass |
| フォーマット | `cargo fmt --check` pass |
| ドキュメント | `cargo doc --workspace --no-deps` 警告なし |
| パフォーマンス | ImGui 統合後の FPS 低下 < 5% |
| リグレッション | 既存機能 (VRM, 動画背景, マスコット, VAD) に影響なし |

---

## 6. 技術的リスクと対策

| # | リスク | 影響 | 対策 |
|---|--------|------|------|
| R1 | wgpu 24→29 の API 差分が大量 | renderer 修正に時間がかかる | 公式 migration guide 参照、段階的修正 |
| R2 | dear-imgui-winit が winit 0.30 と非互換 | ビルドエラー | バージョン互換表確認、必要なら fork |
| R3 | dear-imgui-wgpu の wgpu 29 対応が不完全 | ランタイムエラー | example で動作確認、issue 報告 |
| R4 | ImGui の描画が既存 3D パイプラインと干渉 | アーティファクト | LoadOp::Load + 深度バッファなし |
| R5 | ImGui のメモリ使用量増加 | フォントアトラス + 頂点バッファ | テクスチャサイズ制限、フレーム毎バッファ再利用 |

---

## 7. ライブラリ URL

| ライブラリ | URL |
|---|---|
| Dear ImGui (C++ 本家) | https://github.com/ocornut/imgui |
| cimgui (C API ラッパー) | https://github.com/cimgui/cimgui |
| dear-imgui-rs (Rust バインディング) | https://github.com/Latias94/dear-imgui-rs |
| wgpu | https://github.com/gfx-rs/wgpu |
| winit | https://github.com/rust-windowing/winit |

---

## 8. 将来拡張 (本 SoW のスコープ外)

| 拡張 | 概要 |
|------|------|
| WASM 対応 | WebGPU バックエンドで ブラウザ上 ImGui |
| マルチビューポート | ImGui ウィンドウを OS ウィンドウとして分離 |
| カスタムテーマ | kalidokit-rust 専用のダーク/ライトテーマ |
| プロファイラ UI | Tracy 風のフレーム分析 UI |
