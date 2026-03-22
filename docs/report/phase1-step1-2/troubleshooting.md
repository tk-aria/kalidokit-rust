# Phase 1 Step 1-2: troubleshooting

## エラー: thiserror の `source` フィールド自動解釈

### 症状
```
error[E0599]: the method `as_dyn_error` exists for reference `&String`, but its trait bounds were not satisfied
```

### 原因
thiserror は `source` という名前のフィールドに自動的に `#[source]` を付与する。
`#[source]` は `std::error::Error` trait の実装を要求するが、`String` はこれを実装していない。

### 解決
`Load` variant の `source: String` を `reason: String` にリネーム。
`#[error]` 属性内のフォーマットも `{source}` → `{reason}` に変更。
