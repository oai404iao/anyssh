# Group Persistence and Three-State Inheritance v1

> 状态：设计中
> 日期：2026-07-27

## 目标

在不复制 Credential Secret、Host Endpoint 或展开 Route 的前提下，为 Host
提供树形 Group 和显式三态继承。

## 领域模型

```rust
enum Override<T> {
    Inherit,
    Set(T),
    Clear,
}

struct Group {
    id: GroupId,
    label: String,
    parent_group_id: Option<GroupId>,
    credential_id: Override<CredentialId>,
    jump_route_id: Override<JumpRouteId>,
}

struct Host {
    id: HostId,
    display_name: String,
    endpoint: SshEndpoint,
    group_id: Option<GroupId>,
    credential_id: Override<CredentialId>,
    jump_route_id: Override<JumpRouteId>,
}
```

首版只让 Credential ID 和 Jump Route ID 继承。Proxy、Forward、Terminal Profile
和算法策略后续复用相同编码，不提前创建空表或空字段。

## 解析规则

对每个可继承字段：

1. Host `Set(value)` 直接使用该值。
2. Host `Clear` 得到 None。
3. Host `Inherit` 从直接 Group 开始向父级查找。
4. Group `Set(value)` 停止并返回该值。
5. Group `Clear` 停止并返回 None。
6. 到 Root 仍为 `Inherit` 时返回 Application Default；v1 默认 None。

解析必须在同一个 DB Actor Command 中完成，并最多遍历 32 层。

## Schema v4 草案

```text
groups
  id TEXT PRIMARY KEY
  label TEXT NOT NULL
  parent_group_id TEXT NULL REFERENCES groups(id) RESTRICT

group_overrides
  group_id TEXT PRIMARY KEY REFERENCES groups(id) ON DELETE CASCADE
  credential_state INTEGER NOT NULL
  credential_id TEXT NULL REFERENCES credentials(id) RESTRICT
  jump_route_state INTEGER NOT NULL
  jump_route_id TEXT NULL REFERENCES jump_routes(id) RESTRICT

hosts
  ...
  group_id TEXT NULL REFERENCES groups(id) RESTRICT
  credential_state INTEGER NOT NULL
  credential_id TEXT NULL REFERENCES credentials(id) RESTRICT
  jump_route_state INTEGER NOT NULL
  jump_route_id TEXT NULL REFERENCES jump_routes(id) RESTRICT
```

State 编码：

```text
0 = Inherit, value 必须 NULL
1 = Set, value 必须非 NULL
2 = Clear, value 必须 NULL
```

数据库使用 CHECK Constraint 保证 State/Value 一致。

## Migration v3 -> v4

- 在单个 `IMMEDIATE` Transaction 内创建 Group/Override 结构。
- 已有 Host `credential_id IS NOT NULL` -> `Set(id)`。
- 已有 Host `credential_id IS NULL` -> `Inherit`。
- Jump Route 同理。
- 现有 Host `group_id` 为 NULL，因此 Effective Value 与 v3 完全一致。
- 中断时 `user_version` 和旧表必须保持 v3 完整状态。

## 完整性

- Group ID 由 Rust CSPRNG 生成。
- Parent Group 图使用 Transaction 内全图 DFS 检测循环。
- 最大 Parent 深度为 32。
- 删除有 Child Group 或 Host 的 Group 时失败。
- 删除被 Group/Host `Set` 引用的 Credential/Route 时失败。
- Label 不是安全标识，不参与引用或 AAD。

## Rust 边界

- Storage/Actor 返回 Local Override DTO 和 metadata-only Effective Summary。
- Rust-only Effective Connection Plan 可以包含解析后的 Credential，但不实现
  Serialize。
- Tauri Saved Host Connect Request 继续只包含 Host ID 与 Terminal Size。
- WebView 不递归加载 Group 后自行解析连接配置。

## UI

- Group Tree 支持创建 Root/Child、重命名和选择 Parent。
- Host Editor 支持选择 Group。
- Credential/Jump Route 控件为三态：
  - Inherit from Group
  - Set
  - Clear
- UI 同时展示 Local State 和 Effective Metadata，不能展示 Secret。
- 删除或循环错误使用稳定分类，不显示 SQL 或内部路径。

## 同步边界

Phase 1 不实现 WebDAV，但未来 Operation 必须同步：

- Parent Group ID。
- 每个字段的 Override State。
- `Set` 时的引用 ID。

不得只同步 Effective Value。

## 验证

- Override 单元测试覆盖全部三态组合。
- Schema v4 Migration、回滚、重启和明文扫描。
- Actor CRUD、Restrict 删除、循环和最大深度。
- Saved Host OpenSSH Smoke 验证 Group 继承 Credential/Route。
- Vitest、Playwright、agent-browser、X11、Wayland、Windows 和 Android 回归。
