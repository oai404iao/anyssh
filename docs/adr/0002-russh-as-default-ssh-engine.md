# ADR-0002：使用 russh 作为默认 SSH Engine

- 状态：Proposed
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

SSH Engine 需要跨桌面和移动平台，支持现代算法、密码、公钥、Agent、Jump Host 和端口转发，同时避免 LGPL/GPL 对移动分发造成额外风险。

## 决策

默认采用 russh，并在 `anyssh-ssh` 内建立隔离层：

- 上层不得直接依赖 russh 具体类型。
- Jump Host 使用 `direct-tcpip` Channel Stream。
- 系统 Agent 作为 External Signer。
- 连接、认证、重连、转发和背压由 AnySSH 状态机封装。

## 备选方案

- libssh：功能成熟，但 LGPL 移动分发需要额外合规评估。
- libssh2：许可证宽松，但现代 KEX 与高级能力需要更多补充。
- 系统 OpenSSH 子进程：不能覆盖 iOS，且跨平台行为不一致。

## 后果

### 正面

- Rust/Tokio 原生集成。
- Apache-2.0。
- 支持现代 OpenSSH 算法和 SSH Channel 能力。

### 代价与风险

- AnySSH 需要自行实现高层连接管理。
- 必须建立 OpenSSH、Dropbear 和旧设备兼容矩阵。
- 核心封装上线前需要安全审计。

## 验证

- 密码、私钥、keyboard-interactive、Agent 登录。
- Host Key 校验和变化阻断。
- 两跳 Jump。
- Local/Remote/Dynamic Forward。
- 大输出背压和取消。

截至 2026-07-27：

- 密码认证、PTY、Resize、Disconnect 和二进制输出已通过 OpenSSH Fixture。
- 未加密和口令保护 Ed25519 OpenSSH 私钥已通过真实认证；错误口令与未授权
  Private Key 均被拒绝。
- 加密 Private Key 已写入 Vault，并在 Lock/Unlock 后只通过 Credential ID
  进入 `anyssh-app`，由 Rust 直接构造 russh Authentication 后成功登录。
- 已保存 SHA-256 Fingerprint 匹配时无需重复提示；Fixture 轮换 Host Key 后
  连接被硬阻断且不允许再次 TOFU。
- 4 MiB 连续输出使 64 项 Core Queue 达到容量上限；原生 Tauri 另限制最多
  8 个未确认 Chunk，并由 xterm `write` Callback Ack 后继续读取，输出完成后仍能
  执行后续远端命令。
- `Client -> Jump Host -> Internal Target` 已通过隔离 Docker 网络验证。
- Jump Host 使用 `direct-tcpip` Channel 的 `into_stream()` 接入下一层
  `client::connect_stream()`，未启动系统 `ssh` 子进程。
- Jump Host 与 Target 使用独立 Host Key 请求 ID、Endpoint 和确认步骤。
- 已覆盖密码 Jump Host + 私钥 Target、握手取消、Target 认证失败、
  Target 握手超时和第一跳丢失。
- 2026-07-27：Saved Host ID 已通过 Rust-only Connection Plan 展开为
  `Jump 1 -> Jump 2 -> Target`；两个 Jump 使用 Password，Target 使用 Vault
  Private Key，三跳 Host Key 顺序和 Jump 2 认证失败归属均通过。

Agent、Forward 完整矩阵和平台证据尚未齐全，因此本 ADR继续保持 Proposed。

## 相关文档

- [总体技术设计：SSH](../design/technical-architecture-2026.md#5-ssh-技术方案)
- [Phase 0 ExecPlan](../execplans/active/0001-phase-0-technical-validation.md)
