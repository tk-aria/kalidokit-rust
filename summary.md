# 作業サマリー

## Step 1.1: ワークスペース再構築 (2026/03/09)

### 実行内容

1. **ルート Cargo.toml 更新**
   - 5クレートワークスペース構成: `app`, `renderer`, `vrm`, `solver`, `tracker`
   - Bevy依存 (`bevy`, `bevy_vrm`) を削除
   - wgpu/winit/gltf/bytemuck/serde/serde_json/thiserror/pollster/env_logger/log を追加

2. **crates/renderer 新規作成**
   - `crates/renderer/Cargo.toml` 作成 (wgpu, winit, glam, bytemuck, image, anyhow, log)
   - `crates/renderer/src/lib.rs` 作成 (空)

3. **crates/vrm 新規作成**
   - `crates/vrm/Cargo.toml` 作成 (gltf, glam, serde, serde_json, anyhow, thiserror, log)
   - `crates/vrm/src/lib.rs` 作成 (空)

4. **crates/app/Cargo.toml 書き換え**
   - Bevy依存削除
   - renderer/vrm/solver/tracker クレート依存 + winit/nokhwa/image/pollster/env_logger/log/anyhow

5. **solver/tracker Cargo.toml に thiserror 追加**

6. **既存 Bevy コード削除**
   - `crates/app/src/components/`, `crates/app/src/plugins/`, `crates/app/src/systems/` を削除
   - `crates/app/src/main.rs` をプレースホルダーに置き換え

7. **tracker クレートの API修正** (ライブラリバージョン変更対応)
   - `ort::Session::builder()?.with_model_from_file()` → `.commit_from_file()` (ort 2.0 API)
   - `image::imageops::FilterType::Bilinear` → `::Triangle` (image 0.25 API)

### ビルド環境構築
- C コンパイラ未インストール問題: conda で `gcc_linux-64`, `binutils_linux-64`, `openssl`, `pkg-config`, `libclang`, `kernel-headers_linux-64`, `nasm` をインストール
- `.cargo-env.sh` ラッパースクリプト作成 (PATH/LIBRARY_PATH/LIBCLANG_PATH/OPENSSL_DIR 設定)

### 実行コマンド

```bash
# ディレクトリ作成
mkdir -p crates/renderer/src crates/vrm/src

# 旧コード削除
rm -rf crates/app/src/components crates/app/src/plugins crates/app/src/systems

# ビルド環境構築 (conda経由)
curl -sSL "https://github.com/conda-forge/miniforge/releases/latest/download/Miniforge3-Linux-x86_64.sh" -o /tmp/miniforge.sh
bash /tmp/miniforge.sh -b -p /tmp/conda
conda install -y gcc_linux-64 binutils_linux-64 openssl pkg-config libclang kernel-headers_linux-64 nasm

# コンパイル確認
./.cargo-env.sh cargo check  # → Finished dev profile
```

### 結果
- `cargo check` 全5クレートで成功
