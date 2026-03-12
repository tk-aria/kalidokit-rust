# Step 10.5: Extension ビルド & 署名設定 — 作業報告

**日時**: 2026-03-12 15:50 JST
**ステータス**: 完了

## 実行した操作

### 1. build.rs 作成
- ファイル: `crates/virtual-camera/build.rs`
- `cc` crate で ObjC ソース4ファイルをコンパイル (-fobjc-arc -fmodules)
- CoreMediaIO, CoreMedia, CoreVideo, Foundation フレームワークをリンク
- `#[cfg(target_os = "macos")]` で条件コンパイル
- `cc = "1"` を build-dependencies に追加

### 2. build-camera-extension.sh 作成
- ファイル: `scripts/build-camera-extension.sh`
- clang で ObjC ソースをコンパイル → .appex バンドル構造作成
- `--sign IDENTITY` オプションで codesign + timestamp 対応
- 出力: `target/camera-extension/KalidoKitCamera.appex`

### 3. 開発用ドキュメント作成
- ファイル: `docs/camera-extension-dev-setup.md`
- SIP 無効化手順 (Recovery Mode)
- Extension のビルド・インストール手順
- トラブルシューティング

### 4. 配布用ドキュメント作成
- ファイル: `docs/camera-extension-distribution.md`
- Developer ID 証明書・Provisioning Profile 取得手順
- 署名・公証 (notarytool) 手順
- .app バンドルへの埋め込み構造

### 5. 検証
```bash
# build.rs コンパイル
cargo check -p virtual-camera  # 成功 (ObjC 警告 3件のみ)

# .appex バンドル生成
./scripts/build-camera-extension.sh  # 成功
ls target/camera-extension/KalidoKitCamera.appex/Contents/MacOS/
# kalidokit-camera-extension (実行可能)
```
