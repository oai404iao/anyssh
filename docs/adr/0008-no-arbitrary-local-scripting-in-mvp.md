# ADR-0008：MVP 不允许任意本地脚本执行

- 状态：Accepted
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

产品需要脚本管理，但任意 JavaScript、插件或本地 Shell 会直接接触 Credential、文件系统和网络，显著扩大攻击面，也增加移动平台审核与兼容成本。

## 决策

MVP 仅支持：

- Snippet。
- 受限 Runbook Step。
- SSH Exec。
- PTY Send。
- Wait/Prompt/Confirm。
- Rust Core 注入的 Secret Reference。

不支持：

- `eval`。
- 任意本地 Shell。
- 自动加载第三方插件。
- 远程下载并执行脚本。

## 备选方案

- 内置 JavaScript Runtime：生态丰富，但权限隔离和秘密保护复杂。
- 直接调用系统 Shell：桌面简单，但移动端不可移植且风险高。

## 后果

### 正面

- MVP 安全边界可控。
- 桌面与移动行为一致。
- Runbook 可以进行结构化确认和审计。

### 代价与风险

- 自动化能力不如完整脚本语言。
- 后续引入 Rhai/Starlark/WASM 时需要新 ADR。

## 验证

- Runbook 无法访问未授权本地文件或原生命令。
- Secret 变量由 Rust 注入，前端和持久化日志不出现明文。
- 批量执行具备并发上限和危险步骤确认。

### 当前结论

Phase 0 未引入 JavaScript Runtime、系统 Shell 或第三方插件加载入口。该 ADR
属于 MVP 权限边界而非某个库的技术验证，现接受此范围决策。Snippet/Runbook
实现仍需在后续 ExecPlan 中证明 Secret 注入、并发上限和危险步骤确认。

## 相关文档

- [总体技术设计：脚本管理](../design/technical-architecture-2026.md#10-脚本管理)
