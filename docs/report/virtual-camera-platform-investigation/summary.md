# 仮想カメラ実装 — プラットフォーム調査・試行レポート

**日時**: 2026-03-12
**ステータス**: 調査完了 / macOS 動作確認は SIP 制約で保留 / Linux 実装方式確定

---

## 1. 概要

macOS と Linux の仮想カメラ実装について、複数の方式を調査・試行した。
本レポートは技術選定の過程と判断根拠を記録する。

---

## 2. macOS: CoreMediaIO Camera Extension

### 2.1 実装内容

UniCamEx (https://github.com/creativeIKEP/UniCamEx) を参考に、Objective-C で CoreMediaIO Camera Extension を実装した。

**構成:**
- `crates/virtual-camera/macos-extension/` — ObjC ソースコード (6 ファイル)
  - `ProviderSource.m`: `CMIOExtensionProviderSource` プロトコル実装
  - `DeviceSource.m`: デバイス管理、output stream + sink stream 作成
  - `StreamSource.m`: 出力ストリーム (Zoom/FaceTime 向けフレーム配信)
  - `SinkStreamSource.m`: 入力ストリーム (ホストアプリからのフレーム受信)
  - `ExtensionInstaller.m`: `OSSystemExtensionManager` 経由の Extension 登録
  - `main.m`: `CMIOExtensionProvider.startServiceWithProvider:` エントリポイント
- `crates/virtual-camera/src/macos.rs` — Rust 側 CoreMediaIO クライアント
- `crates/virtual-camera/build.rs` — ObjC コンパイル + フレームワークリンク

### 2.2 ObjC API 修正の試行錯誤

SDK ヘッダーとの不一致で多数の修正が必要だった:

| 問題 | 原因 | 修正 |
|------|------|------|
| `initWithLocalizedName:deviceID:legacyID:` | 引数名が `legacyDeviceID:` | SDK ヘッダー確認して修正 |
| `connectClient:error:` の戻り値 `OSStatus` | 実際は `BOOL` | SDK ヘッダー確認 |
| `disconnectClient:error:` | error パラメータなし | `disconnectClient:` に修正 |
| `propertiesForProperties:error:` | セレクタ名が違う | `providerPropertiesForProperties:error:` |
| `startService:` | セレクタ名が違う | `startServiceWithProvider:` |
| `CFStringEncoding(...)` | コンストラクタではない | `u32` 型エイリアス、raw 値 `0x08000100` を使用 |
| `CFString::get_bytes` | メソッド名が違う | `CFString::bytes` |

### 2.3 Extension 登録の試行錯誤

Extension バイナリのビルド・署名は成功したが、`OSSystemExtensionManager` での登録で複数の障壁に遭遇した。

#### 試行 1: 直接バイナリ実行
- **結果**: プロセスは起動するが、CoreMediaIO daemon に登録されず仮想カメラとして認識されない
- **原因**: `CMIOExtensionProvider.startServiceWithProvider:` は launchd/sysextd 経由の起動を前提としている

#### 試行 2: スタンドアロン install-extension ツール (ad-hoc 署名)
- **結果**: `OSSystemExtensionErrorDomain Code 8` (Invalid code signature)
- **原因**: ad-hoc 署名は SIP 完全無効化が必要

#### 試行 3: `systemextensionsctl developer on` 有効化
- **結果**: 変わらず Code 8
- **原因**: developer mode だけでは不十分、SIP 側の制約

#### 試行 4: .app バンドル + Extension 埋め込み
- **結果**: `Code 4` (Extension not found in App bundle)
- **原因**: install-extension が独立バイナリで、Extension の親バンドルではなかった

#### 試行 5: CFBundleExecutable 不一致修正
- **結果**: Code 8 に戻る
- **原因**: Info.plist の `CFBundleExecutable` = `kalidokit-camera-extension` とバイナリ名 `com.kalidokit.rust.camera-extension` が不一致だった。修正後も SIP が原因で失敗

#### 試行 6: Apple Development 証明書で署名
- **結果**: Code 8
- **発見**: 証明書 `Apple Development: tkaria.info@gmail.com (3QNQKJ5J2J)` (TeamID: BL487W744V) が存在。host/extension 両方を同一 TeamID で署名したが、SIP 制約は解消せず

#### 試行 7: `com.apple.developer.system-extension.install` エンタイトルメント追加
- **結果**: SIGKILL (プロセス即座に終了)
- **原因**: restricted entitlement は Provisioning Profile なしでは使用不可。macOS の amfid が拒否

#### 試行 8: launchd 経由で Extension を直接起動
- **結果**: Extension プロセスは動作するが、CoreMediaIO に登録されない
- **確認**: `CMIOExtensionProvider.startServiceWithProvider:` は正常完了 (NSLog 確認)
- **確認**: `list-cmio-devices.m` で CMIO デバイス列挙 → FaceTime + OBS のみ、KalidoKit は表示されず
- **原因**: Camera Extension は sysextd 経由で cmio_server に登録される必要がある。launchd 直接起動では不可

### 2.4 SIP 状態の詳細

```
System Integrity Protection status: unknown (Custom Configuration).
  Kext Signing: disabled
  Kernel Integrity Protections: disabled
  Filesystem Protections: enabled
  (他は enabled)
```

この「部分無効化」では System Extension の登録に不十分。`csrutil disable` (完全無効化) が必要。

### 2.5 UniCamEx の方式との比較

UniCamEx は SIP を無効化していない。以下の正規ルートで動作:

1. Apple Developer Program 加入 (年額 $99)
2. Apple Developer Portal で Provisioning Profile 作成 (System Extension capability 有効)
3. `com.apple.developer.system-extension.install` エンタイトルメントを Profile で許可
4. Hardened Runtime 有効化

現環境には Apple Development 証明書はあるが Provisioning Profile がない。

### 2.6 macOS まとめ

| 項目 | 状態 |
|------|------|
| ObjC Camera Extension 実装 | 完了 |
| Rust CoreMediaIO クライアント | 完了 |
| ビルド・署名 | 成功 |
| Extension 単体動作 | 確認済み (`startServiceWithProvider:` 正常) |
| Extension 登録 (OSSystemExtensionManager) | **未完了** — SIP 完全無効化 or Provisioning Profile が必要 |

**必要な対応** (いずれか):
1. Recovery Mode で `csrutil disable` (再起動必要)
2. Apple Developer Portal で Provisioning Profile 作成 (再起動不要)

### 2.7 成果物

- `scripts/build-app-bundle.sh` — ホストアプリ + Extension の .app バンドル自動ビルドスクリプト
- `scripts/list-cmio-devices.m` — CoreMediaIO デバイス列挙ツール
- `scripts/test-extension-install.m` — Extension 登録テストツール
- `docs/camera-extension-dev-setup.md` — 開発セットアップ手順 (更新済み)
- `docs/camera-extension-distribution.md` — 配布用署名・公証手順

---

## 3. Linux: 仮想カメラ方式の調査

### 3.1 調査した方式

#### 方式 1: DMA-BUF Zero-Copy (wgpu → PipeWire)

**結論: 不採用**

| 調査項目 | 結果 |
|----------|------|
| wgpu からの VkImage エクスポート | wgpu-hal に `texture_from_raw()` はあるがエクスポート API なし |
| `VK_EXT_external_memory_dma_buf` | Vulkan 拡張自体は存在するが wgpu が公開していない |
| wgpu-hal 内部 API への依存 | unsafe + API 安定性なし。将来の wgpu 更新で破壊リスク |
| NVIDIA NVK ドライバー | `VK_EXT_external_memory_dma_buf` 未対応 |
| 既存の成功事例 | pw-capture は raw Vulkan (ash) で実装。wgpu 経由の事例なし |

#### pw-capture (https://github.com/EHfive/pw-capture)

**結論: 統合不可**

- LD_PRELOAD 型の Vulkan/OpenGL 傍受レイヤー
- アプリケーション内にライブラリとして組み込めない
- `pw-capture-client` クレートはフレーム消費側であり、生成側ではない

#### libfunnel (https://github.com/hoshinolina/libfunnel)

**結論: 不採用**

- DMA-BUF 経由の PipeWire フレーム共有ライブラリ
- raw Vulkan / EGL / GBM を直接操作する必要がある
- wgpu が Vulkan ハンドルを公開しないため統合が困難

#### 方式 4: v4l2loopback 直接書き込み

**結論: フォールバックとして採用**

- CPU readback + BGRA→YUYV 変換 → `/dev/videoN` に write
- 動作実績豊富、レガシーアプリ互換性が高い
- root 必要 (カーネルモジュール)、レイテンシ 30-100ms

#### PipeWire 仮想カメラノード

**結論: 主方式として採用**

| 利点 | 詳細 |
|------|------|
| root 不要 | ユーザースペースで動作 |
| Chrome 127+ 対応 | PipeWire カメラソースを直接認識 |
| Firefox 116+ 対応 | `media.webrtc.camera.allow-pipewire` で有効化 |
| サンドボックス対応 | Flatpak/Snap 環境で動作 (xdg-desktop-portal 経由) |
| フォーマット柔軟 | BGRA をそのまま渡せる (YUYV 変換不要の場合あり) |

`pipewire` Rust クレート (0.8+) で安全な Rust バインディングが利用可能。

### 3.2 Linux まとめ

```
                    ┌──────────────┐
                    │   wgpu       │
                    │  (render)    │
                    └──────┬───────┘
                           │ CPU readback (BGRA)
                           ▼
                    ┌──────────────┐
                    │ virtual-     │
                    │ camera crate │
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              ▼                         ▼
    ┌──────────────┐          ┌──────────────┐
    │  PipeWire    │          │ v4l2loopback │
    │  (主方式)    │          │ (フォールバック) │
    │  root 不要   │          │  root 必要   │
    └──────┬───────┘          └──────┬───────┘
           │                         │
           ▼                         ▼
    Chrome 127+ /             レガシーアプリ
    Firefox 116+
```

---

## 4. 決定事項まとめ

| プラットフォーム | 方式 | 状態 |
|-----------------|------|------|
| **macOS** | CoreMediaIO Camera Extension (ObjC) | 実装完了、登録は SIP/Profile 待ち |
| **Linux (主)** | PipeWire 仮想カメラノード (`pipewire` crate) | features.md に TODO 積み済み |
| **Linux (副)** | v4l2loopback フォールバック (`v4l` crate) | features.md に TODO 積み済み |

### 不採用方式と理由

| 方式 | 不採用理由 |
|------|-----------|
| DMA-BUF zero-copy (wgpu) | wgpu が外部メモリ API 未公開、wgpu-hal 依存は破壊リスク大 |
| pw-capture | LD_PRELOAD 型、ライブラリ統合不可 |
| libfunnel | raw Vulkan 必須、wgpu と組み合わせ不可 |
| Swift Camera Extension | ObjC で同等の機能を実現可能、swift-bridge の複雑さを回避 |
