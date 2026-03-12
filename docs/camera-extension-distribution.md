# macOS Camera Extension — 配布用署名 & 公証

## 前提条件

- Apple Developer Program メンバーシップ ($99/年)
- Developer ID Application 証明書
- Developer ID Installer 証明書 (pkg 配布の場合)

## 1. Developer ID 証明書の取得

```bash
# Xcode で自動管理するか、Apple Developer Portal で手動作成:
# https://developer.apple.com/account/resources/certificates/list
#
# 必要な証明書:
# - Developer ID Application: コード署名用
# - Developer ID Installer: pkg 配布用 (オプション)
```

## 2. Provisioning Profile の作成

Camera Extension には `com.apple.developer.system-extension.install` entitlement が必要です。
この entitlement は **Provisioning Profile 経由** でのみ付与されます。

1. Apple Developer Portal → Identifiers で App ID を作成
   - Bundle ID: `com.kalidokit.rust`
   - Capabilities: System Extension を有効化
2. Profiles → Distribution → Developer ID で Profile を作成
3. ダウンロードして Xcode に登録

## 3. Extension の署名

```bash
# Developer ID で署名
./scripts/build-camera-extension.sh \
    --sign "Developer ID Application: Your Name (TEAMID)"

# 署名の確認
codesign -dvv target/camera-extension/KalidoKitCamera.appex
```

## 4. 公証 (Notarization)

Apple の公証サービスにバイナリを提出し、マルウェアでないことを証明します。

```bash
# .appex を zip に圧縮
ditto -c -k --keepParent \
    target/camera-extension/KalidoKitCamera.appex \
    target/camera-extension/KalidoKitCamera.zip

# 公証に提出
xcrun notarytool submit \
    target/camera-extension/KalidoKitCamera.zip \
    --apple-id "your@email.com" \
    --team-id "TEAMID" \
    --password "@keychain:AC_PASSWORD" \
    --wait

# 公証チケットを添付 (staple)
xcrun stapler staple target/camera-extension/KalidoKitCamera.appex
```

## 5. ホストアプリへの埋め込み

System Extension は `.app` バンドル内の以下のパスに配置する必要があります:

```
KalidoKit.app/
└── Contents/
    ├── MacOS/
    │   └── kalidokit-rust              # メインバイナリ
    ├── Library/
    │   └── SystemExtensions/
    │       └── KalidoKitCamera.appex   # Camera Extension
    ├── Info.plist
    └── Entitlements.plist
```

ホストアプリの Entitlements に追加:
```xml
<key>com.apple.developer.system-extension.install</key>
<true/>
<key>com.apple.security.device.camera</key>
<true/>
```

## 6. Extension のインストール (プログラマティック)

```objc
// OSSystemExtensionManager を使用して Extension を登録
OSSystemExtensionRequest *request =
    [OSSystemExtensionRequest
        activationRequestForExtension:@"com.kalidokit.rust.camera-extension"
        queue:dispatch_get_main_queue()];
request.delegate = self;
[OSSystemExtensionManager.sharedManager submitRequest:request];
```

## 注意事項

- 公証なしの署名済みバイナリは macOS Gatekeeper にブロックされます
- System Extension のインストールにはユーザーの明示的な承認が必要です (セキュリティ設定)
- Apple Silicon Mac ではセキュリティポリシーが追加で適用される場合があります
