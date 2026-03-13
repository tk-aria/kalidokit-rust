# macOS CMIOExtension 仮想カメラ — ad-hoc 署名での実装レポート

## 概要

kalidokit-rust の macOS 仮想カメラ機能について、Apple Developer Program（有料）に加入せず **ad-hoc 署名のみ** で動作させるために必要だった調査・実装・問題解決の全記録。

最終的に、ホスト（Rust アプリ）から CMIOExtension（Objective-C）へ POSIX 共有メモリ経由でフレームを転送し、AVFoundation 上に仮想カメラとして表示するパイプラインを確立した。

## 背景

### CMIOExtension とは

macOS 12.3 以降、仮想カメラは **CMIOExtension**（Camera Extension）フレームワークで実装する。旧来の CMIO DAL プラグインは非推奨。CMIOExtension は System Extension として登録され、sandbox 内で `_cmiodalassistants` ユーザーとして実行される。

### 参考: UniCamEx のアーキテクチャ

[UniCamEx](https://github.com/creativeIKEP/UniCamEx) は Unity アプリから仮想カメラへフレームを送信する OSS。以下の正規 API フローを使用している:

1. ホストが `CMIODeviceStartStream` で Extension の sink stream を起動
2. Extension が `consumeSampleBufferFromClient:` でフレームを受信
3. 受信したフレームを output stream に `sendSampleBuffer:` で転送
4. `notifyScheduledOutputChanged:` でフレーム消費を通知

**この方式は Apple Developer 署名（Team ID 付き）が前提**であり、ad-hoc 署名では動作しない。

## 解決した課題一覧

### 課題 1: sink stream API が ad-hoc 署名で機能しない

| 項目 | 内容 |
|------|------|
| **現象** | ホスト側の `CMIODeviceStartStream` は成功（return 0）するが、Extension 側の `startStreamAndReturnError:` が呼ばれない |
| **原因** | CMIO フレームワーク内部の sink stream ブリッジが、Apple Developer 署名の Team ID を検証している |
| **試行** | UniCamEx パターン（`consumeSampleBuffer` + `notifyScheduledOutputChanged`）を実装したが、sink stream が起動しないため機能せず |
| **対処** | sink stream API を諦め、独自の IPC（共有メモリ）を実装 |

### 課題 2: CMIOExtension sandbox によるファイルアクセス制限

Extension プロセスは `/System/Library/Sandbox/Profiles/cmioextension.sb` の sandbox 内で実行される。

| 試行した IPC パス | 結果 | 詳細 |
|---|---|---|
| `/dev/shm/kalidokit_vcam_frame` (POSIX shm_open) | NG | sandbox が Apple 指定名のみ許可 |
| `/private/tmp/kalidokit_vcam_frame` (ファイル mmap) | NG | `errno=1 EPERM` — `/private/tmp` への全アクセスがブロック |
| `/Library/Caches/kalidokit_vcam_frame` (ファイル mmap) | NG | `errno=1 EPERM` — sandbox で `/Library/Caches` が明示的に deny |
| `temporary-exception` entitlement で特定パスを許可 | NG | sandbox profile コンパイルエラーでプロセス即クラッシュ（`failed to compile sandbox profile`）。ad-hoc 署名では使用不可 |
| Application Group コンテナ（ファイル） | NG | Extension 側 `/var/db/cmiodalassistants/Library/Group Containers/com.kalidokit.rust/` にホスト（一般ユーザー）からアクセス不可 |
| **POSIX shm（グループプレフィックス付き）** | **OK** | `shm_open("com.kalidokit.rust/vcam_frame")` が sandbox で許可された |

**解決の鍵**: sandbox プロファイルの以下のルール:

```scheme
;; cmioextension.sb 内の application-groups セクション
(sandbox-array-entitlement
  "com.apple.security.application-groups"
  (lambda (suite)
    ...
    (allow ipc-posix* (ipc-posix-name-prefix (string-append suite "/")))))
```

Extension の entitlements に `com.apple.security.application-groups` = `com.kalidokit.rust` を宣言すると、`com.kalidokit.rust/` プレフィックスの POSIX 共有メモリが読み書き可能になる。

### 課題 3: Extension バージョン管理

| 項目 | 内容 |
|------|------|
| **現象** | `CFBundleVersion` を変更せずに再インストールすると、macOS は新しいバイナリを `terminated_waiting_to_uninstall_on_reboot` にし、古いバイナリを使い続ける |
| **原因** | `OSSystemExtensionManager` がバージョン比較で「同じバージョン」と判断し、アップグレード不要と判断 |
| **対処** | Extension 更新時は `CFBundleVersion` / `CFBundleShortVersionString` を必ずインクリメントする |

### 課題 4: CFBundleExecutable とバイナリ名の不一致

| 項目 | 内容 |
|------|------|
| **現象** | Extension が `activated enabled` だが、AVFoundation にカメラデバイスが現れない |
| **原因** | `CFBundleExecutable` = `kalidokit-camera-extension` だが、sysextd / codesign がバンドル ID と同名の `com.kalidokit.rust.camera-extension` を検証・起動しようとする |
| **対処** | `CFBundleExecutable` をバンドル ID と同じ `com.kalidokit.rust.camera-extension` に統一し、ビルド出力名も一致させた |

### 課題 5: launchd stale エントリ

| 項目 | 内容 |
|------|------|
| **現象** | `systemextensionsctl reset` 後に再インストールすると `Submit job failed: service = CMIOExtension.com.kalidokit.rust.camera-extension, error = 17: File exists` |
| **原因** | reset で db.plist はクリアされるが、`user/262`（`_cmiodalassistants`）ドメインの launchd ジョブが残留 |
| **対処** | `sudo launchctl bootout user/262/CMIOExtension.com.kalidokit.rust.camera-extension` で stale エントリを削除。根本対策として `reset` は避け、バージョンアップによる上書きインストールを優先 |

### 課題 6: ホストアプリの entitlement 欠落

| 項目 | 内容 |
|------|------|
| **現象** | インストーラーアプリ実行後、Extension が登録されない（ログにもエラーが出ない） |
| **原因** | `codesign` 時に `--entitlements` を指定せず、`com.apple.developer.system-extension.install` entitlement が付与されていなかった |
| **対処** | インストーラーアプリの署名時に entitlement ファイルを指定 |

### 課題 7: developer mode のリセット

| 項目 | 内容 |
|------|------|
| **現象** | `systemextensionsctl reset` 後に Extension が `activated waiting for user`（承認待ち）で停止 |
| **原因** | reset で developer mode フラグもリセットされる |
| **対処** | reset 後に `systemextensionsctl developer on` を再実行。SIP 無効が前提 |

### 課題 8: sink subscribe ループによる CPU 100% / ログ洪水

| 項目 | 内容 |
|------|------|
| **現象** | Extension の CPU 使用率が 100% 超、`Invalid not streaming` エラーが毎秒数千回ログに出力され、他のログが rate-limit で消失 |
| **原因** | `ProviderSource.m` の `connectClient:` で sink stream の `subscribeWithClient:` を即座に呼んでいたが、sink stream がまだ streaming 状態でないため失敗→再帰的にリトライ |
| **対処** | `connectClient:` からの subscribe 呼び出しを削除。subscribe は `SinkStreamSource` の `startStreamAndReturnError:` 内でのみ行う（ただし ad-hoc 署名ではこのメソッド自体が呼ばれない） |

## 最終アーキテクチャ

```
┌─────────────────────┐     POSIX shm      ┌─────────────────────────┐
│   ホスト (Rust)      │  ───────────────►  │  CMIOExtension (ObjC)   │
│                     │  "com.kalidokit.   │                         │
│  MacOsVirtualCamera │   rust/vcam_frame" │  StreamSource           │
│  - shm_open()       │                    │  - shm_open() (読取)     │
│  - mmap() (読書)     │                    │  - mmap() (読取)         │
│  - RGBA→BGRA 変換    │                    │  - 30fps タイマーポーリング │
│  - ヘッダ + ピクセル   │                    │  - sendSampleBuffer:     │
│    データ書込み       │                    │                         │
└─────────────────────┘                    └───────────┬─────────────┘
                                                       │
                                                       ▼
                                              ┌────────────────┐
                                              │  AVFoundation   │
                                              │  (Zoom等が取得)  │
                                              └────────────────┘
```

### 共有メモリレイアウト

| オフセット | サイズ | 内容 |
|-----------|--------|------|
| 0 | 4 bytes | frame_counter (u32 LE) — フレーム毎にインクリメント |
| 4 | 4 bytes | width (u32 LE) |
| 8 | 4 bytes | height (u32 LE) |
| 12 | 4 bytes | reserved |
| 16 | width × height × 4 bytes | BGRA ピクセルデータ |

ホスト側はピクセルデータ → width/height → frame_counter の順で書き込み、frame_counter の前に Release fence を挿入してリーダーとの同期を取る。

Extension 側は 30fps のタイマーで frame_counter をチェックし、前回と異なる場合のみフレームを読み取って IOSurface-backed CVPixelBuffer にコピーし、`sendSampleBuffer:` で出力する。frame_counter が変わらない場合はテストパターン（赤→緑→青の循環）を生成する。

## ファイル構成

### Extension (Objective-C)

| ファイル | 役割 |
|---------|------|
| `macos-extension/main.m` | エントリポイント。ProviderSource 生成→CMIOExtensionProvider サービス開始 |
| `macos-extension/ProviderSource.m` | CMIOExtensionProviderSource 実装。デバイス登録、クライアント接続管理 |
| `macos-extension/DeviceSource.m` | CMIOExtensionDeviceSource 実装。output stream + sink stream 生成 |
| `macos-extension/StreamSource.m` | output stream 実装。POSIX shm ポーリング、テストパターン生成、フレーム送信 |
| `macos-extension/SinkStreamSource.m` | sink stream 実装（Apple Developer 署名時のみ機能。ad-hoc では未使用） |
| `macos-extension/Info.plist` | バンドル設定（CFBundleExecutable = バンドルID、CMIOExtensionMachServiceName） |
| `macos-extension/Extension.entitlements` | sandbox + application-groups entitlement |

### ホスト (Rust)

| ファイル | 役割 |
|---------|------|
| `src/lib.rs` | `VirtualCamera` トレイト定義（`start`, `send_frame`, `stop`） |
| `src/macos.rs` | `MacOsVirtualCamera` 実装。shm_open/mmap でフレーム書き込み、RGBA→BGRA 変換 |

### ビルド・デプロイ

| ファイル | 役割 |
|---------|------|
| `scripts/build-app-bundle.sh` | KalidoKit.app + 内蔵 Extension のビルド・署名 |
| `scripts/build-camera-extension.sh` | Extension 単体ビルド |
| `scripts/install-extension.m` | OSSystemExtensionManager による Extension 登録ツール |

## Extension デプロイ手順

### 前提条件

- macOS 13.x 以降
- SIP 無効（`csrutil disable`）— developer mode に必要
- Xcode Command Line Tools インストール済み

### ビルド＆インストール

```bash
# 1. Extension ビルド（バイナリ名 = バンドルID）
EXT_SRC="crates/virtual-camera/macos-extension"
clang -fobjc-arc -fmodules \
    -framework CoreMediaIO -framework CoreMedia \
    -framework CoreVideo -framework Foundation \
    -I "$EXT_SRC" \
    -o /tmp/com.kalidokit.rust.camera-extension \
    "$EXT_SRC/main.m" "$EXT_SRC/ProviderSource.m" \
    "$EXT_SRC/DeviceSource.m" "$EXT_SRC/StreamSource.m" \
    "$EXT_SRC/SinkStreamSource.m"

# 2. インストーラー .app にバイナリ + Info.plist を配置
INSTALLER_EXT="<installer.app>/Contents/Library/SystemExtensions/\
com.kalidokit.rust.camera-extension.systemextension/Contents"
cp /tmp/com.kalidokit.rust.camera-extension "$INSTALLER_EXT/MacOS/"
cp "$EXT_SRC/Info.plist" "$INSTALLER_EXT/Info.plist"

# 3. 署名（Extension → ホストアプリの順）
codesign --force --sign - \
    --entitlements "$EXT_SRC/Extension.entitlements" \
    "<extension.systemextension>"
codesign --force --sign - \
    --entitlements host.entitlements \
    "<installer.app>"

# 4. インストール（sudo 1 回で完結させる）
sudo bash -c '
    systemextensionsctl developer on
    open <installer.app>
    sleep 12
    systemextensionsctl list
'
```

### バージョンアップ時

Info.plist の `CFBundleVersion` / `CFBundleShortVersionString` をインクリメントしてから上記手順を実行する。`systemextensionsctl reset` は launchd stale エントリ問題を引き起こすため使用しない。

## 動作検証結果

```
$ /tmp/vcam_verify
[VERIFY] Available camera: KalidoKit Virtual Camera (A8D7B8AA-...)
[VERIFY] Using: KalidoKit Virtual Camera
[VERIFY] Starting capture session...
[VERIFY] First frame: 1280x720 format=0x42475241
[VERIFY] Center pixel (BGRA): B=255 G=0 R=0 A=255    ← ホストが書き込んだ青色
[VERIFY] Received 10 frames total. SUCCESS!
```

ホストから POSIX shm に書き込んだフレーム（青色 BGRA）が、仮想カメラ経由で AVFoundation に正しく配信されることを確認。

## Apple Developer 署名への移行時

Apple Developer Program に加入した場合、以下の変更で UniCamEx 方式（sink stream API）に切り替え可能:

1. Extension と ホストアプリを Team ID 付きで署名
2. `StreamSource.m` の shm ポーリングを削除
3. `SinkStreamSource.m` の sink stream → output stream 転送を有効化
4. ホスト側を `CMIODeviceStartStream` + `CMSimpleQueue` 経由の送信に変更
5. `macos.rs` の shm_open/mmap コードを CMIO C API 呼び出しに置き換え

shm 方式と比較して、sink stream 方式はフレームワークが XPC 転送を管理するため、メモリ管理やタイミング同期が不要になる。

## 参考資料

- [CMIOExtension ドキュメント](https://developer.apple.com/documentation/coremediaio/creating-a-camera-extension-with-core-media-i-o)
- [UniCamEx (GitHub)](https://github.com/creativeIKEP/UniCamEx) — Unity → 仮想カメラ送信の参考実装
- `/System/Library/Sandbox/Profiles/cmioextension.sb` — CMIOExtension sandbox プロファイル
