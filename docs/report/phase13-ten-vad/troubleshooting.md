# Phase 13 Troubleshooting

## エラー: dyld Library not loaded @rpath/ten_vad.framework

### エラー内容
```
dyld[61232]: Library not loaded: @rpath/ten_vad.framework/Versions/A/ten_vad
  Referenced from: target/debug/deps/ten_vad-da0a6881b5063ca7
  Reason: tried: '/Library/Frameworks/ten_vad.framework/Versions/A/ten_vad' (no such file)
```

### 原因
macOS の .framework はリンク時に `@rpath` ベースのインストール名を持つ。
実行時に dyld が `@rpath` を解決できないと、フレームワークが見つからない。

build.rs で `cargo:rustc-link-search=framework=` を設定してもリンク時のみ有効で、
実行時の `@rpath` にはそのパスが含まれない。

### 解決
build.rs に以下を追加:
```rust
println!("cargo:rustc-link-arg=-Wl,-rpath,{}", slice.display());
```

これにより実行バイナリの LC_RPATH に xcframework slice のパスが埋め込まれ、
dyld が実行時にフレームワークを発見できるようになる。
