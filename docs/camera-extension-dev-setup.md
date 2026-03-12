# macOS Camera Extension — 開発用セットアップ

## 前提条件

- macOS 12.3+ (Monterey 以降)
- Xcode 14+ (Command Line Tools)
- Apple Development 証明書 (推奨) or SIP 完全無効化

## SIP 無効化 (開発用)

> **重要**: Camera Extension の開発には SIP の **完全無効化** が必要です。
> `csrutil disable --without kext` 等の部分無効化では System Extension の登録が
> Code 8 (Invalid code signature) で失敗します。

### 手順

1. Mac を再起動し、Recovery Mode に入る
   - Apple Silicon Mac: 電源ボタンを長押し → Options → Continue
   - Intel Mac: 起動時に **Command + R** を長押し
2. メニューバーから **Utilities → Terminal** を選択
3. 以下のコマンドを実行:
   ```bash
   csrutil disable
   ```
4. Mac を再起動
5. Developer Mode を有効化:
   ```bash
   systemextensionsctl developer on
   ```

### SIP の状態確認

```bash
csrutil status
# 必要: "System Integrity Protection status: disabled."
# 不十分: "Custom Configuration" (Kext Signing のみ無効等)
```

### SIP を再度有効化する (開発完了後)

```bash
# Recovery Mode で実行
csrutil enable
```

## Camera Extension のビルドとインストール

### 方法 1: .app バンドル経由 (推奨)

Extension はホストアプリの .app バンドル内に埋め込む必要があります。

```bash
# ホストアプリ + Extension を含む .app バンドルをビルド
./scripts/build-app-bundle.sh

# 生成物: target/debug/KalidoKit.app
# 構造:
#   KalidoKit.app/
#     Contents/
#       MacOS/kalidokit-rust
#       Library/SystemExtensions/
#         com.kalidokit.rust.camera-extension.systemextension/
```

### 方法 2: Extension バンドル単体ビルド

```bash
./scripts/build-camera-extension.sh
# 生成物: target/camera-extension/KalidoKitCamera.appex
```

### 署名

```bash
# Apple Development 証明書で署名する場合:
IDENTITY="Apple Development: your@email.com (XXXXXXXX)"
codesign --force --sign "$IDENTITY" \
    --entitlements crates/virtual-camera/macos-extension/Extension.entitlements \
    target/debug/KalidoKit.app/Contents/Library/SystemExtensions/*.systemextension
codesign --force --sign "$IDENTITY" target/debug/KalidoKit.app

# ad-hoc 署名の場合 (SIP 完全無効化が必須):
# build-app-bundle.sh が自動で ad-hoc 署名します
```

### 動作確認

```bash
# アプリを起動 (Extension のインストールが自動実行される)
open target/debug/KalidoKit.app

# または直接実行
target/debug/KalidoKit.app/Contents/MacOS/kalidokit-rust

# Extension が登録されたか確認
systemextensionsctl list

# カメラデバイスの確認
system_profiler SPCameraDataType
# "KalidoKit Virtual Camera" が表示されるはず
```

## トラブルシューティング

### OSSystemExtensionErrorDomain Code 8 (Invalid code signature)

最も一般的なエラー。原因と対策:

1. **SIP が部分無効化のみ**: `csrutil status` で確認。`Custom Configuration` と表示される場合は完全無効化が必要
2. **CFBundleExecutable と実際のバイナリ名が不一致**: Info.plist の `CFBundleExecutable` = `kalidokit-camera-extension` とバイナリファイル名が一致しているか確認
3. **Bundle ID 階層**: Extension ID (`com.kalidokit.rust.camera-extension`) はホスト ID (`com.kalidokit.rust`) の子である必要がある

### OSSystemExtensionErrorDomain Code 4 (Extension not found)

- Extension が `.app/Contents/Library/SystemExtensions/` に正しく配置されているか確認
- ホストバイナリの `CFBundleExecutable` がホストアプリの実際のバイナリ名と一致しているか確認

### Extension がカメラ一覧に表示されない

1. `systemextensionsctl list` で Extension が `[activated enabled]` になっているか確認
2. コンソール.app で `KalidoKit` を検索してログを確認
3. `CMIOExtensionMachServiceName` が Info.plist で正しく設定されているか確認

### SIGKILL (プロセス即座に終了)

- `com.apple.developer.system-extension.install` エンタイトルメントを Provisioning Profile なしで使用した場合に発生
- このエンタイトルメントには Apple Developer Portal で発行した Provisioning Profile が必要

## 代替: Provisioning Profile + Developer ID (SIP 無効化不要)

SIP を無効化せずに開発するには:

1. [Apple Developer Portal](https://developer.apple.com) で App ID を作成
2. System Extension capability を有効化
3. Provisioning Profile を作成・ダウンロード
4. Developer ID Application 証明書で署名
5. `notarytool` で公証 (Notarization)

詳細は `docs/camera-extension-distribution.md` を参照。
