# 実装比較: playwright-go vs meet-bot-rs (chromiumoxide)

## 概要

Google Meet bot の2つの実装を比較する。

| 項目 | playwright-go (Go) | meet-bot-rs (Rust) |
|------|-------------------|-------------------|
| 言語 | Go | Rust |
| ブラウザ制御 | playwright-go | chromiumoxide |
| プロトコル | Playwright Protocol | Chrome DevTools Protocol (CDP) |
| アーキテクチャ | BrowserDriver抽象化 | Clean Architecture (Ports & Adapters) |

---

## アーキテクチャ比較

### playwright-go

```
┌─────────────────────────────────────────────────────────┐
│                    cmd/botserver/main.go                │
└───────────────────────────┬─────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────┐
│                  internal/bot/bot_driver.go             │
│                      MeetBotV2                          │
└───────────────────────────┬─────────────────────────────┘
                            │ BrowserDriver interface
┌───────────────────────────▼─────────────────────────────┐
│              internal/driver/playwright/driver.go       │
│                   Driver (playwright-go)                │
└─────────────────────────────────────────────────────────┘
```

### meet-bot-rs

```
┌─────────────────────────────────────────────────────────┐
│                        src/main.rs                      │
│                    (Dependency Injection)               │
└───────────────────────────┬─────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────┐
│              presentation/handlers/*.rs                 │
│                   HTTP Handlers (axum)                  │
└───────────────────────────┬─────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────┐
│            application/use_cases/join_meeting.rs        │
│                    JoinMeetingUseCase                   │
└───────────────────────────┬─────────────────────────────┘
                            │ BrowserPort trait
┌───────────────────────────▼─────────────────────────────┐
│         infrastructure/browser/chrome_browser.rs        │
│                 ChromeBrowser (chromiumoxide)           │
└─────────────────────────────────────────────────────────┘
```

---

## ブラウザ接続方式

### playwright-go

```go
// LaunchPersistentContext で新規ブラウザを起動
persistentOptions := playwright.BrowserTypeLaunchPersistentContextOptions{
    Headless: playwright.Bool(d.config.Headless),
    Channel:  playwright.String("chrome"),  // システムChromeを使用
    Args: []string{
        "--use-fake-ui-for-media-stream",
        "--use-fake-device-for-media-stream",
    },
    Viewport: &playwright.Size{
        Width:  d.config.WindowWidth,
        Height: d.config.WindowHeight,
    },
    Permissions: []string{"microphone", "camera", "notifications"},
}

browserContext, err := pw.Chromium.LaunchPersistentContext(d.tempDir, persistentOptions)
```

**特徴**:
- Playwright が Chromium/Chrome を起動・管理
- Persistent context で Bot 検出回避
- 権限は起動時に設定

### meet-bot-rs

```rust
// 既存の Chrome インスタンスに CDP で接続
pub async fn connect(cdp_url: &str) -> DomainResult<Self> {
    // /json/version から WebSocket URL を取得
    let ws_url = Self::get_debugger_url(cdp_url).await?;

    // CDP WebSocket 接続
    let (browser, mut handler) = Browser::connect(&ws_url).await?;

    // イベントハンドラを別タスクで実行
    tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            debug!("Browser event: {:?}", event);
        }
    });
}
```

**特徴**:
- 外部で起動された Chrome に接続
- Docker 環境では entrypoint.sh で Chrome を起動
- CDP 直接接続でより低レベルな制御が可能

---

## 要素操作方式

### playwright-go: Selector ベース

```go
// Click
func (d *Driver) Click(selector string) error {
    return d.page.Click(selector, playwright.PageClickOptions{
        Timeout: playwright.Float(5000),
    })
}

// Type
func (d *Driver) Type(selector string, text string) error {
    return d.page.Fill(selector, text)
}

// ElementExists
func (d *Driver) ElementExists(selector string) (bool, error) {
    locator := d.page.Locator(selector)
    count, err := locator.Count()
    return count > 0, err
}
```

### meet-bot-rs: JavaScript 評価ベース

```rust
// CSS セレクタでの click も可能だが...
async fn click(&self, selector: &str) -> DomainResult<()> {
    let page = self.get_page().await?;
    let element = page.find_element(selector).await?;
    element.click().await?;
    Ok(())
}

// 実際の JoinMeetingUseCase では JavaScript 評価を使用
let join_script = r#"
    (function() {
        let btn = document.querySelector('button[aria-label="Ask to join"]');
        if (!btn) btn = document.querySelector('button[aria-label="Join now"]');
        if (!btn) btn = document.querySelector('button[aria-label*="join" i]');

        if (!btn) {
            const buttons = document.querySelectorAll('button');
            for (const b of buttons) {
                const text = b.textContent.toLowerCase();
                if (text.includes('ask to join') || text.includes('join now')) {
                    btn = b;
                    break;
                }
            }
        }

        if (btn && !btn.disabled) {
            btn.click();
            return 'clicked';
        }
        return 'not_found';
    })()
"#;

let result = self.browser.evaluate(join_script).await?;
```

**理由**: Google Meet は `role="textbox"` などカスタム要素を使用しており、標準CSSセレクタでは要素が見つからない場合がある。JavaScript評価でより柔軟に対応。

---

## 機能比較

| 機能 | playwright-go | meet-bot-rs |
|------|:-------------:|:-----------:|
| ミーティング参加 | ✅ | ✅ |
| ミーティング退出 | ✅ | ✅ |
| スクリーンショット | ✅ | ✅ |
| 字幕取得 | ✅ | ⚠️ (TODO) |
| チャット送信 | ✅ | ⚠️ (TODO) |
| 参加者一覧 | ✅ | ❌ |
| 画面録画 | ✅ | ❌ |
| Webhook通知 | ✅ | ❌ |
| Bot検出回避 | ✅ (Persistent Context) | ⚠️ (Chrome フラグのみ) |
| Docker対応 | ✅ | ✅ |
| noVNC対応 | ✅ | ✅ |

---

## エラーハンドリング

### playwright-go

```go
func (b *MeetBotV2) clickJoinButton() error {
    // 標準セレクタを試す
    if exists, _ := b.driver.ElementExists(SelectorJoinButton); exists {
        return b.driver.Click(SelectorJoinButton)
    }

    // JavaScript フォールバック
    var clicked bool
    if err := b.driver.EvaluateWithResult(jsScript, &clicked); err != nil {
        return fmt.Errorf("join button not found: %w", err)
    }
    if clicked {
        return nil
    }
    return fmt.Errorf("join button not found")
}
```

### meet-bot-rs

```rust
let join_result = self.browser.evaluate(join_script).await?;

if join_result.contains("clicked") {
    info!("Clicked join button via JavaScript");
} else if join_result.contains("disabled") {
    return Err(DomainError::ElementNotFound(
        "Join button is disabled (name may not be entered)".to_string(),
    ));
} else {
    return Err(DomainError::ElementNotFound(
        "Join button not found".to_string(),
    ));
}
```

---

## セレクタ定義

### playwright-go (internal/bot/selectors.go)

```go
const (
    SelectorNameInput           = `input[aria-label="Your name"]`
    SelectorJoinButton          = `button[aria-label="Ask to join"], button[aria-label="Join now"]`
    SelectorDialogClose         = `button[aria-label="Close"]`
    SelectorDismissButton       = `button[aria-label="Dismiss"]`
    SelectorGotItButton         = `button[aria-label="Got it"]`
    SelectorMicButton           = `button[aria-label*="microphone"]`
    SelectorCamButton           = `button[aria-label*="camera"]`
    SelectorCaptionButton       = `button[aria-label*="caption"]`
    SelectorCaptionRegion       = `[role="region"][aria-label*="caption"]`
    SelectorCaptionText         = `.caption-text, [data-caption-text]`
    SelectorLeaveButton         = `button[aria-label="Leave call"]`
    SelectorChatPanelButton     = `button[aria-label*="chat"]`
    SelectorChatInput           = `textarea[aria-label*="message"]`
    SelectorChatSendButton      = `button[aria-label="Send"]`
    SelectorParticipantPanelButton = `button[aria-label*="participant"]`
    SelectorParticipantName     = `[data-participant-name], .participant-name`
    SelectorLanguageDropdown    = `[aria-label*="language"]`
)
```

### meet-bot-rs (infrastructure/browser/selectors.rs)

```rust
pub const NAME_INPUT: &str = r#"input[aria-label="Your name"]"#;
pub const JOIN_BUTTON: &str = r#"button[aria-label="Ask to join"]"#;
pub const JOIN_NOW_BUTTON: &str = r#"button[aria-label="Join now"]"#;
pub const CLOSE_BUTTON: &str = r#"button[aria-label="Close"]"#;
pub const DISMISS_BUTTON: &str = r#"button[aria-label="Dismiss"]"#;
pub const LEAVE_BUTTON: &str = r#"button[aria-label="Leave call"]"#;
pub const CAPTION_REGION: &str = r#"[role="region"][aria-label*="caption"]"#;
pub const CAPTION_TEXT: &str = r#".caption-text"#;
pub const CHAT_INPUT: &str = r#"textarea[aria-label*="message"]"#;
pub const CHAT_SEND: &str = r#"button[aria-label="Send"]"#;
```

**注意**: 実際の join_meeting.rs では JavaScript 内にセレクタを直接埋め込んでいる（より柔軟なマッチングのため）。

---

## 状態管理

### playwright-go

```go
type MeetBotV2 struct {
    // 状態管理
    isJoined   bool
    isLoggedIn bool
    mu         sync.RWMutex

    // 字幕収集
    captions    []CaptionEntry
    captionChan chan CaptionEntry
}
```

### meet-bot-rs

```rust
// domain/entities/bot_state.rs
pub struct BotState {
    is_in_meeting: bool,
    is_waiting_for_admission: bool,
    current_meeting_url: Option<String>,
    joined_at: Option<DateTime<Utc>>,
}

// Arc<RwLock<BotState>> で共有
```

---

## テスト方式

### playwright-go

```go
// 統合テストが主（実際のブラウザを使用）
func TestMeetBotV2_JoinMeeting(t *testing.T) {
    cfg := config.DefaultConfig()
    bot, err := NewMeetBotV2(cfg)
    // ...
}
```

### meet-bot-rs

```rust
// Mock を使用した単体テスト
struct MockBrowser {
    navigate_called: std::sync::atomic::AtomicBool,
}

#[async_trait]
impl BrowserPort for MockBrowser {
    async fn navigate(&self, _url: &str) -> DomainResult<()> {
        self.navigate_called.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
    async fn evaluate(&self, script: &str) -> DomainResult<String> {
        if script.contains("btn.click()") {
            Ok("clicked".to_string())
        } else {
            Ok("true".to_string())
        }
    }
    // ...
}

#[tokio::test]
async fn test_join_meeting_sets_waiting_state() {
    let browser = Arc::new(MockBrowser::new());
    let state = Arc::new(RwLock::new(BotState::new()));
    let use_case = JoinMeetingUseCase::new(browser.clone(), state.clone());

    let result = use_case
        .execute("https://meet.google.com/abc-defg-hij", "TestBot")
        .await;

    assert!(result.is_ok());
    assert!(state.read().await.is_waiting_for_admission());
}
```

---

## 依存関係

### playwright-go

```
go.mod:
- github.com/playwright-community/playwright-go v0.4001.0
- github.com/chromedp/chromedp (代替ドライバー)
```

### meet-bot-rs

```
Cargo.toml:
- chromiumoxide = "0.7"
- tokio = { version = "1", features = ["full"] }
- axum = "0.7"
- async-trait = "0.1"
- tracing = "0.1"
```

---

---

## 重要な機能差分 (cdpserver2 固有機能)

playwright-go / cdpserver2 には meet-bot-rs に未実装の重要な機能がある:

### 1. Claude CLI 連携 (AI自動応答)

```go
// cmd/cdpserver2/main.go:1337-1399

func (s *CDPServer) generateAIResponse(captionText string) (string, error) {
    return s.callClaude(captionText, true)
}

func (s *CDPServer) callClaude(captionText string, useSession bool) (string, error) {
    prompt := fmt.Sprintf(`あなたは「%s」という名前のミーティングアシスタントです。
ユーザーがあなたの名前を呼びました。以下の会話文脈に基づいて、短く親しみやすい返答を日本語で生成してください。

会話文脈:
%s

返答:`, s.botName, captionText)

    claudePath := os.Getenv("CLAUDE_PATH")
    if claudePath == "" {
        claudePath = "/usr/local/bin/claude"
    }
    args := []string{"-p", "--dangerously-skip-permissions", "--output-format", "json"}

    if useSession && s.claudeSessionID != "" {
        args = append(args, "--resume", s.claudeSessionID)
    }
    args = append(args, prompt)

    cmd := exec.Command(claudePath, args...)
    // ... セッション管理、JSON パース ...
}
```

**機能**:
- Bot名がメンション（字幕/チャット）されたら Claude CLI を実行
- セッションID を保持して会話を継続
- 生成された応答をチャットに送信

### 2. メンション検知 (字幕 + チャット)

```go
// cmd/cdpserver2/main.go:1172-1186

func (s *CDPServer) checkCaptions() {
    // ...
    // Check for bot name mention
    if s.botName != "" && newText != "" {
        if time.Since(s.lastMentionTime) < s.mentionCooldown {
            return
        }

        nameVariations := s.getNameVariations()
        for _, name := range nameVariations {
            if strings.Contains(newText, name) {
                log.Printf("[MENTION] Bot name '%s' detected", name)
                s.lastMentionTime = time.Now()
                go s.respondToMentionWithDelay(name, newText)
                break
            }
        }
    }
}

// 名前のバリエーション対応
func (s *CDPServer) getNameVariations() []string {
    variations := []string{s.botName}
    nameMap := map[string][]string{
        "あいちゃん": {"愛ちゃん", "アイちゃん"},
        "愛ちゃん":   {"あいちゃん", "アイちゃん"},
    }
    // ...
}
```

**機能**:
- 字幕とチャットを定期的に監視
- Bot名（複数バリエーション対応）が検出されたら応答
- クールダウン機能で連続応答を防止

### 3. 自動参加フロー (join後の自動設定)

```go
// cmd/cdpserver2/main.go:874-933

func (s *CDPServer) waitForAdmissionAndSetupCaptions() {
    // ... 入室待機 ...

    if s.checkIfAdmitted() {
        // Setup captions
        for attempt := 1; attempt <= 3; attempt++ {
            s.dismissDialogs()
            s.enableCaptionsWithLang()
            if s.verifyCaptionsEnabled() {
                break
            }
        }

        // Set language
        for i := 0; i < 3; i++ {
            if s.trySetMeetingLanguage() {
                break
            }
        }

        s.startCaptionWatcher()
        s.sendChatMessage("よろしくお願いします")  // 入室挨拶
    }
}
```

**機能**:
- 入室承認を待機（ポーリング）
- 承認後に自動で字幕ON + 言語設定
- 字幕監視開始 + 挨拶メッセージ送信

### 4. 字幕言語設定

```go
// cmd/cdpserver2/main.go:1031-1077

func (s *CDPServer) trySetMeetingLanguage() bool {
    langMap := map[string]string{
        "ja": "Japanese",
        "en": "English",
    }

    // Click dropdown
    jsCode := `
        (() => {
            const dropdown = document.querySelector('[aria-label="Meeting language"]');
            if (dropdown) { dropdown.click(); return true; }
            return false;
        })()
    `
    // ... 言語選択 ...
}
```

**機能**:
- 起動時に `-lang ja` フラグで言語指定
- Meeting language ドロップダウンを操作して設定

### 5. 字幕ログ出力

```go
// cmd/cdpserver2/main.go:255-298

func (s *CDPServer) openCaptionLog(meetingURL string) error {
    s.captionLogPath = generateCaptionLogPath(meetingURL)
    file, err := os.OpenFile(s.captionLogPath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0644)
    // ...
}

func (s *CDPServer) writeCaptionLog(caption string) {
    if s.captionLogFile != nil {
        timestamp := time.Now().Format("15:04:05")
        entry := fmt.Sprintf("[%s] %s\n", timestamp, caption)
        s.captionLogFile.WriteString(entry)
    }
}
```

**機能**:
- `logs/captions/<meeting-code>_<timestamp>.log` にリアルタイム出力
- 開始/終了ヘッダー付き

### 6. カメラ/マイク自動OFF

```go
// cmd/cdpserver2/main.go:763-805

func (s *CDPServer) turnOffCameraLocked() {
    selectors := []string{
        `button[aria-label*="Turn off camera"]`,
        `button[aria-label*="カメラをオフ"]`,
    }
    // ...
}

func (s *CDPServer) turnOffMicrophoneLocked() {
    selectors := []string{
        `button[aria-label*="Turn off microphone"]`,
        `button[aria-label*="マイクをオフ"]`,
    }
    // ...
}
```

---

## 機能比較表 (詳細版)

| 機能 | playwright-go | cdpserver2 (Go) | meet-bot-rs |
|------|:-------------:|:---------------:|:-----------:|
| **基本機能** |
| ミーティング参加 | ✅ | ✅ | ✅ |
| ミーティング退出 | ✅ | ✅ | ✅ |
| スクリーンショット | ✅ | ✅ | ✅ |
| **字幕機能** |
| 字幕有効化 | ✅ | ✅ | ⚠️ TODO |
| 字幕取得 | ✅ | ✅ | ⚠️ TODO |
| 字幕言語設定 | ✅ | ✅ | ❌ |
| 字幕ログ出力 | ❌ | ✅ | ❌ |
| **チャット機能** |
| チャット送信 | ✅ | ✅ | ⚠️ TODO |
| チャット受信 | ✅ | ✅ | ❌ |
| **AI連携** |
| Claude CLI連携 | ❌ | ✅ | ❌ |
| メンション検知 | ❌ | ✅ | ❌ |
| 自動応答 | ❌ | ✅ | ❌ |
| **入室時自動設定** |
| カメラ自動OFF | ❌ | ✅ | ❌ |
| マイク自動OFF | ❌ | ✅ | ❌ |
| 入室挨拶 | ❌ | ✅ | ❌ |
| **その他** |
| 参加者一覧 | ✅ | ❌ | ❌ |
| 画面録画 | ✅ | ❌ | ❌ |
| Webhook通知 | ✅ | ❌ | ❌ |
| Bot検出回避 | ✅ | ⚠️ 一部 | ⚠️ 一部 |
| Docker対応 | ✅ | ✅ | ✅ |
| noVNC対応 | ✅ | ✅ | ✅ |

---

## 今後の課題 (meet-bot-rs)

### 優先度: 高
1. **字幕機能**: 字幕有効化、取得、言語設定
2. **チャット機能**: 送受信の完全実装
3. **カメラ/マイク自動OFF**: 入室前に自動設定

### 優先度: 中
4. **Claude CLI連携**: メンション検知 + AI応答
5. **字幕ログ出力**: ファイルへのリアルタイム出力
6. **入室時自動設定**: 字幕ON + 言語設定 + 挨拶

### 優先度: 低
7. **Bot検出回避**: Persistent context 相当の実装
8. **画面録画**: ffmpeg/GStreamer 連携
9. **Webhook通知**: イベント通知システム
10. **参加者一覧**: 参加者取得機能

---

## 推奨事項

| 用途 | 推奨 | 理由 |
|------|------|------|
| 迅速なプロトタイプ | playwright-go | 機能が充実、実績あり |
| 本番運用 (Docker) | meet-bot-rs | メモリ効率、型安全性 |
| 低レベル制御が必要 | meet-bot-rs | CDP直接アクセス |
| 多機能が必要 | playwright-go | 字幕/録画/Webhook実装済み |
