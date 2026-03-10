# Step 8.3: ハンドボーン適用 (左右各16ボーン)

## 完了日時
2026-03-10 12:58 JST

## 変更ファイル
- `crates/app/src/update.rs` — `apply_rig_to_model()` にハンドボーン適用追加、`apply_hand_bones()` ヘルパー関数追加
- `crates/app/Cargo.toml` — `serde_json` dev-dependency 追加 (テスト用)

## 実行コマンド
```bash
cargo check --workspace        # OK
cargo test -p solver -p vrm -p renderer  # 62 tests passed
```

## 実装内容
1. `apply_hand_bones()` ヘルパー: RiggedHand の snake_case フィールド → HumanoidBoneName PascalCase マッピング
2. Wrist 回転合成: X/Y は Hand solver、Z は Pose solver から取得
3. 左右各16ボーン (Wrist + 5指 × 3関節) を `set_rotation_interpolated()` で適用
4. limbs config (dampener=1.0, lerp=0.3) を使用
5. テスト2件追加: 左手16ボーンの回転適用確認、Wrist 合成ロジック確認
