# Step 10.2: CoreMediaIO Camera Extension (Objective-C) — 作業報告

**日時**: 2026-03-12 15:35 JST
**ステータス**: 完了

## 実行した操作

### 1. ディレクトリ作成
```bash
mkdir -p crates/virtual-camera/macos-extension
```

### 2. main.m 作成
- `CMIOExtensionProvider startServiceWithProvider:` でサービス起動
- `CFRunLoopRun()` でイベントループ実行

### 3. ProviderSource.h/.m 作成
- `CMIOExtensionProviderSource` プロトコル実装
- `connectClient:error:` → BOOL 返り値 (SDK に合わせて修正)
- `disconnectClient:` → void (error パラメータなし)
- `providerPropertiesForProperties:error:` (正しいセレクタ名に修正)

### 4. DeviceSource.h/.m 作成
- `CMIOExtensionDeviceSource` プロトコル実装
- `initWithLocalizedName:deviceID:legacyDeviceID:source:` (SDK 正確なシグネチャ)
- output stream + sink stream の両方を追加

### 5. StreamSource.h/.m 作成
- `CMIOExtensionStreamSource` プロトコル実装
- 1280x720 BGRA 30fps フォーマット
- `enqueueBuffer:` で sink から受けたフレームを出力に転送

### 6. SinkStreamSource.h/.m 作成
- `consumeSampleBufferFromClient:completionHandler:` で再帰的サブスクリプション
- UniCamEx パターンに従い `dispatch_async` で再サブスクリプション

### 7. Info.plist / Extension.entitlements 作成
- Mach service name: `com.kalidokit.rust.camera-extension`
- App sandbox + application groups entitlement

### 8. コンパイル検証
```bash
# ObjC syntax check (clang)
for f in crates/virtual-camera/macos-extension/*.m; do
  clang -fsyntax-only -fobjc-arc -fmodules -I crates/virtual-camera/macos-extension "$f"
done
# 全ファイル: エラー 0, 警告 0

# Rust crate check
cargo check -p virtual-camera
# 結果: 成功
```

## SDK シグネチャ修正 (troubleshooting)
- `initWithLocalizedName:deviceID:legacyID:source:` → `legacyDeviceID:` に修正
- `connectClient:error:` → 返り値を `OSStatus` → `BOOL` に修正
- `disconnectClient:error:` → `disconnectClient:` (error パラメータ削除)
- `propertiesForProperties:error:` → `providerPropertiesForProperties:error:` に修正
- `startService:` → `startServiceWithProvider:` に修正
- `props.frameDuration = CMTimeMake(...)` → NSDictionary 形式に修正
- `kIOAudioDeviceTransportTypeBuiltIn` → `0x626C746E` (直接値)
- `stream.clients` → `authorizedToStartStreamForClient:` でクライアント保持
