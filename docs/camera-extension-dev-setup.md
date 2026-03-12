# macOS Camera Extension — 開発用セットアップ

## 前提条件

- macOS 12.3+ (Monterey 以降)
- Xcode 14+ (Command Line Tools)

## SIP 無効化 (開発用)

署名なしの System Extension をロードするには SIP (System Integrity Protection) を無効化する必要があります。

### 手順

1. Mac を再起動し、起動時に **Command + R** を長押しして Recovery Mode に入る
   - Apple Silicon Mac: 電源ボタンを長押し → Options → Continue
2. メニューバーから **Utilities → Terminal** を選択
3. 以下のコマンドを実行:
   ```bash
   csrutil disable
   ```
4. Mac を再起動

### SIP を再度有効化する (開発完了後)

```bash
# Recovery Mode で実行
csrutil enable
```

### SIP の状態確認

```bash
csrutil status
# System Integrity Protection status: disabled.
```

## Camera Extension のビルドとインストール

### 1. Extension バンドルをビルド

```bash
./scripts/build-camera-extension.sh
```

生成物: `target/camera-extension/KalidoKitCamera.appex`

### 2. Extension を手動でロード (開発用)

SIP 無効化状態では、Extension バンドルを以下のパスに配置してロードできます:

```bash
# .appex を Library に配置
sudo cp -r target/camera-extension/KalidoKitCamera.appex \
    /Library/SystemExtensions/

# systemextensionsd を再起動
sudo killall -HUP systemextensionsd
```

### 3. 動作確認

```bash
# インストール済み Extension の確認
systemextensionsctl list

# FaceTime でカメラ一覧を確認
# "KalidoKit Virtual Camera" が表示されるはず
```

## トラブルシューティング

### Extension がカメラ一覧に表示されない

1. SIP が無効化されているか確認: `csrutil status`
2. Info.plist の `CMIOExtensionMachServiceName` が正しいか確認
3. コンソール.app で `KalidoKit` を検索してログを確認

### "Operation not permitted" エラー

- SIP が有効: Recovery Mode で `csrutil disable` を実行
- Entitlements 不足: Extension.entitlements に `com.apple.security.app-sandbox` があるか確認
