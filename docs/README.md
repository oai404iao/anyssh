# AnySSH 文档导航

文档按用途拆分，避免把产品目标、长期决策和短期任务混在同一文件中。

## 项目文档

路径：[`project/`](project/README.md)

- [产品构想](project/product-brief.md)
- [项目状态](project/status.md)
- [产品与技术路线图](project/roadmap.md)

项目文档回答“做什么、为什么做、当前做到哪里”。

## Design

路径：[`design/`](design/README.md)

- [2026 总体技术架构](design/technical-architecture-2026.md)
- [Vault Bootstrap v1](design/vault-bootstrap-v1.md)
- [Credential Repository v1](design/credential-repository-v1.md)
- [Host 与 Jump Route Repository v1](design/host-jump-route-repository-v1.md)
- [Saved Host Connection Plan v1](design/saved-host-connection-plan-v1.md)

Design 文档回答“系统如何工作、模块边界是什么”。

## ADR

路径：[`adr/`](adr/README.md)

ADR 记录一项长期决策的背景、选择、后果和状态。状态为 Accepted 的 ADR 是架构决策源。

## ExecPlan

路径：[`execplans/`](execplans/README.md)

ExecPlan 是 Agent 可持续更新的执行计划。活动计划放在 `execplans/active/`，完成后移入 `execplans/completed/`。

## 参考文档

路径：[`reference/`](reference/README.md)

- [2026 技术版本基线](reference/technology-baseline-2026.md)
- [术语表](reference/glossary.md)

参考文档记录外部事实、协议、版本和术语，不替代 ADR。

## 推荐阅读顺序

1. 根目录 [`AGENTS.md`](../AGENTS.md)。
2. [产品构想](project/product-brief.md)。
3. [项目状态](project/status.md)。
4. 与任务相关的 Accepted/Proposed ADR。
5. 当前活动 ExecPlan。
6. 对应 Design 和 Reference 文档。
