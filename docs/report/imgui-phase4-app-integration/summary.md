# Phase 4: kalidokit-rust アプリ統合 — 作業報告

## 実行日時
2026-03-21 11:54-11:58 JST

## 完了タスク
- app/Cargo.toml に imgui-renderer 依存追加
- AppState に imgui + show_imgui フィールド追加
- init.rs で ImGuiRenderer 初期化 (失敗時 None フォールバック)
- app.rs: イベント転送 + F1 キーで表示トグル + about_to_wait フック
- update.rs: 3D シーン + デバッグオーバーレイ後に ImGui render 挿入
- デバッグ UI: FPS, decode FPS, mascot mode, always_on_top 表示

## 検証結果
- cargo check: pass
- cargo build --release: 15.6s
- 30 fps 安定 (ImGui 統合前と同等、FPS 低下なし)
- 10 秒間クラッシュなし
