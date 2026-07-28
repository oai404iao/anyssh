# ExecPlan 0005：Known Host Repository and Durable TOFU

- 状态：Active
- 创建日期：2026-07-28
- 最后更新：2026-07-28
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

让用户第一次确认 Host Key 后，AnySSH 能在 Vault 中持久保存 Endpoint Trust，
以后连接和重启无需重复确认；如果服务端 Key 变化，连接必须明确、不可绕过地
阻断。

这是 Phase 1 下一项优先工作，因为现有 SSH Core 已证明 TOFU 和 Changed-Key
状态机，但产品仍缺少真正的 Known Host Repository。

## 范围

### 包含

- Proposed ADR-0015 与 Known Host Repository v1 Design。
- SQLCipher Schema v6 Known Host/Key 表。
- Endpoint 规范化与最多 16 Key 的 Trust Set。
- DB Actor CRUD、Trust、Lookup 和 Connection Plan Policy。
- SSH Core Pending Public Key Evidence 与多 Fingerprint Policy。
- ApplicationCore “先持久化、后继续握手”。
- First-writer-wins 并发 TOFU。
- typed Changed-Key Event 和无 Accept 的阻断 UI。
- Known Hosts metadata-only 管理页和 WebView 外原生确认的显式 Forget。
- Quick、Saved、两 Jump Route 和重启验证。
- Browser、X11、Wayland、Windows、Android/Linux Container 与同 Commit CI。

### 不包含

- OpenSSH `known_hosts` 文件导入/导出。
- Hash、Wildcard、Negation、`@revoked`、`@cert-authority`。
- Host Certificate、`HostKeyAlias`、DNS/CNAME/IP 自动合并。
- `hostkeys@openssh.com` 自动轮换。
- WebDAV Outbox/Operation 实现。
- 在 Changed-Key Dialog 中直接替换 Trust。
- Keyboard-interactive/OTP、Forwarding、多 Tab 或 Key Export。

## 上下文

当前状态：

- `anyssh-ssh` 有 `Prompt` 和单一 `RequireSha256` Policy。
- Prompt Event 只有 Request ID、Hop、Endpoint、Algorithm 和 Fingerprint。
- `SessionControl::confirm_host_key` 会直接解除 SSH Worker 等待。
- `ApplicationCore` 对 Quick/Saved/Jump/Target 都固定生成 `Prompt`。
- SQLCipher 当前为 Schema v5，没有 Known Host 表。
- Host Key 变化 Smoke 已证明匹配免提示、变化硬阻断，但 Fingerprint 只存在于
  测试进程内。

关键路径：

- `crates/anyssh-domain/src/lib.rs`
- `crates/anyssh-storage/src/lib.rs`
- `crates/anyssh-storage/src/actor.rs`
- `crates/anyssh-storage/src/connection_plan.rs`
- `crates/anyssh-ssh/src/lib.rs`
- `crates/anyssh-app/src/lib.rs`
- `apps/client/src-tauri/src/lib.rs`
- `apps/client/src/App.tsx`
- `apps/client/src/components/ConfigurationWorkspace.tsx`
- `scripts/test-ssh-smoke.sh`
- `scripts/qa/native-xvfb-smoke.sh`
- `scripts/qa/native-windows-smoke.ps1`

架构来源：

- Accepted ADR-0002：russh Engine 和 Host Key 变化硬阻断。
- Accepted ADR-0010：Saved Host Plan 只在 Rust 内解析。
- Proposed ADR-0015：Endpoint-scoped Trust 和 persist-before-continue。
- Threat Model T-06/T-07：MITM、轮换和延迟决策绑定。

## Progress

- [x] 2026-07-28：选择 Known Host Repository 作为下一项 Phase 1 工作。
- [x] 2026-07-28：核验 OpenSSH 格式、固定 `ssh-key`/russh 能力和当前
  Session/Application/DB Actor 边界。
- [x] 2026-07-28：创建 OpenSSH Reference、Proposed ADR-0015、Design 和本
  ExecPlan。
- [ ] 完成 Milestone 1：Schema v6 与 Repository。
- [ ] 完成 Milestone 2：SSH/Application Trust Boundary。
- [ ] 完成 Milestone 3：Tauri/React Product UI。
- [ ] 完成 Milestone 4：Protocol 与 Native QA。
- [ ] 完成全量回归、Artifact 人工检查、ADR-0015 状态评审和计划收尾。

## Milestones

### Milestone 1：Schema v6 与 Repository

工作：

1. 在 `anyssh-domain` 定义 Endpoint Identity Normalization v1。
2. 新增 `known_host` Storage Module、CSPRNG ID 和 Summary/Policy 类型。
3. Schema v6 创建 `known_hosts` 与 `known_host_keys`。
4. 增加 Public Key Bytes/Algorithm/Fingerprint 一致性验证。
5. 如 `anyssh-storage` 需要直接使用 `ssh-key`，固定与 russh 相同版本并确认
   Apache-2.0/MIT 兼容性。
6. 实现 Actor Lookup、Trust、List、Delete。
7. 实现 First-writer-wins、幂等和最多 16 Key。
8. 把 Policy 加入 Saved Host Connection Plan。

出口：

- v5 数据无损迁移到 v6。
- Quick/Saved/Jump 可在网络前得到 Prompt 或 Trusted Policy。
- Migration 中断回滚，Locked Actor 拒绝。

### Milestone 2：SSH/Application Trust Boundary

工作：

1. `HostKeyPolicy` 支持有界 SHA-256 Fingerprint Set。
2. SSH Core 保存 metadata Event 之外的 Rust-only Observed Public Key。
3. Request ID 继续绑定 Hop/Endpoint/Key，过期请求拒绝。
4. `ApplicationCore::decide_host_key` 实现 persist-before-continue。
5. DB Failure/Conflict 自动拒绝 Pending Request。
6. 新增 typed Changed-Key Event。

出口：

- WebView 仍只提交 Session ID、Request ID 和 Boolean。
- 第一次接受写库后才继续认证。
- 已知 Endpoint 的不同 Key 无法进入普通确认路径。

### Milestone 3：Tauri/React Product UI

工作：

1. 新增 metadata-only Known Host Bridge 和 typed Tauri Commands。
2. Host Key 主操作改为 `Trust and continue`。
3. Changed-Key 使用独立硬阻断 UI，无 Accept/Replace。
4. Configuration Workspace 增加 Known Hosts 页面。
5. 定义 `KnownHostForgetPrompt` Application Boundary；Linux/Windows 使用原生
   确认，WebView 只提交 Known Host ID。
6. Browser QA、Vitest 和 Playwright 覆盖 Desktop/Compact/Mobile。

出口：

- 用户能查看和经原生确认 Forget Trust，但不能从连接错误直接覆盖。
- Public Key Bytes 不出现在 IPC、React State 或 Browser Fixture。

### Milestone 4：Protocol 与 Native QA

工作：

1. OpenSSH Smoke：首次 Trust、二次免提示、重启恢复、轮换阻断。
2. 两 Jump Route：Jump 1、Jump 2、Target 分别持久化。
3. X11 Native：首次 Prompt、第二次无 Prompt、原生 Forget、轮换阻断。
4. Windows Native：真实 EXE/WebView2、原生 Forget、重启、standalone
   OpenSSH 轮换。
5. 保持 Wayland/IBus、Encrypted Key、System Agent 和 4 MiB 回归。
6. 扫描 SQLCipher/WAL/Sidecar，确认测试 Endpoint、Fingerprint 和 Key Blob
   不以明文存在。

出口：

- Linux/Windows 真实 Runtime 证明 Durable TOFU。
- Changed-Key 不提示重新信任且不修改旧记录。

### Milestone 5：全量回归与治理收尾

工作：

1. Workspace、Frontend、Browser、OpenSSH、Native 和 Container 全量回归。
2. 同 Commit CI 九个 Job。
3. 下载并人工检查关键截图、报告、Browser Error 和 Build Hash。
4. 更新 Threat Model、Status、Roadmap、README 和 AGENTS。
5. 根据证据接受、拒绝或替代 ADR-0015。
6. 完成后移动本计划到 `completed/`。

出口：

- Schema v6、Runtime、UI、Native Evidence 和治理文档一致。

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
pnpm check:android
pnpm check:container:linux
pnpm check:container:android
pnpm docs:check
pnpm format:check
git diff --check
```

验收重点：

- Schema v5 -> v6 和中断恢复。
- Endpoint 规范化和 Port 隔离。
- Public Key Bytes/Algorithm/Fingerprint 一致性。
- First-writer 相同 Key 幂等、不同 Key 冲突。
- Persist Failure 不继续认证。
- Quick/Saved/Jump/Target 共用 Trust。
- 首次 Prompt、二次免提示、重启恢复、Forget 后重提示。
- WebView 不能绕过 Native Forget Confirmation。
- Changed-Key typed hard block，无 Accept/Replace，无数据库覆盖。
- Public Key Bytes 不进入 IPC/React/Error/Debug。
- SQLCipher 文件中无测试 Known Host 明文。

## Surprises & Discoveries

- 2026-07-28：固定的 `ssh-key 0.7.0-rc.11` 已能解析 OpenSSH Known Hosts
  Entry，并能把 Public Key 转为规范 Bytes；无需自写 Key Parser。
- 2026-07-28：russh 自带 Known Hosts Helper 直接读写用户主目录文件，不符合
  Vault、DB Actor、移动端和未来同步边界。
- 2026-07-28：当前 `SessionControl::confirm_host_key` 会立即解除 SSH Worker
  等待，无法保证数据库先成功，因此必须增加 Rust-only Pending Key Evidence 和
  ApplicationCore 编排。
- 2026-07-28：系统文件支持 Hash、Pattern、Marker 和多 Key；如果第一版把这些
  全部与 Runtime Repository 同时实现，会扩大安全语义和测试矩阵，因此先完成
  Exact Endpoint Durable TOFU。

## Decision Log

- 2026-07-28：下一项 Phase 1 工作选择 Known Host Repository，而不是 WebDAV、
  Forwarding 或多 Tab；原因是它关闭现有 Host Key 状态机最重要的产品安全缺口。
- 2026-07-28：Trust 按规范化 Endpoint，而不是 Host ID 建模。
- 2026-07-28：保存完整 Public Key Bytes，同时保存并校验 Algorithm/SHA-256
  Fingerprint。
- 2026-07-28：首次接受必须先持久化再继续握手。
- 2026-07-28：并发 TOFU 使用 First-writer-wins，不自动合并不同 Key。
- 2026-07-28：Changed-Key 保持硬阻断；重新信任只能通过独立 Forget 流程。
- 2026-07-28：Forget Trust 必须经过 WebView 外的原生确认，避免被攻陷的
  WebView 静默降级已有 Host 身份。
- 2026-07-28：OpenSSH 文件导入/导出从本计划分离，但 Schema 保留完整 Key。

## Outcomes & Retrospective

尚未完成。
