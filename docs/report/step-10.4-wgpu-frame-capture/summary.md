# Step 10.4: wgpu フレームキャプチャ統合 — 作業報告

**日時**: 2026-03-12 15:46 JST
**ステータス**: 完了

## 実行した操作

### 1. app crate に virtual-camera 依存追加
- ファイル: `crates/app/Cargo.toml`
- `virtual-camera = { path = "../virtual-camera" }` 追加

### 2. Scene にフレームキャプチャ機能追加
- ファイル: `crates/renderer/src/scene.rs`
- 4フィールド追加: `frame_capture_buffer`, `frame_capture_texture`, `frame_capture_width`, `frame_capture_height`
- 4メソッド追加:
  - `ensure_frame_capture()`: テクスチャ+ステージングバッファ作成 (256バイトアライメント)
  - `frame_capture_view()`: キャプチャテクスチャのビュー取得
  - `copy_frame_to_buffer()`: テクスチャ→バッファコピー
  - `read_frame_capture()`: `buffer.map_async()` + `device.poll(Wait)` でCPU読み出し

### 3. AppState に仮想カメラ状態追加
- ファイル: `crates/app/src/state.rs`
- `vcam: Option<MacOsVirtualCamera>` (#[cfg(target_os = "macos")])
- `vcam_enabled: bool`

### 4. キーバインド追加
- ファイル: `crates/app/src/app.rs`
- `KeyCode::KeyC` → `vcam_enabled` トグル

### 5. update.rs にフレーム送信統合
- ファイル: `crates/app/src/update.rs`
- `vcam_send_frame()` 関数: render→copy→map→send パイプライン
- BGRA→RGBA 変換 (wgpu出力がBGRA、VirtualCamera trait がRGBA)
- 初回フレームで MacOsVirtualCamera::start() を遅延初期化
- HUD に "C: VCam (ON/OFF)" 行追加

### 6. init.rs で初期化
- ファイル: `crates/app/src/init.rs`
- `vcam: None`, `vcam_enabled: false`

### 7. コンパイル検証
```bash
cargo check --workspace
```
結果: 成功 (dead_code 警告 1 件のみ)
