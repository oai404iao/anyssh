# ADR-0020：Appearance、Font 与 Snippet 是受限数据而不是可执行扩展

- 状态：Proposed
- 日期：2026-07-29
- 决策人：项目维护者

## 背景

AnySSH Desktop MVP 已完成核心 SSH、Vault、Key、Trust、Multi Tab 和 Forwarding。
下一阶段需要把 App Theme、Terminal Theme、自定义 Font 和 Snippet 从硬编码原型
提升为可持久化产品能力。

主题包若允许 CSS、JavaScript 或远程资源，会绕过 CSP 和现有 WebView 边界。
字体导入若让 WebView提交任意 Path，会重新引入本地文件读取能力。Snippet 若使用
`eval`、本地 Shell 或可执行插件，会破坏 ADR-0008，并可能读取 Vault、环境变量和
文件系统。

## 决策

- App Theme、Terminal Theme、Font Profile 和 Snippet 都建模为带版本、大小与数量
  上限的 Typed Data；不加载脚本、插件、远程 URL 或任意 CSS。
- App Theme v1 提供 `system`、`dark` 和 `light`。Terminal Theme 与 App Theme
  分离；自定义 Terminal Theme JSON 必须包含 `schemaVersion`，颜色只接受规范化
  十六进制值和固定字段集合。
- System Font 由 Rust/平台层枚举。自定义 Font 通过 Rust-owned Native Picker
  导入并复制到应用管理目录；原始 Path 不进入 WebView、普通 IPC、日志或持久化
  DTO。WebView 只获得 Font ID、Family、Style、Format 和来源元数据。
- Font 文件不是 Credential Secret，但仍拒绝 Symlink/Reparse Point、特殊文件、
  远程资源、超大文件和无法解析的 Font。自定义 Font 通过受限应用协议按 Opaque
  Font ID 提供给 WebView。
- Appearance Settings、Custom Terminal Theme Metadata、Imported Font Metadata
  和 Snippet Repository 使用 SQLCipher Schema v8。Snippet Body 额外使用现有
  Record AEAD；Imported Font Binary 作为非秘密、完整性校验的应用管理 Asset
  保存，不放入 SQLCipher BLOB。
- Snippet 是发送到当前已连接 SSH PTY 的有界命令模板，不是本地脚本：
  - 不调用本地 Shell。
  - 不使用 `eval`、JavaScript、Rhai、Starlark 或第三方 Plugin。
  - 变量使用固定 `{{name}}` 语法和 Literal Substitution。
  - 普通 Run IPC 只提交 Session ID、Snippet ID、变量值和是否追加 Enter；
    Rust 从 Vault 解析并渲染 Body。
  - 显式 Edit 才把 Snippet Body 返回 Request-local React State；关闭、提交、
    Lock 或切换 Workspace 后清空。
  - 多行 Send/Run 必须显示完整 Preview 并要求显式确认。
- v1 不提供 Secret Variable、Credential 插值、自动触发、定时执行、Host 批量
  Runbook 或本地文件/网络访问。用户不得把 Password、Token 或 Private Key 直接
  存入 Snippet；未来 Secret Binding 必须另建 Rust-only 设计。
- Browser QA 只模拟 Theme/Font Metadata 和 Snippet Repository/Terminal Send，
  不打开 Font/Theme Picker，不读取本地文件。

## 备选方案

- 让 Theme 包携带 CSS/JavaScript：会扩大 Renderer 执行面并绕过设计令牌，拒绝。
- WebView 使用 File Input 导入 Font/Theme：让 Renderer 获得文件选择和内容，
  与 Native Picker 边界冲突，拒绝。
- 把 Font Binary 存为 SQLCipher BLOB：会让大型字体显著膨胀数据库、WAL 和迁移
  成本；字体不是秘密，v1 使用完整性校验的应用管理 Asset。
- Snippet 直接保存在 `localStorage`：无法统一 Vault Lock、迁移、备份与未来同步，
  拒绝。
- Snippet 通过本地 Shell 或通用脚本 Runtime 执行：违反 ADR-0008，拒绝。
- 在 React 中长期缓存全部 Snippet Body：增加意外泄漏面，拒绝。

## 后果

### 正面

- 主题和字体可产品化，同时保持 CSP、Path 和文件边界。
- Snippet 能复用当前 SSH Session，而不会引入本地代码执行能力。
- Schema v8 为未来 E2EE Sync 提供稳定对象模型。
- Snippet Body 默认不随列表进入 WebView，降低包含敏感命令时的暴露范围。

### 代价与风险

- 需要 Schema v7 -> v8 Migration、Font Asset 生命周期和跨文件/数据库恢复策略。
- Tauri 需要受限 Font Protocol 与 Linux/Windows Native Picker QA。
- xterm.js Mounted Terminal 必须在 Appearance 变化时原地更新，不能破坏 Tab、
  Output Ack 或 Scrollback。
- 用户仍可主动把 Secret 写进 Snippet；v1 只能警告、加密存储并减少常驻暴露。

## 验证

- Schema v7 -> v8 成功、重启、中断回滚和旧 Repository 保持。
- Theme JSON Unknown Field、Remote URL、非法 Color、超限和脚本字段拒绝。
- Font 普通文件/大小/Format/Parse、Symlink/Reparse Point、删除与选中回退测试。
- Snippet CRUD、Record AEAD、变量 Parser、重复/缺失/多余变量、Size/Count、
  Multi-line Confirmation 和 Vault Lock 清理测试。
- Browser Desktop/Mobile Theme/Font/Snippet UI 和 Error Log。
- X11/Wayland/Windows Native Font Import、Appearance、真实 SSH Snippet Marker。
- Linux/Android/Windows Build 与同 Commit CI。

## 相关文档

- Design：[Terminal Appearance, Font, and Snippet v1](../design/terminal-appearance-font-and-snippet-v1.md)
- ExecPlan：[Terminal Appearance, Font, and Snippet Productization](../execplans/active/0010-terminal-appearance-font-and-snippet-productization.md)
- ADR：[ADR-0006](0006-secrets-stay-out-of-webview.md)
- ADR：[ADR-0008](0008-no-arbitrary-local-scripting-in-mvp.md)
- ADR：[ADR-0017](0017-session-tabs-own-independent-runtime-lifecycles.md)
- Supersedes：
- Superseded by：
