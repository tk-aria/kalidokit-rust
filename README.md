# KalidoKit Rust

Webカメラからの映像をリアルタイムに解析し、顔・体・手のモーションキャプチャを行い、VRM形式の3Dアバターに反映するデスクトップアプリケーション。

[kalidokit-testbed](https://github.com/tk-aria/kalidokit-testbed) の `vrm/index.html` をベースに、Rustで同等の機能を実現する。

## Architecture

```
Camera (nokhwa)
  → ML Inference (ort / ONNX Runtime)
    → Face 468pts / Pose 33pts / Hand 21pts×2
      → Solver (KalidoKit algorithm port)
        → Bone Rotations + BlendShapes
          → VRM Avatar (wgpu direct rendering)
```

## Tech Stack

| Component | JS (Original) | Rust (This Project) |
|-----------|--------------|---------------------|
| 3D Engine | Three.js | wgpu (Vulkan/Metal/DX12/WebGPU) |
| VRM Loader | @pixiv/three-vrm | vrm crate (custom gltf parser) |
| Shader | THREE.MeshToonMaterial | MToon WGSL shader |
| Motion Capture | MediaPipe Holistic | ort + ONNX models |
| Rig Solver | KalidoKit | solver crate (port) |
| Camera | Camera Utils | nokhwa |
| Math | THREE.Quaternion | glam (Quat/Vec3/Mat4) |
| Physics | - | SpringBone (Verlet integration) |

## Project Structure

```
kalidokit-rust/
├── Cargo.toml                 # Workspace root
├── assets/
│   ├── models/                # VRM avatar + ONNX models
│   └── shaders/               # WGSL shaders (skinning, mtoon)
├── crates/
│   ├── app/                   # Application entry point (bin)
│   ├── renderer/              # wgpu rendering engine (lib)
│   ├── vrm/                   # VRM loader & bone/blendshape (lib)
│   ├── solver/                # Rig solver - face/pose/hand (lib)
│   └── tracker/               # ML tracking - ONNX inference (lib)
└── .github/workflows/         # CI/CD & Release
```

## Install

### ワンライナー (Linux / macOS)

```bash
# 最新バージョンをインストール
curl -sSLf https://raw.githubusercontent.com/tk-aria/kalidokit-rust/main/scripts/setup.sh | sh -s install

# バージョン指定
curl -sSLf https://raw.githubusercontent.com/tk-aria/kalidokit-rust/main/scripts/setup.sh | KALIDOKIT_VERSION=v0.1.0 sh -s install

# カスタムインストール先
curl -sSLf https://raw.githubusercontent.com/tk-aria/kalidokit-rust/main/scripts/setup.sh | KALIDOKIT_INSTALL_PATH=~/.local/bin sh -s install

# アンインストール
curl -sSLf https://raw.githubusercontent.com/tk-aria/kalidokit-rust/main/scripts/setup.sh | sh -s uninstall
```

> **注意**: デフォルトのインストール先は `/usr/local/bin` です。権限がない場合は `sudo` を使うか `KALIDOKIT_INSTALL_PATH` を指定してください。

### モデルファイルのダウンロード

実行にはMLモデル（ONNX）とVRMアバターが必要です。以下のコマンドで自動ダウンロードできます:

```bash
# プロジェクトルートで実行（assets/models/ にダウンロード）
sh scripts/setup.sh download-models

# カスタムパスを指定
KALIDOKIT_MODELS_PATH=./my-models sh scripts/setup.sh download-models
```

| ファイル | 説明 | サイズ |
|---------|------|--------|
| `face_landmark.onnx` | 顔ランドマーク検出 (468点) | ~2.4 MB |
| `pose_landmark.onnx` | 体ポーズ推定 (33点) | ~5.3 MB |
| `hand_landmark.onnx` | 手ランドマーク検出 (21点×2) | ~4.0 MB |
| `default_avatar.vrm` | デフォルトVRMアバター | ~7.6 MB |

---

### 手動ダウンロード

各プラットフォーム向けのビルド済みバイナリは [GitHub Releases](https://github.com/tk-aria/kalidokit-rust/releases) からダウンロードできます。

### Linux (x86_64)

```bash
# 最新リリースをダウンロード
curl -LO https://github.com/tk-aria/kalidokit-rust/releases/latest/download/kalidokit-rust-<VERSION>-x86_64-unknown-linux-gnu.tar.gz

# 展開
tar xzf kalidokit-rust-<VERSION>-x86_64-unknown-linux-gnu.tar.gz

# 実行
cd kalidokit-rust-<VERSION>-x86_64-unknown-linux-gnu
./kalidokit-rust
```

> **必要なシステムライブラリ**: `libvulkan1`, `libx11-6`, `libxkbcommon0`, `libwayland-client0`
>
> ```bash
> # Ubuntu/Debian
> sudo apt-get install -y libvulkan1 libx11-6 libxkbcommon0 libwayland-client0
> ```

### macOS (Apple Silicon / Intel)

```bash
# Apple Silicon (M1/M2/M3/M4) & Intel Mac (Rosetta 2 経由で動作)
curl -LO https://github.com/tk-aria/kalidokit-rust/releases/latest/download/kalidokit-rust-<VERSION>-aarch64-apple-darwin.tar.gz
tar xzf kalidokit-rust-<VERSION>-aarch64-apple-darwin.tar.gz

# 実行
cd kalidokit-rust-<VERSION>-aarch64-apple-darwin
./kalidokit-rust
```

> **Intel Mac**: aarch64 バイナリは Rosetta 2 経由で動作します。
>
> **注意**: 初回実行時に「開発元を検証できない」と表示された場合:
> ```bash
> xattr -cr kalidokit-rust
> ```

### Windows (x86_64)

1. [Releases ページ](https://github.com/tk-aria/kalidokit-rust/releases) から `kalidokit-rust-<VERSION>-x86_64-pc-windows-msvc.zip` をダウンロード
2. ZIP を展開
3. `kalidokit-rust.exe` をダブルクリックで実行

> **必要**: 最新の [Vulkan Runtime](https://vulkan.lunarg.com/sdk/home) がインストールされていること (多くの環境ではGPUドライバに含まれています)

## Build from Source

```bash
# Clone
git clone https://github.com/tk-aria/kalidokit-rust.git
cd kalidokit-rust

# Build
cargo build --release

# Run
cargo run --release
```

### 必要なシステム依存

- **Linux**: `cmake`, `pkg-config`, `libx11-dev`, `libxkbcommon-dev`, `libwayland-dev`
- **macOS**: Xcode Command Line Tools
- **Windows**: Visual Studio Build Tools (MSVC)

## Release (メンテナー向け)

タグをプッシュすると GitHub Actions が自動で全プラットフォームのバイナリをビルドし、GitHub Release を作成します。

```bash
git tag v0.1.0
git push origin v0.1.0
```

## License

MIT
