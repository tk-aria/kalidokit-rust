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
          → VRM Avatar (Bevy + bevy_vrm)
```

## Tech Stack

| Component | JS (Original) | Rust (This Project) |
|-----------|--------------|---------------------|
| 3D Engine | Three.js | Bevy Engine (wgpu) |
| VRM Loader | @pixiv/three-vrm | bevy_vrm |
| Motion Capture | MediaPipe Holistic | ort + ONNX models |
| Rig Solver | KalidoKit | solver crate (port) |
| Camera | Camera Utils | nokhwa |
| Math | THREE.Quaternion | glam (Quat/Vec) |

## Project Structure

```
kalidokit-rust/
├── Cargo.toml                 # Workspace root
├── assets/
│   ├── models/                # VRM avatar files
│   └── ml/                    # ONNX inference models
├── crates/
│   ├── app/                   # Bevy application (bin)
│   ├── solver/                # Rig solver library (lib)
│   └── tracker/               # ML tracking library (lib)
├── docs/
│   └── design.md              # Full design document
└── examples/
    └── simple_tracking.rs
```

## Documentation

- [Design Document](docs/design.md) - E-R diagram, sequence diagram, directory layout, module I/O, sample code

## License

MIT
