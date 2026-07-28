# ExecPlan 0008：SSH Port Forwarding

- 状态：Active
- 创建日期：2026-07-28
- 最后更新：2026-07-28
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

让用户在当前 AnySSH Session 内启动和停止 Local、Remote、Dynamic Forward，
访问远端内网服务、把本地服务安全映射到 SSH Server Loopback，或通过 SOCKS5
按需访问 Target 网络，同时不调用系统 `ssh`、不让 TCP Payload 进入 WebView，
并在 Tab Close/Vault Lock/App Exit 时可靠清理。

## 范围

### 包含

- Proposed ADR-0018 与 SSH Port Forwarding v1 Design。
- Session-scoped Local、Remote 和 Dynamic Forward。
- Loopback-only Bind、Port 0、16 Forward/64 Connection 上限。
- SOCKS5 `CONNECT` IPv4/IPv6/Domain。
- Direct、Saved Host 和 Jump Route Target Session。
- Tauri Metadata-only Start/Stop IPC。
- React Active Forward UI 与每 Tab Lifecycle。
- OpenSSH Protocol、Browser、X11、Wayland、Windows、Container 和同 Commit CI。

### 不包含

- Forward Profile Repository、Schema Migration、自动启动或重启恢复。
- Wildcard/LAN/Public Bind。
- SOCKS Authentication、SOCKS4、`BIND`、`UDP ASSOCIATE`。
- Unix Socket、X11 或 Agent Forwarding。
- Headless/Mobile Background Session 和自动重连。
- SFTP、WebDAV、Key Export、Theme 或 Snippet。

## 上下文

当前状态：

- Multi Tab 已完成；每 Tab 对应独立 Rust Session 和 Lifecycle。
- `SessionControl` 通过有界 `SessionCommand` 发送 Input/Resize，Disconnect 使用
  Cancellation。
- Target Session Task 独占 russh `Handle<ClientHandler>` 和 PTY Channel。
- russh Handle 不可 Clone，但已提供 `channel_open_direct_tcpip`、
  `tcpip_forward`、`cancel_tcpip_forward` 和
  `server_channel_open_forwarded_tcpip`。
- Jump Route 最终返回 Target russh Handle，因此 Forward 可复用同一 Target
  网络视角。
- TCP Payload 当前没有进入 Tauri/WebView 的路径，必须保持该边界。

关键路径：

- `crates/anyssh-ssh/src/lib.rs`
- `crates/anyssh-ssh/tests/`
- `crates/anyssh-app/src/lib.rs`
- `apps/client/src-tauri/src/lib.rs`
- `apps/client/src/lib/ssh-bridge.ts`
- `apps/client/src/App.tsx`
- `apps/client/src/App.css`
- `apps/client/e2e/connect-preview.spec.ts`
- `scripts/test-ssh-smoke.sh`
- `scripts/qa/native-xvfb-smoke.sh`
- `scripts/qa/native-wayland-ime-smoke.sh`
- `scripts/qa/native-windows-smoke.ps1`
- `tests/fixtures/openssh/`

## Progress

- [x] 2026-07-28：完成 ExecPlan 0007，同 Commit Run `30368134792` 九个 Job
  通过并接受 ADR-0017。
- [x] 2026-07-28：核验固定 russh `0.62.4` 的 Direct/Remote Forward API 和
  不可 Clone Handle 约束。
- [x] 2026-07-28：创建 Proposed ADR-0018、Design 和本 ExecPlan。
- [ ] 完成 Milestone 1：Forward Runtime/Core Control。
- [ ] 完成 Milestone 2：Local 与 Dynamic Forward。
- [ ] 完成 Milestone 3：Remote Forward。
- [ ] 完成 Milestone 4：Tauri/React Product UI。
- [ ] 完成 Milestone 5：Native QA、全量回归与治理。

## Milestones

### Milestone 1：Forward Runtime/Core Control

1. 定义 Forward Request/Summary/Event/Error 和 Loopback Validation。
2. 扩展 SessionCommand Start/Stop + oneshot Response。
3. Forward Registry、Cancellation、Semaphore 和有界 Queue。
4. Session Disconnect/Drop 清理全部 Forward。
5. Unit Test 覆盖 Limit、Late Command、Stop Idempotency 和 Debug Redaction。

出口：

- Forward ID/State 只属于当前 Session。
- Payload 和 Socket/Channel Handle 不进入 Debug/Event。

### Milestone 2：Local 与 Dynamic Forward

1. Loopback TcpListener 与 Port 0。
2. Local Accept -> `direct-tcpip` -> 双向 Copy。
3. SOCKS5 Fragmented Parser、IPv4/IPv6/Domain 和 `CONNECT`。
4. Queue Full、Connection Limit、Timeout、Half-close 和 Cancellation。
5. Direct/Jump OpenSSH Fixture。

出口：

- Local/Dynamic 可访问 Target-only Fixture。
- 错误 SOCKS Request Fail Closed。

### Milestone 3：Remote Forward

1. ClientHandler Forwarded Channel Queue。
2. `tcpip_forward`/Assigned Port/Registration Match。
3. Local Destination Connect 后 Accept，否则 Reject。
4. Stop/Cancel/Late Channel 和 Session Close。
5. OpenSSH Remote Forward Protocol Smoke。

出口：

- Server Loopback Port 可到达本地 Fixture，Stop 后立即不可达。

### Milestone 4：Tauri/React Product UI

1. Metadata-only Start/Stop IPC。
2. Per-tab Active Forward State/Event Routing。
3. Local/Remote/Dynamic Form、Validation、Actual Port 和 Stop。
4. Disconnect/Close/Vault Lock 清理 UI State。
5. Playwright、agent-browser Desktop/Mobile。

出口：

- WebView 不接触 Payload。
- 一个 Tab 的 Forward 不出现在另一个 Tab。

### Milestone 5：Native QA 与治理

1. X11、Wayland、Windows 真实 Forward Marker。
2. Tab Close/Vault Lock/Channel Loss Cleanup。
3. Workspace/OpenSSH/Container/Android 全量回归。
4. 同 Commit CI 与 Artifact/Payload/Secret Scan。
5. 更新 Threat Model、Status、Roadmap、README、AGENTS。
6. 接受、拒绝或替代 ADR-0018 并移动本计划。

出口：

- Local/Remote/Dynamic Forward 的 Runtime、UI、Cleanup 和治理一致。

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

- Payload 不进入 IPC/React/Log/Evidence。
- Loopback-only Bind 和 Port 0。
- Local/Dynamic Target-side DNS。
- Remote Registration/Assigned Port/Late Channel。
- 16 Forward/64 Connection/Queue/Timeout。
- Direct/Saved/Jump。
- Stop/Disconnect/Close/Vault Lock/App Exit Cleanup。

## Surprises & Discoveries

- 2026-07-28：russh `Handle<ClientHandler>` 不可 Clone；它包含 Reply Receiver 和
  Join Handle。Forward Channel Open 不能简单分发到 Listener Task，必须由 Target
  Session Task 串行编排。
- 2026-07-28：russh Remote Forward Handler 提供显式
  `ChannelOpenHandle::accept/reject`。AnySSH 可以先验证 Registration 和本地
  Destination，再接受 Server Channel。

## Decision Log

- 2026-07-28：v1 Forward 绑定 Live Session，不新增持久化 Repository 或
  Headless Auto-reconnect。
- 2026-07-28：v1 Local/Dynamic/Remote Bind 只允许 Loopback。
- 2026-07-28：Dynamic v1 只支持无认证 SOCKS5 `CONNECT`。
- 2026-07-28：Forward Payload 保持 Rust-only；Tauri 只传 Metadata 和控制命令。

## Outcomes & Retrospective

尚未完成。
