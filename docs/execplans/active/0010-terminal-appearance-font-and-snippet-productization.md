# ExecPlan 0010：Terminal Appearance, Font, and Snippet Productization

- 状态：Active
- 创建日期：2026-07-29
- 最后更新：2026-07-29
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

让用户可以选择 App/Terminal Theme、系统或导入字体，并管理可复用的 SSH
Snippet；这些能力必须跨重启持久化、适配 Desktop/Mobile UI，并且不引入任意
Theme Script、本地 Shell、WebView Path 或 Secret 插值。

## 范围

### 包含

- Proposed ADR-0020 与 Terminal Appearance/Font/Snippet v1 Design。
- Schema v7 -> v8 Appearance、Custom Theme、Imported Font Metadata 和 Snippet。
- App System/Dark/Light、Built-in/Custom Terminal Theme。
- Bundled/System/Imported Font、Font Size/Line Height/Ligature/Ambiguous Width。
- Snippet CRUD、`{{variable}}`、Insert/Run、Multi-line Confirmation。
- Typed Tauri IPC、Browser QA、X11/Wayland/Windows Native Evidence。
- Linux/Android/Windows Build 与同 Commit CI。

### 不包含

- Theme/Plugin Marketplace、Remote Font、任意 CSS/JavaScript。
- Secret Variable、Credential 插值、Runbook、批量 Host 或 Schedule。
- 本地 Shell、`eval`、Rhai、Starlark 或第三方 Plugin。
- Per-Host/Group Appearance Override。
- Android/iOS Custom Font Picker；v1 仅保留类型和 Unsupported 结果。

## 上下文

当前：

- App Design Token 和 Terminal Palette/Font 硬编码在 `App.css` 与
  `TerminalPane.tsx`。
- Bundled JetBrains Mono Nerd Font 与 Noto Emoji 已存在。
- Terminal Tab 最多 8 个，Inactive xterm 必须保持 Mounted/Ack。
- Schema v7 已包含 Credential/Group/Host/Route/Known Host/Interactive。
- 当前没有 Settings、Theme、Font 或 Snippet Repository。
- ADR-0008 已禁止任意本地脚本执行。

关键路径：

- `apps/client/src/App.tsx`
- `apps/client/src/App.css`
- `apps/client/src/components/TerminalPane.tsx`
- `apps/client/src/components/ConfigurationWorkspace.tsx`
- `apps/client/src-tauri/src/lib.rs`
- `crates/anyssh-app/src/lib.rs`
- `crates/anyssh-storage/src/lib.rs`
- `crates/anyssh-storage/src/actor.rs`
- `apps/client/e2e/connect-preview.spec.ts`
- `scripts/qa/agent-browser-smoke.sh`
- `scripts/qa/native-xvfb-smoke.sh`
- `scripts/qa/native-wayland-ime-smoke.sh`
- `scripts/qa/native-windows-smoke.ps1`

## Progress

- [x] 2026-07-29：Private Key Management Head
  `6dd5cd13e85d4b746bb3b7f60d8783e2b75d8eec` 的 GitHub Actions Run
  `30427696136` 九个 Job 全部通过；ADR-0019 已接受，ExecPlan 0009 已完成。
- [x] 2026-07-29：创建 Proposed ADR-0020、Design 和本 ExecPlan。
- [x] 2026-07-29：完成 Milestone 1：Schema v8、v7 -> v8 Migration、
  Appearance/Theme/Font/Snippet Repository、Snippet Body Record AEAD 和 DB
  Actor/ApplicationCore Typed API。
- [x] 2026-07-29：完成 Milestone 2：App Light/Dark/System、Built-in/Custom
  Terminal Theme、Desktop/Mobile Appearance Workspace，以及 Mounted xterm.js
  原地 Theme/Font/Size/Line-height/Ligature/Ambiguous Width 更新。
- [x] 2026-07-29：完成 Milestone 3：有界 System Font Catalog、Linux/Windows
  Native Picker、TTF/OTF/TTC/WOFF2 Parse、SHA-256 Managed Asset、受限
  `anyssh-font` Protocol、Integrity/Fallback、Live Registration 和 Orphan Cleanup。
- [x] 2026-07-29：完成 Milestone 4：Snippet Summary-only List、显式 Edit、
  Literal `{{variable}}`、Insert/Run、Multi-line Preview/Confirmation 和真实 SSH
  Marker。
- [ ] 完成 Milestone 5：Native QA、CI 与治理。
- [x] 2026-07-29：Frontend/Vitest/Playwright/Browser QA、Rust Workspace、
  OpenSSH Smoke、Linux X11、Wayland/IBus、Linux Container 和 Android ARM64
  Container Build 已通过。
- [ ] Windows Native Picker/WebView2/Restart Evidence 与同 Commit GitHub Actions
  尚待 authoritative Windows Runner 验证。

## Milestones

### Milestone 1：Schema v8 与 Rust Repository

1. Appearance Singleton、Custom Terminal Theme、Font Metadata 和 Snippet Schema。
2. v7 -> v8 Migration、重启、中断回滚和旧引用保持。
3. Snippet Body Record AEAD、Parser、Limits 和 Summary-only List。
4. DB Actor/ApplicationCore Typed Operations。

出口：

- `user_version = 8`，旧 v7 Vault 可恢复迁移。
- Snippet List 不返回 Body。

### Milestone 2：Appearance 与 Mounted Terminal Runtime

1. CSS Design Token 拆出 Dark/Light/System。
2. Built-in/Custom Terminal Theme 验证与选择。
3. `TerminalPane` 原地更新 Theme/Font，不重建 xterm。
4. Desktop/Mobile Appearance Workspace 和 Browser Persistence Simulation。

出口：

- Theme 切换不影响 Tab、Scrollback、Output Ack 或 Session。
- Theme 数据不包含 CSS/URL/Script。

### Milestone 3：System/Imported Font

1. Rust System Font Catalog。
2. Linux/Windows Native Picker、Parse、Size/Digest 和 Asset Store。
3. 受限 Font Protocol 与 Dynamic `@font-face`。
4. Delete/Selected Fallback、Android/iOS Unsupported。

出口：

- WebView 不接收 Path 或 Font Bytes。
- Nerd Font、Emoji、CJK 和自定义 Font Visual QA 通过。

### Milestone 4：Snippet Product Workflow

1. CRUD 与 Request-local Edit。
2. `{{variable}}` Literal Template 和 Exact Variable Set。
3. Current Session Insert/Run。
4. Multi-line Preview/Confirmation、Stale Session/Vault Lock Fail Closed。
5. Browser/Multi Tab/Native SSH Marker。

出口：

- 不调用本地 Shell、不使用 `eval`。
- 普通 Run IPC 不发送 Snippet Body。

### Milestone 5：Native QA、CI 与治理

1. Workspace、Frontend、Browser、OpenSSH、X11、Wayland、Windows。
2. Linux/Android Container Build。
3. 同 Commit CI、Screenshot、Error Log、Build Hash 和 Secret/Path Scan。
4. 更新 Threat Model、Status、Roadmap、README 和 AGENTS。
5. 接受、拒绝或替代 ADR-0020。
6. 移动本计划到 `completed/`。

出口：

- Appearance/Font/Snippet Runtime、UI、Persistence 和治理一致。

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
git diff --check
```

验收重点：

- Schema v8 Migration/Recovery。
- Theme JSON 是数据而不是代码。
- Font Path/Bytes 不进入普通 IPC。
- Mounted Terminal 原地更新。
- Snippet Summary/Body 边界、Literal Variable、Multi-line Confirmation。
- 不执行本地 Shell，不持久化 Variable Value。

## Surprises & Discoveries

- 2026-07-29：当前 Terminal Theme、Font Family、Font Size 和 Line Height 全部
  在 `TerminalPane.tsx` 构造时硬编码；实施必须增加 Update Effect，不能用 React
  `key` 重建 xterm，否则会破坏 Multi Tab Scrollback 和 Ack。
- 2026-07-29：Bundled Nerd Font 已具备 OFL License，Noto Emoji 也已进入
  Frontend Dependency；v1 可以先以这两项作为跨平台 Fallback。
- 2026-07-29：`woff2` 0.3.0 与固定 Rust 1.93.1/当前依赖图不兼容；改用
  `wuff` 0.2.8，并给 Brotli 输出提供固定 64 MiB 上限的 Allocator。
- 2026-07-29：xterm Unicode Grapheme Addon 没有公开 Ambiguous Width Setter；
  当前只在 `TerminalPane` 的单一适配函数中访问 pinned 0.4.0 Provider，等待上游
  Public API 后替换。
- 2026-07-29：Nested Weston Kiosk 下 GTK Portal File Chooser 没有可自动化的
  Accept 控件；Wayland QA 因而验证 no-`DISPLAY` Appearance Workspace、Mounted
  Terminal、Snippet 和 IBus，Linux Native Picker/Managed Font 由同一代码路径的
  X11 QA 覆盖。
- 2026-07-29：仅校验 Opaque ID/Digest 仍可能让已删除但残留的 Asset 被猜测 URL
  访问；Protocol 必须额外要求当前进程中的 Live DB Registration，Vault Lock
  清空 Registry，Repository List 同时清理无引用 Managed Asset。

## Decision Log

- 2026-07-29：Theme、Font 和 Snippet 统一采用 Typed Data，不采用可执行包。
- 2026-07-29：Snippet 普通 Run 只提交 ID/变量/Session；Rust 解析 Body 并直接
  发送到远端 PTY。
- 2026-07-29：Imported Font Binary 不是 Secret，不存 SQLCipher BLOB；使用
  Rust-owned App Asset Store、Opaque ID 和 Digest，Path 不进入 WebView。
- 2026-07-29：v1 Appearance 是 Vault-wide，不增加 Group/Host 三态继承。
- 2026-07-29：两个 Embedded Family 相同的 Imported Font 使用
  `AnySSH Imported {font_id}` 作为独立 CSS Family，避免 `@font-face` 冲突。
- 2026-07-29：Theme/Font Picker 成功后立即选中并应用；普通 IPC 仍只返回
  Metadata，Source Path 和 Binary 不进入 WebView。
- 2026-07-29：受限 Font Protocol 只服务当前 Vault Repository 已注册的
  `id + format + digest`；删除、Integrity Reconciliation 或 Vault Lock 后立即
  Fail Closed。

## Outcomes & Retrospective

计划刚开始。ADR-0020 保持 Proposed，直到 Schema v8、Mounted Terminal、
Native Font、Snippet SSH Marker 和同 Commit CI 全部完成。
