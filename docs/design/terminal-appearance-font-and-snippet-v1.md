# Terminal Appearance, Font, and Snippet v1

## 1. 目标

在不引入可执行 Theme/Plugin、本地 Shell 或任意文件读取的前提下，为 AnySSH
提供可持久化的 App Theme、Terminal Theme、Font Profile 和 Snippet 工作流。

v1 必须保持：

- Appearance 更新不销毁 Mounted xterm.js，不丢失 Scrollback 或 Output Ack。
- Font/Theme 原始文件 Path 不进入 WebView。
- Snippet 默认按 Summary 列表，Body 只在显式 Edit 时进入 Request-local UI。
- Snippet Run 只发送到指定的 Live SSH Session，不执行本地代码。

## 2. 范围

### 2.1 包含

- App Theme：System、Dark、Light。
- Built-in Terminal Theme 与 Versioned Custom Terminal Theme JSON。
- Bundled Font、System Font 枚举和 Linux/Windows Native Custom Font Import。
- Font Size、Line Height、Ligature 开关和 East Asian Ambiguous Width 设置。
- Snippet CRUD、`{{variable}}`、Insert/Run、Multi-line Preview/Confirmation。
- SQLCipher Schema v8、Typed Tauri IPC、Browser QA 和 Native Evidence。

### 2.2 不包含

- 任意 CSS、JavaScript、Theme Marketplace 或 Remote Font。
- Android Document Provider Font Import 和 iOS Picker。
- Secret Variable、Credential 插值、自动触发、批量 Host Runbook。
- 本地 Shell、`eval`、Plugin、Rhai 或 Starlark。
- Per-Host/Group Appearance Override；v1 是 Vault-wide Setting。
- Snippet Output 持久化、Schedule 或 Trigger。

## 3. 数据模型

Schema v8 在现有 v7 Repository 上新增：

### 3.1 Appearance Settings

单例记录：

```text
app_theme                 system | dark | light
terminal_theme_id         built-in 或 custom Theme ID
font_source_kind          bundled | system | imported
font_id                   Opaque Font ID / System Family Selector
font_size                 10..32
line_height_millis        1000..2000
ligatures_enabled         bool
ambiguous_width           narrow | wide
updated_at                Unix milliseconds
```

Migration 创建默认记录，保持当前视觉：

```text
app_theme = dark
terminal_theme_id = builtin:obsidian
font_source_kind = bundled
font_id = builtin:anyssh-nerd-mono
font_size = 13
line_height_millis = 1420
ligatures_enabled = false
ambiguous_width = narrow
```

### 3.2 Terminal Theme

Custom Theme 使用 CSPRNG Opaque ID、Label、Schema Version 和固定 Palette。
颜色只允许 `#RRGGBB` 或 `#RRGGBBAA`，字段包括：

```text
background foreground cursor cursor_accent selection_background
black red green yellow blue magenta cyan white
bright_black bright_red bright_green bright_yellow
bright_blue bright_magenta bright_cyan bright_white
```

JSON 使用 `deny_unknown_fields`；禁止 URL、CSS Function、Variable、Image、Font
和 Script 字段。每个文件最大 32 KiB，最多 32 个 Custom Theme。

### 3.3 Imported Font

Metadata：

```text
id family style format sha256 size_bytes created_at
```

Font Binary 位于 Vault Root 下的应用管理 `font-assets/`，文件名只由 Opaque ID
和受控 Format 生成。Binary 不是 Secret，但必须：

- 由 Native Picker 返回给 Rust。
- 以普通文件、No-follow/Create-new 方式读取和复制。
- 限制为 `.ttf`、`.otf`、`.ttc`、`.woff2`，单文件最大 16 MiB。
- 至少解析出一个 Face，Family/Style 限长并清理控制字符。
- 保存 SHA-256，读取时校验大小与 Digest。
- Unix 使用私有目录与普通只读文件权限；Windows 拒绝 Reparse Point。
- 删除选中 Font 时先原子回退 Bundled Font，再清理 Asset。

最多保存 32 个 Imported Font。System Font 只返回 Family/Style Metadata，不复制。

### 3.4 Snippet

```text
id
label
body_ciphertext
body_nonce
variables_json
line_count
created_at
updated_at
```

限制：

- 最多 256 个 Snippet。
- Label 1..128 个字符。
- UTF-8 Body 1..65536 Bytes。
- 最多 16 个唯一 Variable。
- Variable Name 匹配 `[A-Za-z][A-Za-z0-9_]{0,31}`。
- Body 使用现有 Record AEAD；AAD 绑定 Schema、Entity ID 和字段用途。

List 只返回：

```text
id label variables line_count updated_at
```

显式 Edit 才返回 Body。Debug/Error/Log 不包含 Body 或 Variable Value。

## 4. Theme 与 Terminal Runtime

React 根节点按解析后的 App Theme 设置 `data-app-theme`。`system` 使用
`matchMedia("(prefers-color-scheme: dark)")`，监听系统变化但不改写保存值。

Design Token 使用 CSS Variable；Light/Dark 不通过重复组件样式实现。

`TerminalPane` 接收稳定的 `TerminalAppearance`：

```text
fontFamily
fontSize
lineHeight
ligaturesEnabled
ambiguousWidth
theme
```

Terminal 只在首次 Mount 时创建。Appearance 变化通过 `terminal.options` 和
受控 Addon 更新，然后 Visible Terminal 执行 Fit；Inactive Terminal 继续 Mounted
并 Ack Output，不因 Theme/Font 切换重建。

Custom Font 使用受限 `anyssh-font://<font-id>` Protocol 注册 `@font-face`。
Protocol 只接受规范 Opaque ID，不接受 Path、Query、Traversal 或任意 MIME。

## 5. Snippet 执行

### 5.1 Create/Edit

React Editor 可暂时持有当前 Snippet Body。提交、取消、Vault Lock、Workspace
切换或组件卸载立即清空。UI 明确警告不要保存 Password、Token 或 Private Key。

Rust 在保存时解析 Variable，并把 Canonical Variable List 放入 Summary。

### 5.2 Run

普通 Run Request：

```text
session_id
snippet_id
variables: map<string, string>
append_enter: bool
```

Rust：

1. 确认 Vault Unlocked。
2. 解析 Snippet ID 与 Live Session ID。
3. 检查变量集合完全匹配、每个值不超过 4096 Bytes、总渲染结果不超过 64 KiB。
4. 执行 Literal Substitution；不解释 Escape、Expression 或嵌套模板。
5. 多行请求必须带由当前 UI Preview 生成的短期 Confirmation Token，或由 Tauri
   在同一 Request 中使用显式 `confirmed_multiline=true` 且再次核验 Body。
6. 把渲染结果直接送入目标 SSH PTY；`append_enter` 时只追加一个 `\r`。

Snippet 不打开新的 Exec Channel，不访问本地 Shell、文件或网络。Session Closed、
Stale ID、Vault Lock 或 Tab Close 全部失败关闭。

## 6. IPC

WebView 可获得：

- `AppearanceSettings`
- `TerminalThemeSummary` 与已验证 Palette
- `FontSummary`
- `SnippetSummary`
- 显式 Edit 的单个 `SnippetDraft`

WebView 不可提交：

- Font/Theme Path。
- Arbitrary CSS、URL、Script 或 Font Bytes。
- Local Command、Shell、Working Directory 或 Environment。
- Credential ID/Secret Binding 作为 Snippet Variable。

所有 Request 使用 `deny_unknown_fields` 和 Rust 侧长度/字符验证。

## 7. Vault Lock 与失败处理

- Vault Lock 清空 Snippet Draft、Variable Form、Pending Multi-line Confirmation。
- Appearance 与已加载 Font 可继续显示，因为它们不是 Secret；Lock Gate 不允许
  再查询或修改 Repository。
- Migration、Theme Import 或 Font Import 失败不改变当前 Appearance。
- Font Asset 与 DB Metadata 使用 Staging File + Transaction/Compensation：
  DB Commit 失败删除 Staging；最终 Rename 失败回滚 Metadata；启动时清理过期
  Staging 和无引用 Asset。
- Browser QA 不创建本地 Asset。

## 8. 平台

- Linux X11/Wayland：GTK Native Picker、System Font Catalog、Custom Font Protocol。
- Windows：Native Picker、Reparse Point Guard、WebView2 Custom Font Protocol。
- Android/iOS：Built-in/System Font 与 Appearance 可构建；Custom Font Import
  v1 明确 Unsupported。

## 9. 验证

- Storage Migration、Repository、Record AEAD、Asset Recovery。
- Theme JSON/Color/Unknown Field/Remote Resource 拒绝。
- Font Parse、Size、Symlink/Reparse、Digest、Delete/Fallback。
- Snippet Parser、Literal Substitution、Variable Exact Match、Size/Count、
  Multi-line Confirmation、Stale Session 和 Vault Lock。
- Mounted Multi Tab Theme/Font Update 不影响 Output Ack 或 Scrollback。
- Browser Desktop/Mobile/Compact Screenshot 和 Empty Error Log。
- X11/Wayland/Windows 真实 Font Import 与 SSH Snippet Marker。
- Linux/Android/Windows Build 和同 Commit CI。
