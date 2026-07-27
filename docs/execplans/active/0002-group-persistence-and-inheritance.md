# ExecPlan 0002：Group 持久化与三态继承

- 状态：Active
- 创建日期：2026-07-27
- 最后更新：2026-07-27
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

让用户可以把 Host 组织为树形 Group，并从父 Group 继承 Credential 与 Jump
Route，同时能明确覆盖或清除父级值。连接仍只提交 Host ID，所有 Effective
Configuration 和 Credential Secret 在 Rust 内解析。

## 范围

### 包含

- SQLCipher Schema v4 Group Repository。
- Rust CSPRNG Group ID、Parent 引用、Restrict 删除和循环检测。
- `Inherit / Set / Clear` 通用 Override 模型。
- Host 的 Group ID、Credential Override 和 Jump Route Override。
- v3 -> v4 原子 Migration 与中断回滚。
- DB Actor/ApplicationCore/Tauri metadata-only CRUD。
- Rust-only Effective Host Connection Plan。
- Group Tree、Host Group 选择和三态配置 UI。
- Workspace、OpenSSH、Playwright、agent-browser 与原生平台回归。

### 不包含

- WebDAV Sync。
- Proxy、Port Forward、Terminal Profile 和算法字段继承。
- Secret Reveal/Export。
- SSH Agent、Keyboard-interactive 和加密 Key Passphrase Prompt。
- Group ACL 或多用户共享。

## 上下文

Phase 0 已完成 Schema v3 Credential/Host/Jump Route Repository、Rust-only Saved
Host Plan 和配置 UI。Accepted ADR-0006/0009/0010 要求 Secret 与 Connection Plan
保持在 Rust 边界。Proposed ADR-0012 定义 Group 三态语义。

关键路径：

- `crates/anyssh-storage/src/lib.rs`
- `crates/anyssh-storage/src/actor.rs`
- `crates/anyssh-storage/src/connection_plan.rs`
- `crates/anyssh-app/src/lib.rs`
- `apps/client/src-tauri/src/lib.rs`
- `apps/client/src/components/ConfigurationWorkspace.tsx`

威胁与失败模式：

- Migration 不能改变现有 Host 的 Effective Credential/Route。
- Group/Host 循环、深度膨胀和缺失引用必须在写入或连接前失败。
- WebView 不得获得解析后的 Credential Secret。
- 删除 Group/Credential/Route 不得静默改变连接行为。
- Sync 尚未实现，但持久化必须保留 Override State，而非 Effective Value。

## Progress

- [x] 2026-07-27：创建 ADR-0012 与 Group Inheritance v1 Design。
- [x] 2026-07-27：创建本 ExecPlan，并明确 Migration、安全和同步边界。
- [ ] 实现通用 Override、Group Model 和 Schema v4 Migration。
- [ ] 实现 Repository/Actor/Application/Tauri Commands。
- [ ] 扩展 Saved Host Effective Plan 与 OpenSSH Smoke。
- [ ] 实现 Group/Host 三态配置 UI。
- [ ] 完成全平台回归与 ADR-0012 状态评审。

## Milestones

### Milestone 1：Schema v4 与领域模型

1. 增加 Group ID、Group Model 和 `Override<T>`。
2. 定义 State/Value CHECK Constraint。
3. 实现 v3 -> v4 Migration。
4. 覆盖成功、重启、中断回滚和明文扫描。

出口：

- 旧 Host 的 Effective Credential/Route 与 v3 一致。
- Group/Override 数据可在 Lock/Unlock 后恢复。

### Milestone 2：Repository 与完整性

1. Group Create/Update/List/Delete。
2. Host Group/Override Update。
3. Parent 全图循环检测和 32 层限制。
4. Restrict 删除和稳定错误分类。

出口：

- 无效引用、循环、超深和被占用删除全部事务性失败。

### Milestone 3：Effective Connection Plan

1. 在 DB Actor 内解析 Host -> Group Parent Chain。
2. 对 Credential/Route 应用 Inherit/Set/Clear。
3. 保持 Saved Host IPC 只提交 Host ID。
4. 增加 Group Inherited Jump Route OpenSSH Smoke。

出口：

- WebView 不读取 Secret 或展开 Route 即可连接继承配置的 Host。

### Milestone 4：产品 UI

1. Group Tree 和 Parent 选择。
2. Host Group 选择。
3. Credential/Route 三态 Editor。
4. Local State 与 Effective Metadata 展示。
5. 桌面、Compact 和移动响应式检查。

出口：

- 用户能观察并修改继承、覆盖和清除结果。

### Milestone 5：回归与决策

1. 运行全部 Workspace、OpenSSH、Browser 和 Native QA。
2. 验证 Windows/Android/Linux Build。
3. 更新 Threat Model、Design、Status 和 ADR-0012。

出口：

- CI 全部通过，证据截图已人工检查。

## Validation

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm test:ssh:smoke
pnpm test:e2e
pnpm qa:browser
pnpm qa:native:xvfb
pnpm qa:native:wayland
pnpm qa:native:windows
pnpm check:container:linux
pnpm check:container:android
pnpm docs:check
pnpm format:check
```

`pnpm qa:native:windows` 只在 Windows 执行。

## Surprises & Discoveries

- 尚无。

## Decision Log

- 2026-07-27：首版只继承 Credential ID 和 Jump Route ID，不为未来字段创建
  空列或空表。
- 2026-07-27：Effective Configuration 在 DB Actor/Rust 内解析，WebView 只
  显示 metadata。
- 2026-07-27：v3 空引用迁移为 Inherit；无 Group 时 Effective None，保持语义。

## Outcomes & Retrospective

尚未完成。
