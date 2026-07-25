# ExecPlan 规范

ExecPlan 是给人类和 Coding Agent 共同使用的“可执行活文档”。它不仅描述最终目标，还必须在执行过程中记录进度、发现、决策和结果。

## 目录

```text
execplans/
├── README.md
├── 0000-template.md
├── active/                     # 正在执行
└── completed/                  # 已结束，保留历史
```

## 何时必须创建 ExecPlan

满足任一条件时必须使用：

- 跨多个目录或模块。
- 预计需要多个独立步骤。
- 涉及架构、安全、存储、同步或迁移。
- 需要技术验证或存在明显不确定性。
- 工作可能跨多个 Agent Session。

小型拼写、单文件无风险修复不要求单独 ExecPlan。

## 必需章节

每个 ExecPlan 至少包含：

1. **目的与用户价值**
2. **范围与非目标**
3. **上下文**
4. **Progress**
5. **Milestones**
6. **Validation**
7. **Surprises & Discoveries**
8. **Decision Log**
9. **Outcomes & Retrospective**

## 更新规则

- Progress 必须与实际仓库状态一致。
- 每完成一个里程碑立即更新，不等任务结束后补写。
- 发现与原设计冲突时，先记录 Surprises，再记录 Decision。
- 影响长期架构的 Decision 必须同步到 ADR。
- 命令必须来自真实 package、workspace、script 或 CI，不写猜测命令。
- 完成后移动到 `completed/`，保留过程信息。

## 命名

```text
NNNN-short-kebab-case-title.md
```

当前计划：

- [Phase 0：技术风险验证](active/0001-phase-0-technical-validation.md)
