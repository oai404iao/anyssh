# ADR-0012：Group 继承使用显式三态 Override

- 状态：Proposed
- 日期：2026-07-27
- 决策人：项目维护者

## 背景

Group 需要向子 Group 和 Host 继承 Credential、Jump Route、Proxy、Terminal
Profile 等配置。普通 `Option<T>` 只能表达“有值/无值”，无法区分：

- 没有本地配置，继续继承父级。
- 显式设置一个值。
- 显式清除父级提供的值。

若把父配置复制到 Host，会产生陈旧副本、扩大同步冲突，并可能复制 Credential
引用或未来 Secret。

## 决策

- 所有可继承字段使用：

  ```text
  Inherit
  Set(value)
  Clear
  ```

- Group 保存可选 Parent Group ID，并拒绝直接或间接循环。
- Host 保存可选 Group ID；Endpoint 和 Display Name 仍属于 Host 本身。
- Credential、Jump Route 及未来 Profile 只继承 ID 引用，不复制目标记录。
- Effective Host Configuration 在 DB Actor/ApplicationCore 的 Rust 边界内解析。
- WebView 可以读取本地 Override 状态和非敏感 Effective Summary，但不能解析或
  返回 Credential Secret。
- Group/Host/Route/Credential 删除继续使用 Restrict 语义。
- 继承深度必须有上限；v1 使用最多 32 层 Group。
- Schema v3 -> v4 Migration 把已有 Host 的非空 Credential/Route 引用迁移为
  `Set(id)`；空引用迁移为 `Inherit`，无 Group 的 Effective Value 仍为 None。

## 备选方案

- 普通 Nullable 字段：无法区分 Inherit 和 Clear，拒绝。
- 保存 Effective Value：父级变更后会产生陈旧副本，拒绝。
- WebView 递归解析：产生竞态并扩大 Credential/Topology 边界，拒绝。
- 删除 Group 时自动移动/清空 Host：会静默改变连接行为，拒绝。

## 后果

### 正面

- 父级修改可以确定地影响后代。
- Host 可以明确阻止某个父级配置。
- 不复制 Credential Secret 或展开后的 Route。
- 未来 Proxy/Profile 字段可以复用同一模型。

### 代价与风险

- Schema、DTO、UI 和 Connection Plan 都必须理解三态。
- Migration 和 Effective Resolution 需要覆盖深度、循环与缺失引用。
- 同步 Operation 必须保留 Override State，不能只同步 Effective Value。

## 验证

- v3 -> v4 Migration 成功、可中断回滚并保持现有连接语义。
- Parent/Child Group 顺序、直接/间接循环和 32 层限制。
- Inherit、Set、Clear 的全组合解析。
- Host 连接只提交 Host ID，Rust 内解析 Effective Credential/Route。
- 被 Group/Host 引用的对象删除失败。
- IPC/Debug/日志不包含 Credential Secret。

## 当前证据

- 2026-07-27：Schema v4 已实现 `host_groups`、`group_overrides`、Host Group ID
  和带 CHECK Constraint 的 Credential/Route State/Value 列。
- 2026-07-27：v3 -> v4 成功迁移、语义保持、中断回滚、重启恢复、明文扫描和
  非法 State/Value 拒绝测试通过。
- 2026-07-27：Storage/Actor 覆盖 Parent/Child、直接/间接循环、32 层限制、
  Inherit/Set/Clear、Restrict 删除和 Effective Jump Route 循环。
- 2026-07-27：Tauri/React 已提供 metadata-only Group/Host DTO、Group Tree 和
  三态 Editor；Saved Host IPC 仍只提交 Host ID。
- 2026-07-27：Docker OpenSSH 已验证 Group 继承 Password/Private Key
  Credential 和两级 Jump Route。
- 2026-07-27：Playwright、agent-browser、X11 Native Picker/SSH 和真实 Wayland/
  IBus/SSH 本地回归通过。Windows、Android 和 Container CI Evidence 尚待同一
  Feature Commit。

## 相关文档

- [Group Inheritance v1](../design/group-inheritance-v1.md)
- [ADR-0006](0006-secrets-stay-out-of-webview.md)
- [ADR-0009](0009-host-jump-route-reference-model.md)
- [ADR-0010](0010-saved-host-plans-resolve-in-rust.md)
- [Phase 1 Group ExecPlan](../execplans/active/0002-group-persistence-and-inheritance.md)
