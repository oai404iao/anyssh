# ExecPlan 0008：SSH Port Forwarding

- 状态：Completed
- 创建日期：2026-07-28
- 最后更新：2026-07-29
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
- [x] 2026-07-29：完成 Milestone 1。`SessionControl` 使用独立有界
  `TerminalCommand`/`ForwardCommand`，实现 Session-owned Registry、16/64
  上限、Cancellation、幂等 Stop 和全量 Cleanup。
- [x] 2026-07-29：完成 Milestone 2。Local 与 Dynamic SOCKS5 支持 Loopback、
  Port 0、IPv4/IPv6/Domain、`CONNECT`、有界 Handshake、Half-close 和
  Target-side DNS。
- [x] 2026-07-29：完成 Milestone 3。Remote 使用
  `tcpip_forward`/`cancel_tcpip_forward`、Assigned Port、显式 Channel
  Accept/Reject、Registration Match 和本地 Destination Connect。
- [x] 2026-07-29：完成 Milestone 4。Tauri IPC 只传 Metadata；React 提供每 Tab
  Local/Remote/Dynamic Form、Actual Port、Active List 和 Stop；Browser Preview
  不打开 Listener。
- [x] 2026-07-29：真实 OpenSSH Protocol Smoke 已覆盖 Direct/Jump Local、
  Dynamic、Remote、4 MiB Copy、Half-close、16 Forward、64 Connection、
  Stop/Disconnect Cleanup 和错误 SOCKS Command。
- [x] 2026-07-29：Frontend 17 个 Vitest 与 11 个 Playwright 通过；Browser
  agent QA `smoke-1785290802` 通过并人工检查 Desktop/Mobile Forward 截图。
- [x] 2026-07-29：Linux X11 Native QA `smoke-1785290843-2194048` 与 Wayland
  Native QA `smoke-1785289772-2158531` 通过真实 Local/Dynamic/Remote Marker、
  Tab Close/Disconnect/Vault Lock Cleanup 和 Payload Evidence Scan。
- [x] 2026-07-29：独立 Linux Container Build `build-1785290669-1` 与 Android
  ARM64 Container Build `build-1785290750-1` 通过。
- [x] 2026-07-29：Head
  `6fcb1a68d5d791d164f3ed43209aa3a9613b5acf` 的 GitHub Actions Run
  `30416305300` 九个 Job 全部通过。
- [x] 2026-07-29：人工检查远端 Browser
  `smoke-1785291333`、X11 `smoke-1785291361-5978`、Wayland
  `smoke-1785291518-8738` 和 Windows
  `smoke-20260729-021700-9484` 的 Forward、Multi Tab、Vault Lock 截图、空
  Browser Error Log 和 Payload/Secret Scan。
- [x] 2026-07-29：远端 Linux ELF
  `9f2d409feb9c32d3a415886e63f5af4b0cb7e04717e4fc3cd7665e8a5111a0dd`、
  Android APK
  `794c988bc07d8fce907d2f434852d26ac9f5b852b17b5a5473f8e058ea34b989`
  和 Windows EXE
  `a6c3d7e3beccf77cdc3a895dac8133e211e9549084d23ee035db540abb906d1b`
  已记录。
- [x] 2026-07-29：完成 Milestone 5，接受 ADR-0018 并将本计划移动到
  `completed/`。

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
- 2026-07-29：PTY 与 Forward 共用一个不可 Clone russh Handle，但把
  Terminal Loop 与 Forward Actor 拆为两个 Task 后，Terminal 输入/输出不会被
  Listener Accept 阻塞；只有 Channel Open/Registration 在 Forward Actor 内串行。
- 2026-07-29：OpenSSH Remote Port 0 会返回 Assigned Port；非零请求成功时 russh
  按协议返回 0，因此 Summary 必须在这两种情况间显式选择。
- 2026-07-29：原生 1280px WebKitGTK 窗口中的 Connection Panel 需要滚动后才能
  查看全部 Forward；X11 Driver 增加 Mouse Wheel 命令并以真实输入验证。

## Decision Log

- 2026-07-28：v1 Forward 绑定 Live Session，不新增持久化 Repository 或
  Headless Auto-reconnect。
- 2026-07-28：v1 Local/Dynamic/Remote Bind 只允许 Loopback。
- 2026-07-28：Dynamic v1 只支持无认证 SOCKS5 `CONNECT`。
- 2026-07-28：Forward Payload 保持 Rust-only；Tauri 只传 Metadata 和控制命令。
- 2026-07-29：Stop 对合法、已不存在的 Forward ID 幂等成功；这使重复 Close、
  Disconnect 和 Stale UI Cleanup 不需要区分竞态先后。
- 2026-07-29：v1 不发送逐 Connection Forward Event。Start/Stop Error 通过
  oneshot 返回，Session Closed 清空全部 UI Metadata；这样 SOCKS Destination
  和 Payload 不会因诊断事件越过 Rust 边界。
- 2026-07-29：Remote Cancel 失败时立即取消本地 Connection，但保留一个已取消
  Registration Entry 供重试/Session Cleanup，再到 Channel 一律拒绝。

## Outcomes & Retrospective

完成。

- russh Target Session 现在拥有 Local、Remote 和 Dynamic Forward Registry。
  Terminal 与 Forward Command 使用独立有界 Queue；Forward ID、Listener、
  Remote Registration、Handshake 和 Connection Task 均绑定当前 Session。
- Local/Dynamic 通过 Target `direct-tcpip` 实现 Target-side DNS；Dynamic 只
  接受无认证 SOCKS5 `CONNECT`。Remote 使用显式 Registration Match、
  `forwarded-tcpip` Accept/Reject 和本地 Destination Connect。
- v1 强制 Loopback-only Bind、16 Forward/64 Connection、Port 0、Timeout、
  Backpressure、Cancellation 和幂等 Stop；Disconnect、Tab Close、Channel
  Loss、Vault Lock 与 App Exit 会清理全部 Forward。
- Tauri/React 只处理 Forward Metadata 和 Start/Stop。Browser Preview 不打开
  Listener；真实 Payload、SOCKS Destination、Socket 和 SSH Channel 不进入
  WebView、Tauri Event、日志或 Evidence。
- OpenSSH Direct/Jump Smoke、Browser、X11、Wayland、Windows、Linux Container、
  Android ARM64 和同 Commit CI 全部通过。Windows 真实 EXE 证明三类 Forward、
  Dynamic Stop、Session Disconnect、Interactive Tab Close 和 Vault Lock
  Cleanup。
- 实现没有新增 Schema 或持久化 Forward Profile；应用重启和 Session 断开后仍需
  用户显式重建 Forward。Wildcard/LAN/Public Bind、SOCKS Authentication 和
  Background Auto-reconnect 继续留在后续独立设计。
