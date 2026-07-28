# ADR-0017：Session Tab 拥有独立的 Runtime Lifecycle

- 状态：Proposed
- 日期：2026-07-28
- 决策人：项目维护者

## 背景

AnySSH 的 Tauri `SessionRegistry` 已能按不透明 Session ID 保存多个
`SessionControl` 和独立 Output Acknowledgement Channel，但 React 当前只有一组
全局 Session State、一个 `TerminalPane`、一个 Host Key Dialog 和一个
Keyboard-interactive Dialog。

直接把多个连接复用到这些全局状态会造成 Terminal Output、Resize、Host Key
Decision、OTP Response、Error 和 Disconnect 被路由到错误 Session。只在切换 Tab
时重新创建 xterm.js 也会丢失 Scrollback，并可能让非活动 Session 因没有消费和
Ack Output 而停在八个 In-flight Chunk 的背压上限。

## 决策

- 每个 Session Tab 是独立的短生命周期 Runtime Object，拥有：
  - Frontend-local Tab ID。
  - 至多一个 Rust-issued SSH Session ID。
  - 独立 Connection Status、Endpoint/Host Metadata 和 Error。
  - 独立 Host Key/Changed-Key/Keyboard-interactive Pending Request。
  - 独立 xterm.js Instance、Terminal Size 和 Output Callback。
- Rust/Tauri `SessionRegistry` 继续是 Live SSH Control 与 Output Ack 的所有者。
  WebView 不生成、猜测或复用 Rust Session ID。
- v1 最多同时保留 8 个 Session Tab。达到上限后必须显式关闭 Tab，不能静默回收
  Live Session。
- 每个 Live/Connecting Tab 使用独立 Tauri Event Channel 和 Binary Data
  Channel；不得把多个 Session 合并到无 Session Scope 的全局 Event Bus。
- 非活动但仍存在的 Terminal 必须保持 Mounted 并继续消费 Output/发送 xterm
  Write Ack；UI 只隐藏其 Surface，不暂停 SSH Core 或复用另一 Tab 的 Terminal。
- Disconnect 与 Close 是不同操作：
  - Disconnect 终止 SSH Runtime，但保留 Tab、Scrollback 和最终状态。
  - Close Live/Connecting Tab 必须先请求 Disconnect，再移除 Tab。
  - Close 已结束 Tab 只移除本地 Runtime/UI State。
- Vault Lock、应用退出、WebView Channel 丢失或 Session Registry Drain 必须
  Fail Closed：取消 Pending Challenge、断开全部 Session、清除全部 Tab 和
  Terminal Buffer，不允许后台孤儿 Session。
- Host Key Decision 和 Keyboard-interactive Response 必须同时绑定 Tab 当前的
  Rust Session ID 与 Request ID。切换 Tab 会清空未提交的局部 Response Input，
  但不会把 Response 自动应用到另一 Tab。
- v1 Tab/Scrollback 不持久化，不在重启后自动恢复连接，也不共享或复用同一
  russh Transport。一个 Tab 对应一个独立 SSH Session。
- Browser QA 使用同样的 Tab/Session Routing Model，但只运行本地 Preview
  Session，不打开网络连接。

## 备选方案

- 继续使用单全局 Session，只做视觉 Tab：无法安全隔离 Event、Challenge 和
  Output，拒绝。
- 切换时销毁 xterm.js 并序列化 Buffer：复杂、容易丢失终端状态，并会破坏当前
  Output Ack 背压，v1 拒绝。
- 多 Tab 共享一个 SSH Transport/Connection Multiplexing：会把重连、Host Key、
  Authentication 和 Failure Domain 耦合，超出当前 russh Session Model，拒绝。
- 持久化 Tab、Scrollback 或自动重连：扩大敏感 Terminal Data 的存储和生命周期
  风险，留到独立设计。
- 达到上限时自动关闭最旧 Tab：可能在用户不知情时终止 Live Session，拒绝。

## 后果

### 正面

- Output、Input、Resize、Host Key、OTP 和 Disconnect 都有明确 Session Scope。
- 非活动 Tab 仍能持续排空 Output，不破坏现有八 Chunk Ack Window。
- Disconnect 后可保留 Scrollback 供用户检查，同时 Vault Lock 仍能彻底清空。
- Tauri 现有 Registry Map 和 SSH Core Session Control 可以复用，不需要改变
  Saved Host/Credential/Trust 安全边界。

### 代价与风险

- React State 将从单组字段重构为按 Tab ID 管理的 Reducer/Controller。
- 多个 xterm.js Instance 会增加内存与渲染成本，因此需要固定 Tab 上限。
- Close-during-connect、Channel Failure、同时多个 Challenge 和 Tab 切换存在竞态，
  必须使用 Tab Generation/存在性检查和 Disconnect-after-late-connect。
- Desktop/Mobile 都需要可访问、可滚动且不会遮挡 Terminal 的 Tab UI。

## 验证

- 两个及以上 Browser Preview Session 的 Output/Input/Status 完全隔离。
- 两个 Native OpenSSH Session 同时连接，关闭一个不会影响另一个。
- 非活动 Tab 排空大输出并继续执行后续命令。
- 每 Tab Host Key 和 Keyboard-interactive Request 不能交叉响应。
- Close Connecting/Authenticating/Connected/Closed Tab 的行为确定且无孤儿
  Registry Entry。
- Vault Lock 断开并清除全部 Tab、Pending Response 和 Terminal Buffer。
- X11、Wayland、Windows、Desktop/Mobile Browser、Android/Linux Build 和同
  Commit CI。

## 相关文档

- Design：[Multi Tab Terminal and Session Lifecycle v1](../design/multi-tab-session-lifecycle-v1.md)
- ExecPlan：[Multi Tab Terminal and Session Lifecycle](../execplans/active/0007-multi-tab-terminal-and-session-lifecycle.md)
- ADR：[ADR-0006](0006-secrets-stay-out-of-webview.md)
- ADR：[ADR-0010](0010-saved-host-plans-resolve-in-rust.md)
- ADR：[ADR-0015](0015-known-host-trust-is-endpoint-scoped.md)
- ADR：[ADR-0016](0016-keyboard-interactive-responses-are-session-bound.md)
- Supersedes：
- Superseded by：
