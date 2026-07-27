# ADR-0009：Host 与 Jump Route 只保存 ID 引用

- 状态：Accepted
- 日期：2026-07-27
- 决策人：项目维护者

## 背景

AnySSH 需要持久化 Host 和有序 Jump Route，同时保持 Credential Secret 的单一
所有权。若 Host、Route Step 或 IPC 复制用户名、密码、Private Key 或
Passphrase，会造成更新不一致并扩大秘密暴露面。旧 Schema v1/v2 的兼容
`hosts` 表仍内嵌一个记录级加密 Password，必须提供保留数据的迁移路径。

## 决策

- Host 保存 endpoint 元数据、可选 `credential_id` 和可选 `jump_route_id`。
- Jump Route 保存 Label 和有序 Host ID 列表，不复制 Host endpoint 或
  Credential 数据。
- Credential 的 Username 和 Secret 只存在于 Credential Repository。
- 新 Host、Route 和 Credential ID 由 Rust CSPRNG 生成，WebView 不指定 ID；
  Schema Migration 保留已有 Host ID。
- SQLCipher Foreign Key 使用 Restrict 语义；被 Host/Route 引用的对象不能静默
  删除。
- 保存 Host 或 Jump Route 时对 Host -> Route -> Step Host 图执行循环检测。
- Schema v2 到 v3 在单个 `IMMEDIATE` Transaction 中把旧 Host Password
  解密、重新加密为 Password Credential，并让迁移后的 Host 引用该 Credential。
- Tauri CRUD 只返回非敏感 Host/Route DTO，不提供 Secret 字段。

## 备选方案

- Host 内嵌 Credential：更新和同步会产生多个 Secret 副本，拒绝。
- Route Step 内嵌 endpoint 与 Credential：容易产生陈旧副本，拒绝。
- 删除旧 Host 数据：违反 Vault Migration 的已有数据恢复要求，拒绝。
- 删除被引用对象时自动清空引用：会静默改变连接语义，拒绝。

## 后果

### 正面

- Credential Secret 保持单一存储和统一生命周期。
- 同一个 Credential 可以被多个 Host 安全复用。
- Jump Route 顺序稳定，并可直接被 Rust-only Connection Plan 递归解析。
- 旧 Phase 0 Host 数据可无损迁移。

### 代价与风险

- Host、Route、Credential CRUD 需要 Foreign Key 与引用占用错误处理。
- Route 更新需要全图循环检测。
- Route Runtime 由 ADR-0010 的 Rust-only Connection Plan 执行；持久化模型仍
  不包含展开后的 endpoint 或 Credential 副本。

## 验证

- v2 -> v3 成功迁移、迁移中断回滚和重启恢复。
- 数据库、WAL、Bootstrap 和 Sidecar 中无测试 Host/Credential 明文。
- Host/Route Summary 与 Tauri JSON 不包含 Password、Private Key 或 Passphrase。
- 不存在的 Credential/Route/Host 引用被拒绝。
- 删除被引用 Credential、Host 或 Route 被拒绝。
- 直接和间接 Jump Route 循环被拒绝。

### 当前证据

- 2026-07-27：Storage 与 DB Actor 测试覆盖 CSPRNG ID、CRUD、Lock/Unlock、
  Route 顺序、无效引用、Restrict 删除和直接/间接循环。
- 2026-07-27：真实 SQLCipher Vault 已验证 v2 Host Password 重新加密为
  Password Credential、Host ID 保持不变、v3 中断完整回滚和重启恢复。
- 2026-07-27：Tauri/TypeScript IPC 测试确认 Host/Route JSON 只含元数据与 ID，
  并拒绝 Host Password 和 Route Credential 字段。
- 2026-07-27：原生 X11/Wayland Vault QA 与 Android ARM64 Debug Build 通过。
- 2026-07-27：Commit `5e366fd` 的 GitHub Actions Run `30245997616` 九个 Job
  全部通过，包括 Windows、Android/Linux Container、原生 X11/Wayland、
  OpenSSH、浏览器和 Rust Core。
- 2026-07-27：后续 Saved Host Runtime 直接使用本 ADR 的 Host/Route ID 图，
  WebView 不读取 Route Step 或 Credential。

## 相关文档

- Design：[Host 与 Jump Route Repository v1](../design/host-jump-route-repository-v1.md)
- ExecPlan：[Phase 0 技术风险验证](../execplans/completed/0001-phase-0-technical-validation.md)
- Supersedes：
- Superseded by：
