# AnySSH Host 与 Jump Route Repository v1

> 状态：已实现
> 日期：2026-07-27

本文定义在 SQLCipher Schema v3 引入的 Vault-backed Host/Jump Route
Repository。当前 Schema v7 延续了 Schema v4 的 Group/三态 Host 引用列，完整定义见
[Group Inheritance v1](group-inheritance-v1.md)。Known Host 由
[Known Host Repository v1](known-host-repository-v1.md) 单独定义。
任意长度 Jump Route 的 Rust-only 执行见
[Saved Host Connection Plan v1](saved-host-connection-plan-v1.md)。

## 安全边界

- Host 不保存 Username、Password、Private Key 或 Passphrase。
- Host 只保存 Group ID 和 Credential/Route 的三态引用；Credential Secret 仍由
  Credential Repository 独占。
- Jump Route 只保存有序 Host ID，不复制 endpoint 或 Credential。
- Tauri CRUD 只返回 SQLCipher 保护的非敏感元数据和不透明 ID。
- Vault Locked 时所有 Host/Route Repository Command 必须失败。

## Schema v3

```sql
CREATE TABLE jump_routes(
    id TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE hosts(
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
    credential_id TEXT,
    jump_route_id TEXT,
    FOREIGN KEY(credential_id) REFERENCES credentials(id) ON DELETE RESTRICT,
    FOREIGN KEY(jump_route_id) REFERENCES jump_routes(id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE jump_route_steps(
    route_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK(position >= 0),
    host_id TEXT NOT NULL,
    PRIMARY KEY(route_id, position),
    UNIQUE(route_id, host_id),
    FOREIGN KEY(route_id) REFERENCES jump_routes(id) ON DELETE CASCADE,
    FOREIGN KEY(host_id) REFERENCES hosts(id) ON DELETE RESTRICT
) WITHOUT ROWID;
```

新 Host 和 Route ID 由 Rust CSPRNG 生成 128-bit 随机值；迁移保留已有 Host
ID。Route 至少包含一个、最多包含 32 个唯一 Host。

## 引用与循环

逻辑图为：

```text
Host -> optional Jump Route -> ordered Step Hosts
```

创建或更新 Host/Route 后，在同一 Transaction 内对完整图执行 DFS。任何直接或
间接回到当前 Host 的路径都拒绝提交。

删除采用 Restrict 语义：

- Credential 被 Group/Host 的 `Set` Override 引用时不能删除。
- Host 被 Route Step 引用时不能删除。
- Jump Route 被 Group/Host 的 `Set` Override 引用时不能删除。

## Schema v2 到 v3

旧 `hosts` 记录包含 `username` 和记录级加密 Password。迁移在一个
`IMMEDIATE` Transaction 中：

1. 使用旧 v1 Host AAD 解密 Password。
2. 为每个旧 Host 生成 Password Credential ID。
3. 使用 Credential Repository AAD 重新加密 Password。
4. 创建新 Host，并保存该 Credential ID。
5. 删除旧 Host 表并把 `user_version` 更新到 `3`。

中断时必须保留完整 Schema v2，下次解锁可重试。Bootstrap、VMK 和 Key Slot
不变。

## Repository Commands

DB Actor 顺序处理：

- Group Commands 见 [Group Inheritance v1](group-inheritance-v1.md)。
- `CreateHost` / `UpdateHost` / `ListHosts` / `DeleteHost`
- `CreateJumpRoute` / `UpdateJumpRoute` / `ListJumpRoutes` /
  `DeleteJumpRoute`

所有 ID 引用在 Actor-owned SQLCipher Transaction 内校验。

## 同步边界

当前实现只写本地 SQLCipher，不创建 Outbox，且不得同步数据库文件。后续 WebDAV
Operation Log 中，Host 的 Credential/Route ID 作为普通引用字段合并；Jump Route
的有序 Host ID 列表必须使用版本化原子更新或明确的顺序 Operation，并在合并后
重新执行引用与循环校验。

## 验证

- Host、Route 和 Credential 引用在 Lock/Unlock 与重启后恢复。
- v2 Host Password 自动迁移为 Credential，且旧 Host ID 保持不变。
- Migration 中断回滚。
- 无效引用、删除占用和 Route 循环被拒绝。
- Host/Route Debug、List 和 IPC JSON 不包含 Credential Secret。
- Saved Host ID 解析与多跳 Runtime 见独立 Connection Plan 设计。
