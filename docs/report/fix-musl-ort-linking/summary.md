# Fix: Linux musl ORT 静的ライブラリリンクエラー

## 問題
```
error: could not find native static library `onnx`, perhaps an -L flag is missing?
error: could not compile `ort-sys` (lib) due to 1 previous error
```

## 根本原因
`ort-sys` 2.0.0-rc.12 の build.rs は4つのディレクトリ構造パターンを順に試す。

ORT ビルド後の実際のディレクトリ構造:
```
/tmp/ort-build/
  Release/
    libonnxruntime_common.a  (ここ)
    _deps/
      onnx-build/
        libonnx.a            (ここ)
```

`ORT_LIB_PATH=/tmp/ort-build` + `ORT_LIB_PROFILE=Release` の場合:
- Config 1 が最初にマッチ: `lib_dir = base/Release` (OK)
- しかし `external_lib_dir = base/_deps` → `/tmp/ort-build/_deps` を探す (存在しない)
- `libonnx.a` は `/tmp/ort-build/Release/_deps/onnx-build/` にある

## 修正
`ORT_LIB_PATH` を直接 `Release` ディレクトリに向け、`ORT_LIB_PROFILE` を削除:

```yaml
# Before:
ORT_LIB_PATH: /tmp/ort-build
ORT_LIB_PROFILE: Release

# After:
ORT_LIB_PATH: /tmp/ort-build/Release
```

これにより profile が空になり、Config 1 が `lib_dir = base`, `external_lib_dir = base/_deps` と解釈。
`/tmp/ort-build/Release/_deps/onnx-build/libonnx.a` が正しく発見される。

## 実行コマンド
```bash
# release.yml を修正
git add . && git commit && git push origin main
# タグをリトリガー
git tag -d v0.2.0 && git push origin :refs/tags/v0.2.0
git tag v0.2.0 && git push origin v0.2.0
```
