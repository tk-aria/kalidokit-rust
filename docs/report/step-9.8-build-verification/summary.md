# Step 9.8: ビルド検証 — GitHub Actions ビルド & ローカル動作確認

## 作業日時
2026-03-11 15:41 JST

## 実行した操作

### 1. テストタグ push (v0.2.1-rc.1)
```bash
git tag v0.2.1-rc.1
git push origin v0.2.1-rc.1
```

### 2. GitHub Actions ビルド結果 (run ID: 22940005956)

| ジョブ | 結果 | 所要時間 |
|-------|------|---------|
| Build (aarch64-apple-darwin) | ✅ 成功 | 2m48s |
| Build (x86_64-pc-windows-msvc) | ✅ 成功 | 6m42s |
| Build (x86_64-unknown-linux-gnu) | ❌ 失敗 | - |

### 3. Linux ビルド失敗原因
- ORT ソースビルド時に Eigen の FetchContent ダウンロードでハッシュ不一致エラー
- GitLab の ZIP アーカイブのハッシュが不安定な既知の問題
- 修正: Eigen 事前クローンを ORT ビルドステップに復元

### 4. macOS バイナリのローカル動作確認
```bash
# アーティファクトダウンロード
gh run download 22940005956 --name kalidokit-rust-aarch64-apple-darwin --dir /tmp/kalidokit-download

# モデルダウンロード
KALIDOKIT_MODELS_PATH=assets/models sh scripts/setup.sh download-models

# 実行 (10秒タイムアウト)
RUST_LOG=info timeout 10 ./kalidokit-rust
```

### 5. macOS バイナリ実行結果
```
[2026-03-11T06:41:35Z INFO  kalidokit_rust::tracker_thread] Tracker worker thread started
[2026-03-11T06:41:37Z WARN  kalidokit_rust::init] Failed to initialize webcam: Failed to create camera. Falling back to dummy frames.
```

- 起動: OK (パニックなし)
- Tracker スレッド: 正常開始
- Webcam: カメラ未接続のため `Failed to create camera` → ダミーフレームにフォールバック (想定通り)
- 終了: timeout (exit code 124) で正常停止

### 6. Linux ビルド修正
- `release.yml` に Eigen 事前クローンを復元
- `FETCHCONTENT_SOURCE_DIR_EIGEN=/tmp/eigen-src` を ORT ビルドフラグに追加
- 新しいテストタグ (v0.2.1-rc.2) で再ビルド予定
