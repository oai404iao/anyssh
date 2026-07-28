# Multi Tab Terminal and Session Lifecycle v1

> 状态：本地实现完成；等待 ExecPlan 0007 同 Commit CI 与 ADR 评审
> 日期：2026-07-28

本文定义 Phase 1 Desktop MVP 的多 Session Tab、Terminal Instance、Event
Routing、Close/Disconnect/Vault Lock 生命周期和 Browser/Native 验证。长期决策
见 Proposed ADR-0017。

## 目标

- 同时打开最多 8 个独立 SSH Session Tab。
- 每 Tab 隔离 Output、Input、Resize、Status、Host Key 和
  Keyboard-interactive Challenge。
- 非活动 Tab 继续排空 Terminal Output，不破坏 Core/Tauri/xterm Ack 背压。
- Disconnect 保留 Scrollback，Close 删除 Tab，Vault Lock 清空全部。
- Quick Connection 和 Saved Host 都能创建新 Tab。
- Desktop 和 Mobile 都能识别活动 Session、Pending Action 和 Closed/Error。

## 非目标

- SSH Connection Multiplexing 或多个 PTY 共享一个 Transport。
- Tab/Scrollback 持久化、应用重启恢复或自动重连。
- Background Mobile Session、Keepalive Policy 或网络切换恢复。
- Split Pane、Broadcast Input、Synchronized Typing 或 Session Group。
- Terminal Search、Copy Mode、Recording 或加密 Session Log。
- Forwarding UI；Forwarding 仍属于后续 ExecPlan。

## 当前实现

- Tauri `SessionRegistry` 是 `HashMap<String, SessionEntry>`，每个 Entry 有独立
  `SessionControl` 和 Output Ack Sender。
- `register_spawned_session` 为每次连接创建独立 Event/Data Channel Pump；Channel
  丢失时显式 `remove_and_disconnect`。
- React `App.tsx` 使用 Ref-backed Immutable Session Controller，按 Tab ID 和
  Generation 更新独立 Status、Form、Rust Session ID、Pending Host Key、
  Changed-Key、Authentication 和 Terminal Size。
- 每个 Tab 有独立 `TerminalPane` 和 xterm.js。Inactive Terminal 保持 Mounted，
  但 `visible` Guard 阻止隐藏 Surface 执行 Fit/发送 0x0 Resize。
- Vault Lock 在 Rust 侧 `disconnect_all`，前端同时替换为一个全新 Quick Tab 并
  清除全部 Terminal Ref、Challenge 和临时 Password。

## Frontend Session Model

Session Controller 管理的等价模型为：

```ts
type SessionTabId = string;

interface SessionTab {
  tabId: SessionTabId;
  generation: number;
  title: string;
  source:
    | { kind: "quick"; form: QuickConnectionDraft }
    | { kind: "savedHost"; hostId: string };
  nativeSessionId: string | null;
  status: ConnectionStatus;
  statusDetail: string;
  error: string | null;
  pendingHostKey: HostKeyEvent | null;
  changedHostKey: HostKeyChangedEvent | null;
  pendingAuthentication: AuthenticationChallengeEvent | null;
  terminalSize: { columns: number; rows: number };
}
```

响应、Quick Temporary Password 和 Terminal Instance 不进入 Reducer：

- Keyboard-interactive Response 继续只存在于按 Request ID 重建的 Dialog
  Local State。
- Quick Password 只存在于当前 Tab 的局部 Draft Form，并在 Submit/Cancel/
  Disconnect/Close/Lock 时清空。
- xterm.js Handle 存在 `Map<TabId, TerminalHandle>` Ref，不可序列化到 State。

每个异步 Connect Attempt 捕获 `tabId + generation`。若 `connectSsh` 返回时 Tab
已关闭或 Generation 已变化，立即对返回的 Rust Session ID 调用 Disconnect，
不得把它附着到新 Tab。

## Lifecycle

```text
Draft
  -> Connecting
    -> Verifying Host Key
    -> Authenticating
    -> Connected
    -> Error
    -> Closed

Connected --Disconnect--> Closed (Tab/Scrollback retained)
Any state --Close--> Disconnect if needed -> Removed
Any state --Vault Lock--> Disconnect all -> Remove all -> PIN Gate
```

规则：

- 新建 Tab 立即成为 Active。
- 同一 Saved Host 可以打开多个 Tab；Tab Identity 不是 Host ID。
- Live Tab 的 Close 使用明确的“Disconnect and close”语义。
- Closed/Error Tab 可直接关闭。
- Session Event 到达已删除 Tab 时忽略；若仍有 Rust Session ID，则请求
  Disconnect。
- `closed` Event 只结束对应 Tab，不改变其他 Tab。
- 最后一个 Tab 关闭后显示 Empty Terminal Workspace，而不是复用旧 Scrollback。

## Terminal Mount 与 Backpressure

所有未关闭 Tab 的 `TerminalPane` 都保持 Mounted：

```tsx
tabs.map((tab) => (
  <div hidden={tab.tabId !== activeTabId}>
    <TerminalPane ... />
  </div>
))
```

不能只 Mount Active Tab：

- 非活动 Session 的 Binary Data Channel 仍会到达。
- xterm `write` Callback 完成后才发送 `ssh_ack_output`。
- 若 Terminal 被卸载或暂停，Tauri 在 8 个 In-flight Chunk 后会停止读取该
  Session，最终影响远端进程。

每个 Tab 独立记录 Terminal Size。切换到 Active 后执行 Fit；只有对应 Session
Connected 时发送 Resize。隐藏 Tab 的 ResizeObserver 不应把 0x0 发送给 SSH。

## Event、Challenge 与 Dialog Routing

- Connect Callback 闭包固定绑定 `tabId + generation`。
- Event Reducer 只能更新该 Tab。
- Host Key/Changed-Key Dialog 显示 Tab Title、Hop 和 Endpoint。
- 每个 Tab 最多一个 Pending Authentication Request，但多个 Tab 可以同时
  Pending。
- 只有 Active Tab 显示其 Dialog；Tab Strip 必须显示 Pending Badge。切换到另一
  Tab 会卸载当前 Response Form 并清空未提交内容。
- Submit/Cancel 使用该 Tab 当前 `nativeSessionId + requestId`。若二者已变化则
  Fail Closed，不发送 Response。
- 第一个 Pending Action 到达且 Active Tab 没有 Host Key/Challenge/Changed-Key
  Dialog 时自动激活；其他 Pending Tab 只显示文本 Indicator，不持续抢焦点。

## Tauri Session Registry

现有 Registry Map 保留，并补充：

- 可测试的 `remove_and_disconnect` / `disconnect_all` 生命周期 Helper。
- Event/Data Channel 关闭时显式请求 Disconnect，而不是只依赖 Sender Drop。
- Vault Lock 继续先 Drain/Disconnect，再锁 DB Actor。
- 应用/窗口退出时 Drain 全部 Session。
- Registry Unit Test 覆盖多个 Session ID、独立 Ack、单个 Remove 和全量 Drain。

WebView 不需要 `list_sessions` 或可伪造的 Session Restore API。Tab 是当前
Renderer 的临时展示模型；Rust Session ID 只由 Connect Command 返回。

## UI

Desktop：

- Terminal Card 上方增加可横向滚动的 Tab Strip。
- Tab 显示 Title、Status Dot、Pending Badge 和 Close。
- 提供 `+` 新建 Quick Tab；Saved Host 的 Open 操作创建新 Tab。
- Header Status/Disconnect 只反映 Active Tab。

Mobile：

- Session Tab 使用横向滚动 Chip/Compact Strip。
- Terminal 保持全屏优先；Connection Panel 在 Terminal 下方。
- Pending Challenge Dialog 必须在 390x844 内完整可滚动。

可访问性：

- Tab Strip 使用 `role="tablist"`，Tab 使用 `role="tab"` /
  `aria-selected` / `aria-controls`。
- Terminal Panel 使用 `role="tabpanel"`。
- Close Button 有包含 Tab Title 的 Accessible Name。
- Pending Badge 不只依赖颜色。

## Browser Preview

`previewSessions` 已按 Session ID 保存 Callback 和 Command Buffer，可并行运行。
新增测试必须证明：

- 两个 Preview Session 的 Output/Input 不串线。
- 一个 Tab 的 Host Key/OTP Decision 不影响另一个。
- 关闭一个 Preview Session 后另一个继续响应命令。
- Close-during-connect 会 Disconnect Late Preview Session；Native Vault Lock
  负责全量清理真实 Session。

Browser QA 仍不得打开网络或保存 Response。

## 验证

### Unit/Frontend

- Reducer：Create/Activate/Attach Session/Event/Disconnect/Close/Remove。
- Late Connect Return、Stale Event 和 Generation Mismatch。
- 多 Tab Host Key/Challenge Request 路由。
- Quick Password/Response 清理。
- Terminal Ref/Resize 只路由到对应 Tab。

### Protocol/Native

- 两个 OpenSSH Session 同时 Connected。
- 关闭 Tab A 后 Tab B 仍能输入并创建远端 Marker。
- Tab A 非活动时排空大输出，切回后内容完整且可继续执行命令。
- 一个 Tab Pending Keyboard-interactive 时另一个 Tab 正常 Connected。
- Vault Lock 断开全部 Session，重开 Vault 后没有残留 Registry Entry。

### UI/Platform

- Playwright 和 agent-browser Desktop/Compact/Mobile Tab Strip。
- X11、Wayland 和 Windows 真实 Native Tab 生命周期。
- Browser/Native Error Log、Screenshot 和 Secret Scan。
- Android/Linux Container、Workspace 和同 Commit CI。

## 相关文档

- [ADR-0017](../adr/0017-session-tabs-own-independent-runtime-lifecycles.md)
- [Threat Model v1](threat-model-v1.md)
- [Technical Architecture 2026](technical-architecture-2026.md)
- [Keyboard-interactive Authentication v1](keyboard-interactive-authentication-v1.md)
- [ExecPlan 0007](../execplans/active/0007-multi-tab-terminal-and-session-lifecycle.md)
