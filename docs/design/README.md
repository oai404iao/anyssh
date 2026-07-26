# Design 文档

Design 文档描述系统如何实现，包括组件、数据流、接口、安全边界和失败处理。

## 当前设计

- [technical-architecture-2026.md](technical-architecture-2026.md)：总体技术选型和完整架构设计。
- [vault-bootstrap-v1.md](vault-bootstrap-v1.md)：已实现的 Vault Bootstrap、PIN Slot 和数据库格式。

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
