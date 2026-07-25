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

## 相关文档

- [总体技术设计：SSH](../design/technical-architecture-2026.md#5-ssh-技术方案)
- [Phase 0 ExecPlan](../execplans/active/0001-phase-0-technical-validation.md)
