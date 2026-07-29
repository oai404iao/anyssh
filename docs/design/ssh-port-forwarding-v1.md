# SSH Port Forwarding v1

> 状态：已实现；ADR-0018 已接受
> 日期：2026-07-29

本文定义 Session-scoped Local、Remote 和 Dynamic TCP Forwarding。长期决策见
Accepted ADR-0018。

## 目标

- 在现有 russh Session 上提供 Local、Remote 和 Dynamic Forward。
- Forward Payload 始终留在 Rust Async Stream/SSH Channel。
- Direct 与 Saved Host/Jump Route 使用同一 Forward Runtime。
- Stop、Disconnect、Close、Vault Lock 和 App Exit 有确定 Cleanup。
- 默认只绑定 Loopback，并提供明确的 Active Forward Metadata。

## 非目标

- Forward Profile 持久化、自动启动或应用重启恢复。
- Wildcard/LAN/Public Bind。
- SOCKS4、SOCKS5 Authentication、`BIND` 或 `UDP ASSOCIATE`。
- Unix Domain Socket、X11、Agent Forwarding。
- Headless Background Session、Mobile Foreground Service 或自动重连。
- HTTP Reverse Proxy、TLS Termination、Traffic Inspection 或 Recording。

## 固定依赖能力

固定的 russh `0.62.4` 已提供：

- `Handle::channel_open_direct_tcpip`。
- `Handle::tcpip_forward` 和 `cancel_tcpip_forward`。
- `Handler::server_channel_open_forwarded_tcpip` 与显式 Accept/Reject。
- `Channel::into_stream()`，可接入 Tokio Async I/O。

`Handle` 不可 Clone，因为它拥有 Reply Receiver 和 Join Handle。所有 Channel
Open/Forward Registration 必须由 Target Session Task 编排，不能把 Handle 复制
到每个 Listener Task。

## Runtime Model

```text
SessionControl
  -> bounded TerminalCommand + ForwardCommand
    -> Target Session Task owns russh Handle
       |- PTY Channel
       |- Forward Registry (max 16)
       |- Local/Dynamic Accept Queue
       |- Remote forwarded-tcpip Queue
       `- Connection Tasks (max 64 per Forward)
```

建议的公共值对象：

```rust
pub enum ForwardKind {
    Local,
    Remote,
    Dynamic,
}

pub struct ForwardRequest {
    pub kind: ForwardKind,
    pub bind_host: String,
    pub bind_port: u16,
    pub destination_host: Option<String>,
    pub destination_port: Option<u16>,
}

pub struct ForwardSummary {
    pub id: String,
    pub kind: ForwardKind,
    pub bind_host: String,
    pub bound_port: u16,
    pub destination_host: Option<String>,
    pub destination_port: Option<u16>,
}
```

Forward ID 由 Rust 生成并只在当前 Session 内有效。Start/Stop 使用 oneshot
Response。v1 不发送逐 Connection Event；Connection Failure 只关闭该 Stream，
Session Closed 清空全部 Active Metadata，不把 SOCKS Destination 或 Payload
放入 Event。

## Session Command 与 Cancellation

Runtime 使用两条独立有界 Queue：

- `TerminalCommand::{Input, Resize}`。
- `ForwardCommand::{Start, Stop}`，均带 oneshot Response。

每个 Forward Entry 持有：

- Cancellation Token。
- Listener/Remote Registration Metadata。
- 当前 Connection Semaphore。
- 必要的 Accept Sender。

Session Cancellation 先阻止新 Connection，再取消 Listener/Remote Registration，
最后等待或中止 Connection Task。Stop 对合法但已不存在的 Forward ID 幂等成功。
`cancel_tcpip_forward` 失败时本地 Connection 立即取消，但保留一个已取消 Entry
供重试和 Session Cleanup；后到 Channel 一律拒绝。

## Local Forward

1. Rust 在 Loopback `bind_host:bind_port` 创建 `TcpListener`；Port 0 返回实际 Port。
2. Listener Task 接受 TCP Stream，获取 Originator Address，并把有界 Open Request
   发送给 Target Session Task。
3. Session Task 调用 `channel_open_direct_tcpip(destination, originator)`。
4. 成功后把 `Channel::into_stream()` 与 Local TCP Stream 交给 Copy Task。
5. Channel Open 失败、Session 取消、Connection 超限或 Queue 满时关闭 Local
   Stream，不重试到其他 Destination。

Destination Hostname 原样发送给 SSH Server，由 Target 侧解析。

## Dynamic Forward

Dynamic Listener 与 Local Forward 共用 Loopback Listener 和 Connection Limit。
每个连接先在 Rust 内解析有界 SOCKS5 Handshake：

1. 只接受 Version 5。
2. 只选择 `NO AUTHENTICATION REQUIRED`；没有该 Method 时拒绝。
3. 只接受 `CONNECT`。
4. 解析 IPv4、IPv6 或最长 255 Byte Domain。
5. 将目标交给 `channel_open_direct_tcpip`。
6. SSH Channel 成功后返回 SOCKS Success，再开始双向 Copy。

在 Channel 成功前不得返回 Success。错误输入使用最小必要 Reply，不记录目标或
Payload。

## Remote Forward

1. Session Task 验证 Server Bind 是 Loopback，调用
   `tcpip_forward(bind_host, bind_port)`。
2. Port 0 时保存 Server 返回的实际 Port。
3. `ClientHandler::server_channel_open_forwarded_tcpip` 把 Channel、Connected/
   Originator Metadata 和 Reply Handle 发送到有界 Queue；Queue 满时显式
   `ResourceShortage` Reject，Queue Closed/非 Target Handler 显式
   `AdministrativelyProhibited` Reject。
4. Session Task 查找匹配 Active Registration，并尝试连接本地 Destination。
5. Local Connect 成功后 Accept Remote Channel，并启动 Copy；失败、超时、已停止
   或超限时 Reject。
6. Stop 先逻辑取消 Entry，再调用 `cancel_tcpip_forward`；成功后移除。失败时保留
   已取消 Cleanup Entry，后到 Channel 一律拒绝。

## Backpressure 与资源上限

- 每 Session 最多 16 个 Active Forward。
- 每 Forward 最多 64 个并发 Connection。
- Listener Accept/Remote Channel Queue 必须有界。
- SOCKS Handshake、SSH Channel Open 和 Local Destination Connect 使用 Timeout。
- 使用 `copy_bidirectional`/Async Stream Backpressure，不建立全量 Payload
  Buffer。
- 不逐连接输出字节数或目标；v1 只通过 Start/Stop Response 和 Session Lifecycle
  更新 UI。

## Tauri 与 Frontend

Typed IPC：

- `ssh_forward_start(sessionId, request) -> ForwardSummary`
- `ssh_forward_stop(sessionId, forwardId)`

WebView 可提交 Endpoint Metadata，但不能提交 Socket Handle、SSH Channel、
Payload、Proxy Credential 或任意 Shell Command。

Active Tab 的 Connection Panel 增加 Forward 区域：

- Kind。
- Loopback Bind Port（0 表示自动分配）。
- Local/Remote Destination。
- Start/Stop。
- Active Forward 列表和实际 Port。

切换 Tab 保留 Forward；Close/Disconnect 后列表变为 Stopped。Browser QA 只模拟
Metadata 和 Lifecycle，不打开 Listener。

## 验证

### Unit

- Loopback 地址规范化和非 Loopback 拒绝。
- Request Shape、Port 0 和 Kind/Destination Constraint。
- SOCKS5 IPv4/IPv6/Domain、Fragmented Read 和全部错误分支。
- Forward Limit、Connection Limit、Queue Full、Timeout 和 Cancellation。
- Remote Registration Match、Late Channel Reject 和 Stop Idempotency。

### Protocol

- OpenSSH Local Forward 到 Target-only Echo/HTTP Fixture。
- Password Jump -> Target Local Forward。
- Dynamic SOCKS5 访问 Target-only Fixture。
- OpenSSH Remote Forward 到本地 Fixture，Stop 后 Port 不可达。
- 大流量双向 Copy、Half-close 和 Session Disconnect。

### Product/Platform

- Playwright/agent-browser Start/Stop Metadata。
- X11、Wayland、Windows 真实 Listener/Remote Marker。
- Vault Lock/Tab Close 全量 Cleanup。
- Error Log、Screenshot、Payload Marker 和 Secret Scan。
- Linux/Android Container 与同 Commit CI。

## 相关文档

- [ADR-0018](../adr/0018-port-forwarding-is-rust-owned-and-session-scoped.md)
- [Multi Tab Session Lifecycle v1](multi-tab-session-lifecycle-v1.md)
- [Technical Architecture 2026](technical-architecture-2026.md)
- [Threat Model v1](threat-model-v1.md)
- [ExecPlan 0008](../execplans/completed/0008-ssh-port-forwarding.md)
