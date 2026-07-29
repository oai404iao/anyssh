# ADR-0018：SSH Port Forwarding 由 Rust 拥有并绑定 Session

- 状态：Accepted
- 日期：2026-07-28
- 决策人：项目维护者

## 背景

AnySSH 已有独立 Session Tab、任意长度 Jump Route 和 russh Target Session。
Local、Remote 和 Dynamic Forwarding 会处理任意 TCP Byte Stream，并可访问用户
本机、SSH Server 所在网络或 SOCKS Client 指定的目标。若把 Forward Payload
经 WebView/Tauri JSON 中转、调用系统 `ssh -L/-R/-D`，或让 Forward 脱离 Session
生命周期，会扩大 Secret/流量暴露、绕过现有 Host Key/Credential/Jump Route
边界，并可能在 Tab 关闭或 Vault Lock 后留下后台 Listener。

## 决策

- Local、Remote 和 Dynamic Forwarding 全部在 Rust SSH Core/Tauri Runtime 内
  实现，不调用系统 `ssh`，不把 TCP Payload 发送到 WebView。
- v1 Forward Instance 绑定一个 Live Rust Session ID：
  - 复用该 Session 已完成的 Host Key、Authentication 和 Jump Route。
  - Session Disconnect、Tab Close、Channel Loss、Vault Lock 或 App Exit 会取消
    Listener、Remote Registration 和全部 Connection Task。
  - v1 不持久化 Forward/Listener，也不在重启后自动恢复。
- Tauri IPC 只接受和返回受限元数据：
  - Forward Kind。
  - Bind Host/Port。
  - Destination Host/Port（Dynamic 不需要固定 Destination）。
  - Rust-issued Forward ID 和实际分配的 Port。
  TCP Payload、SOCKS Request 内容和应用数据不得进入 IPC、日志或遥测。
- Local 和 Dynamic v1 只允许绑定 Loopback；Remote v1 只请求 Server
  Loopback Bind。Wildcard、LAN/Public Bind 留到带额外确认和平台策略的后续版本。
- Local Forward 使用 Target russh Session 的 `direct-tcpip` Channel。Destination
  Hostname 由 SSH Server 一侧解析；Jump Route 不改变该语义。
- Remote Forward 使用 `tcpip_forward`/`cancel_tcpip_forward`。只有当前 Active
  Registration 的 `forwarded-tcpip` Channel 才可接受；本地 Destination 连接
  失败、超时、超限或 Registration 已取消时拒绝 Channel。
- Dynamic Forward 实现 SOCKS5 `CONNECT`：
  - 支持 IPv4、IPv6 和 Domain Name。
  - v1 不提供 SOCKS Authentication。
  - `BIND`、`UDP ASSOCIATE`、非 SOCKS5 和超长 Request Fail Closed。
- 每 Session 最多 16 个 Active Forward，每 Forward 最多 64 个并发 Connection；
  Listener/Connect/SOCKS Handshake 使用有界 Timeout 和 Cancellation。
- Forward Copy 使用 Rust Async Stream 与 SSH Channel 背压；不构造无界 Buffer，
  不逐 Chunk 生成 Tauri Event。
- Frontend 只管理 Active Tab 的 Forward Metadata 和 Start/Stop Action。切换 Tab
  不停止 Forward；关闭拥有它的 Tab 会停止。

## 备选方案

- 调用系统 `ssh -L/-R/-D`：绕过 russh、Credential、Known Host、Jump Route 和
  Session Registry，拒绝。
- 在 WebView 使用 WebSocket/Tauri Event 转发 TCP Payload：扩大攻击面并破坏
  Binary Backpressure，拒绝。
- Forward 独立于 SSH Session 自动重连：会引入 Headless Session、Credential
  生命周期和后台策略，v1 拒绝。
- 默认允许 `0.0.0.0`/`::` Bind：可能无意暴露本机或远端网络服务，v1 拒绝。
- v1 同时加入持久化 Forward Repository：需要新的 Schema/同步语义，留到运行时
  模型验证后。

## 后果

### 正面

- Forward Traffic 保持在 Rust/SSH Channel 内，不经过 WebView。
- Forward 自动继承现有 Endpoint Trust、Credential 和 Jump Route 安全边界。
- Session Tab、Vault Lock 和 App Exit 提供统一、可验证的 Cleanup。
- Loopback-only Default 显著降低误暴露风险。

### 代价与风险

- `SessionCommand`、Target Session Loop 和 Client Handler 需要同时处理 PTY、
  Listener Accept、Remote Channel 和 Forward Cancellation。
- russh `Handle` 不可 Clone；Channel Open 必须由拥有 Handle 的 Session Task
  串行编排。
- Dynamic SOCKS5 Parser 和 Remote Forward Channel 必须有独立恶意输入测试。
- v1 Forward 不持久化，应用重启或 Session 断开后需手动重建。

## 验证

- Local Forward 通过真实 OpenSSH Target 访问远端-only TCP Fixture。
- Jump Route 上的 Local Forward 仍由最终 Target 网络解析 Destination。
- Dynamic SOCKS5 IPv4/IPv6/Domain `CONNECT` 成功；错误版本、Command、Address
  和超限 Request 被拒绝。
- Remote Forward 接受 Server 连接并到达本地 Fixture；Stop 后 Server Port
  关闭，取消/超时/本地连接失败 Fail Closed。
- 16 Forward/64 Connection、Loopback Bind 和 Cancellation 上限。
- Forward Payload 不进入 Tauri IPC、React State、Log 或 Evidence。
- 单 Tab Close、Vault Lock、Channel Loss 和 App Exit 清除全部 Forward。
- Browser Metadata QA、OpenSSH Protocol、X11、Wayland、Windows、Container 和
  同 Commit CI。

## 相关文档

- Design：[SSH Port Forwarding v1](../design/ssh-port-forwarding-v1.md)
- ExecPlan：[SSH Port Forwarding](../execplans/completed/0008-ssh-port-forwarding.md)
- ADR：[ADR-0002](0002-russh-as-default-ssh-engine.md)
- ADR：[ADR-0006](0006-secrets-stay-out-of-webview.md)
- ADR：[ADR-0010](0010-saved-host-plans-resolve-in-rust.md)
- ADR：[ADR-0017](0017-session-tabs-own-independent-runtime-lifecycles.md)
- Supersedes：
- Superseded by：
