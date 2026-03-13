# CLAUDE.md - Project Rules for kalidokit-rust

## Project Overview

Rust implementation of KalidoKit (VRM motion capture). Uses wgpu + winit for rendering, ort for ML inference, nokhwa for webcam capture.

Workspace crates: `app`, `renderer`, `vrm`, `solver`, `tracker`

## Implementation Rules

### features.md のチェックボックス運用ルール

1. **チェックをつける条件**: 以下の **全て** を満たすこと
   - コードが存在し、`cargo check` が通る
   - **他のクレート/モジュールから実際に呼び出されている**（デッドコードでない）
   - **実行して動作が確認できている**、または実行不可能な環境の場合はその旨を明記して `[ ]` のままにする

2. **「コードが存在する」だけではチェックしない**: 構造体・関数が定義されていても、統合（パイプラインへの組み込み、update ループでの呼び出し等）がされていなければ未完了

3. **ダミー実装・TODO コメント付きコードは未完了**: `// TODO` コメントがある時点でそのチェックボックスは `[ ]` でなければならない

4. **ヘッドレス環境での制約を明記する**: GPU/ウィンドウ/カメラ等の実行検証ができない場合、チェックボックスを `[ ]` のままにし `— ヘッドレス環境のため未検証` と注記する

### コード品質ルール

1. **`cargo check` が通る ≠ 動作する**: コンパイル成功だけで完了としない。特に以下は実行確認が必須:
   - winit の `ApplicationHandler` イベントハンドラ（`about_to_wait`, `resumed` 等）
   - wgpu のレンダーパス・パイプライン
   - カメラキャプチャの初期化と毎フレーム取得

2. **統合テストの確認**: 新しいモジュールを追加したら、以下を確認する:
   - そのモジュールが実際に呼び出し元から使われているか
   - VrmModel 等のメイン構造体にフィールドが追加されているか
   - update ループやレンダーパイプラインに組み込まれているか

3. **デッドコードを残さない**: 実装したが統合していないコードがある場合、features.md に `[ ]` で統合タスクを追記する

## Build & Test Commands

```bash
cargo check --workspace          # 型チェック
cargo test -p renderer -p vrm -p solver  # 単体テスト (tracker は ort-sys リンク制約で除外)
cargo clippy --workspace -- -D warnings  # lint
cargo fmt --check                # フォーマット
cargo build --release            # リリースビルド (要 ONNX Runtime)
```

## Key Architecture Notes

- winit 0.30: `ApplicationHandler` trait 実装。`about_to_wait()` で毎アイドル `request_redraw()` を呼ぶことでレンダーループを駆動
- wgpu Metal backend: macOS 13.x 互換のため `.cargo/config.toml` で CoreML シンボルを `-U` リンカフラグで許容
- ORT ビルド (Linux): cargo-zigbuild で glibc 2.17+ ターゲットにクロスビルド。musl ワークアラウンドは不要
- features.md が実装計画の信頼できる唯一のソース (Single Source of Truth)

## macOS CMIOExtension (仮想カメラ) 運用ルール

### ユーザーに sudo コマンドを振らない

- Extension のデプロイ・更新に必要な sudo 操作は **1つのスクリプトにまとめる**こと
- `systemextensionsctl reset` → `developer on` → `launchctl bootout` → `open Installer.app` のような複数ステップを個別にユーザーに依頼しない
- `/tmp/fix_and_install.sh` のように `sudo ./script.sh` 一発で完結させる

### Extension バージョン管理

- `CFBundleVersion` / `CFBundleShortVersionString` を更新しないと macOS は同じ Extension とみなし、新しいバイナリを `terminated_waiting_to_uninstall_on_reboot` にして古い方を使い続ける
- Extension を更新する際は **必ずバージョンを上げる**こと
- インストーラーアプリ（ホスト）と Extension 両方の Info.plist のバージョンを合わせる

### `CFBundleExecutable` とバイナリ名

- sysextd/codesign は **バンドル ID と同名のバイナリ** (`com.kalidokit.rust.camera-extension`) を優先的に検証・起動する場合がある
- `CFBundleExecutable` の値とビルド出力のバイナリ名は必ず一致させること
- 現在の正しい値: `com.kalidokit.rust.camera-extension`

### launchd stale エントリ問題

- `systemextensionsctl reset` を繰り返すと `user/262` (\_cmiodalassistants) ドメインに stale な launchd ジョブ `CMIOExtension.com.kalidokit.rust.camera-extension` が残る
- 新規インストール時に `Submit job failed: error = 17: File exists` となり Extension プロセスが起動しない
- 修正: `sudo launchctl bootout user/262/CMIOExtension.com.kalidokit.rust.camera-extension` の後に再インストール
- **reset は安易に行わない**。バージョンアップによる上書きインストールを優先する

### CMIOExtension sandbox 制約

- Extension プロセスは `_cmiodalassistants` ユーザーで sandbox 内実行される
- `/private/tmp/`、`/Library/Caches/` へのファイルアクセスは sandbox でブロックされる（errno=1 EPERM）
- `temporary-exception.files.absolute-path.read-only` entitlement は ad-hoc 署名で sandbox profile コンパイルエラーを起こしクラッシュする
- **解決済み**: `com.apple.security.application-groups` entitlement で `com.kalidokit.rust` グループを宣言すると、sandbox が `com.kalidokit.rust/` プレフィックスの POSIX shm を許可する（`ipc-posix*` ルール）
- 現在の IPC: `shm_open("com.kalidokit.rust/vcam_frame")` で POSIX 共有メモリを使用

### Extension デプロイ手順（正規フロー）

```bash
# 1. ビルド（バイナリ名 = バンドルID）
clang -fobjc-arc -fmodules ... -o /tmp/com.kalidokit.rust.camera-extension ...

# 2. インストーラーに配置 + Info.plist バージョン更新 + 署名
cp バイナリ → Installer.app 内
cp Info.plist → Installer.app 内
codesign --force --sign - --entitlements Extension.entitlements <extension bundle>
codesign --force --sign - --entitlements host.entitlements <installer app>

# 3. インストール（sudo スクリプト1発で実行）
sudo /tmp/fix_and_install.sh
```
