# Design 文档

Design 文档描述系统如何实现，包括组件、数据流、接口、安全边界和失败处理。

## 当前设计

- [technical-architecture-2026.md](technical-architecture-2026.md)：总体技术选型和完整架构设计。
- [vault-bootstrap-v1.md](vault-bootstrap-v1.md)：已实现的 Vault Bootstrap、PIN Slot 和数据库格式。
- [credential-repository-v1.md](credential-repository-v1.md)：Credential Schema、
  Actor Commands 和 Private Key Rust-only 数据流。
- [host-jump-route-repository-v1.md](host-jump-route-repository-v1.md)：Host/
  Jump Route Schema、ID 引用和循环检测。
- [saved-host-connection-plan-v1.md](saved-host-connection-plan-v1.md)：Saved Host
  ID、Rust-only Route 解析和任意长度 Jump Runtime。
- [native-private-key-import-v1.md](native-private-key-import-v1.md)：Rust-owned
  Native Picker、Private Key 文件约束和 IPC 边界。
- [threat-model-v1.md](threat-model-v1.md)：Phase 0 资产、信任边界、威胁控制和
  剩余风险。
- [group-inheritance-v1.md](group-inheritance-v1.md)：Group Schema、三态
  Override、Migration 和 Rust-only Effective Resolution。
- [system-ssh-agent-authentication-v1.md](system-ssh-agent-authentication-v1.md)：
  Linux/Windows 系统 Agent Identity 选择、外部签名和 Secret 边界。
- [native-encrypted-private-key-passphrase-v1.md](native-encrypted-private-key-passphrase-v1.md)：
  加密 OpenSSH Key 检测、Desktop Native Secure Prompt 和 Secret 生命周期。
- [known-host-repository-v1.md](known-host-repository-v1.md)：Endpoint-scoped
  Trust、Schema v6、Durable TOFU 和 Changed-Key 阻断。

## 规则

- 长期决策以 Accepted ADR 为准。
- 多步骤实施工作写入 ExecPlan，不在 Design 中维护任务进度。
- Design 与 ADR 冲突时，更新 Design 以符合最新 Accepted ADR。
- 带“当前版本”含义的内容应移入 `docs/reference/` 并标记核验日期。

后续建议继续按领域拆分：

- `ssh-connection-model.md`
- `sync-protocol.md`
- `terminal-data-path.md`
- `platform-security.md`
