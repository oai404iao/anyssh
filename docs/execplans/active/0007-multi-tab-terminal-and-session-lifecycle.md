# ExecPlan 0007：Multi Tab Terminal and Session Lifecycle

- 状态：Active
- 创建日期：2026-07-28
- 最后更新：2026-07-28
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

让用户同时保持多个独立 SSH Terminal Session，并能安全切换、断开和关闭，而不
混淆 Output、Input、Host Key、OTP Challenge 或错误。Vault Lock 和应用退出必须
可靠终止全部 Session，不能留下后台连接或未清空的临时秘密。

## 范围

### 包含

- Proposed ADR-0017 与 Multi Tab Session Lifecycle v1 Design。
- 最多 8 个短生命周期 Session Tab。
- Quick Connection 和 Saved Host 新建 Tab。
- 每 Tab xterm.js、Status、Terminal Size、Host Key 和 Authentication State。
- Disconnect 保留 Scrollback，Close 移除 Tab。
- Vault Lock、Window/App Exit、Channel Loss 和 Late Connect Return。
- Tauri Session Registry Lifecycle/Ack 测试。
- Browser Preview 并发 Session。
- Playwright、agent-browser、X11、Wayland、Windows 和同 Commit CI。

### 不包含

- Tab/Scrollback 持久化、重启恢复或自动重连。
- Split Pane、Broadcast Input、Session Group 或 Terminal Recording。
- SSH Connection Multiplexing。
- Android Foreground Service 或 Mobile Background Session。
- Forwarding、SFTP、WebDAV、Theme、Snippet 或 Key Export。

## 上下文

当前状态：

- Tauri `SessionRegistry` 已按 Session ID 保存多个 `SessionEntry`。
- 每个 Entry 已有独立 `SessionControl` 和 Output Ack Sender。
- 每次 Connect 已创建独立 Event/Data Channel Pump。
- React 仍只有一个 `sessionId`、一个 `TerminalPane` 和一组全局
  Host Key/Authentication/Status State。
- `TerminalPane` 卸载会 Dispose xterm.js；非活动 Tab 若卸载会丢 Scrollback，并
  可能因没有 xterm Write Ack 在八 Chunk Window 上停住。
- Rust `vault_lock` 已 Drain 并 Disconnect Registry，但前端只清理单 Session。

关键路径：

- `apps/client/src/App.tsx`
- `apps/client/src/App.css`
- `apps/client/src/components/TerminalPane.tsx`
- `apps/client/src/lib/ssh-bridge.ts`
- `apps/client/src-tauri/src/lib.rs`
- `apps/client/e2e/connect-preview.spec.ts`
- `apps/client/e2e/windows-native-smoke.mjs`
- `scripts/qa/agent-browser-smoke.sh`
- `scripts/qa/native-xvfb-smoke.sh`
- `scripts/qa/native-wayland-ime-smoke.sh`
- `scripts/qa/native-windows-smoke.ps1`

## Progress

- [x] 2026-07-28：完成 ExecPlan 0006，接受 ADR-0016。
- [x] 2026-07-28：确认 Tauri Registry 已支持多个独立 Session Entry，主要缺口在
  React 单 Session Model 和 Lifecycle Race。
- [x] 2026-07-28：创建 Proposed ADR-0017、Design 和本 ExecPlan。
- [ ] 完成 Milestone 1：Per-tab Frontend Session Model。
- [ ] 完成 Milestone 2：Terminal Mount、Routing 与 Lifecycle。
- [ ] 完成 Milestone 3：Desktop/Mobile Product UI。
- [ ] 完成 Milestone 4：Native Multi-session QA。
- [ ] 完成 Milestone 5：全量回归、Artifact 检查、ADR 评审和收尾。

## Milestones

### Milestone 1：Per-tab Frontend Session Model

1. 引入 `SessionTab`、Tab ID、Generation 和 Reducer。
2. Quick/Saved Host Connect 创建独立 Tab。
3. Event/Data Callback 绑定 Tab ID/Generation。
4. Status、Error、Host Key、Changed-Key 和 Challenge 按 Tab 隔离。
5. Late Connect Return 和 Stale Event Fail Closed。

出口：

- 两个 Browser Preview Session 的状态和输出不串线。
- 临时 Password/Response 不进入共享 Global State。

### Milestone 2：Terminal Mount、Routing 与 Lifecycle

1. 每 Tab 独立 `TerminalPane`/Ref/Size。
2. 非活动 Terminal 保持 Mounted 并继续 Ack Output。
3. Disconnect 保留 Scrollback；Close Disconnect 后 Remove。
4. Vault Lock/Channel Loss/App Exit 断开全部。
5. Tauri Registry 多 Session、Ack、Remove/Drain Unit Test。

出口：

- 非活动 Tab 能排空大输出。
- 关闭一个 Session 不影响其他 Session。
- 无后台孤儿 Registry Entry。

### Milestone 3：Desktop/Mobile Product UI

1. Desktop 可横向滚动 Tab Strip 和 New Tab。
2. Mobile Compact Session Strip。
3. Status/Pending Badge/Close/Disconnect。
4. `tablist`/`tab`/`tabpanel` ARIA 与 Focus。
5. 多 Tab Host Key/Keyboard-interactive Dialog Routing。

出口：

- Desktop/Compact/Mobile 不截断、不遮挡 Terminal 或 Dialog。
- Pending Challenge 归属清晰且不能提交到错误 Tab。

### Milestone 4：Native Multi-session QA

1. X11 同时连接两个 OpenSSH Session。
2. 非活动 Tab 大输出与后续命令。
3. 一个 Tab Challenge、另一个 Tab Connected。
4. Wayland/IBus 回归。
5. Windows 真实 EXE/WebView2 多 Session 与单 Tab Close。
6. Vault Lock 全量断开和 Secret Scan。

出口：

- Linux/Windows Runtime 证明 Session Independence 和 Fail-closed Cleanup。

### Milestone 5：全量回归与治理

1. Workspace、Frontend、Browser、OpenSSH、Native、Container。
2. 同 Commit CI 九个 Job。
3. 人工检查 Screenshot、Error Log、Build Hash 和 Secret Scan。
4. 更新 Threat Model、Status、Roadmap、README 和 AGENTS。
5. 接受、拒绝或替代 ADR-0017。
6. 移动本计划到 `completed/`。

出口：

- Multi Tab Runtime、UI、Backpressure 和治理文档一致。

## Validation

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm test:ssh:smoke
pnpm test:e2e
pnpm qa:browser
pnpm qa:native:xvfb
pnpm qa:native:wayland
pnpm qa:native:windows
pnpm check:container:linux
pnpm check:container:android
pnpm docs:check
pnpm format:check
git diff --check
```

验收重点：

- Tab/Session ID/Generation 绑定。
- Output/Input/Resize/Host Key/OTP/Error 隔离。
- Inactive Terminal 持续 Ack。
- Close-during-connect 和 Stale Event。
- Disconnect vs Close。
- Vault Lock/App Exit 全量清理。
- Browser 与 Native 多 Session。
- Desktop/Mobile 可访问性和截图。

## Surprises & Discoveries

- 2026-07-28：Rust/Tauri 并不是单 Session Registry；当前
  `HashMap<String, SessionEntry>` 已具备多 Session 基础，主要重构集中在 React
  State、Terminal Mount 和 Lifecycle。
- 2026-07-28：当前 Output Ack 在 xterm `write` Callback 后发送。若非活动 Tab
  卸载 Terminal，Tauri 会在 8 个 In-flight Chunk 后停止读取该 Session，因此
  “只 Mount Active Tab”不是可接受实现。
- 2026-07-28：`vault_lock` 已先 Drain/Disconnect 全部 Rust Session；前端多 Tab
  必须同步清除所有 Terminal/Challenge State，不能只清 Active Tab。

## Decision Log

- 2026-07-28：v1 使用最多 8 个独立 Session Tab，不做 SSH Transport
  Multiplexing。
- 2026-07-28：Disconnect 保留 Tab/Scrollback，Close 才删除 Tab。
- 2026-07-28：Tab/Scrollback 不持久化；Vault Lock 清除全部。
- 2026-07-28：非活动 xterm.js 保持 Mounted 并继续消费/Ack Output。
- 2026-07-28：Frontend Tab ID 与 Rust Session ID 分离；异步回调还必须校验
  Generation。

## Outcomes & Retrospective

尚未完成。
