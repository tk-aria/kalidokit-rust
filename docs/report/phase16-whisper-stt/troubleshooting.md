# Phase 16 Troubleshooting

## エラー: libclang architecture mismatch (x86_64 vs arm64)

### エラー内容
```
Unable to find libclang: "the `libclang` shared library at
/usr/local/Cellar/llvm/19.1.7_1/lib/libclang.dylib could not be opened:
(mach-o file, but is an incompatible architecture (have 'x86_64', need 'arm64'))"
```

### 原因
Homebrew (x86_64) の llvm がインストールされており、bindgen がそれを発見するが
arm64 バイナリとして実行しているため ABI 不一致。

### 解決
Xcode Command Line Tools の arm64 libclang を使うよう環境変数を設定:
```bash
LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib \
CC=/usr/bin/clang CXX=/usr/bin/clang++ \
cargo check -p speech-capture --features stt
```
