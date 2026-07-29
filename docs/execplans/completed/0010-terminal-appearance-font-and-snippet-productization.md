# ExecPlan 0010：Terminal Appearance, Font, and Snippet Productization

- 状态：Completed
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
- [x] 完成 Milestone 5：Native QA、CI 与治理。
- [x] 2026-07-29：Frontend/Vitest/Playwright/Browser QA、Rust Workspace、
  OpenSSH Smoke、Linux X11、Wayland/IBus、Linux Container 和 Android ARM64
  Container Build 已通过。
- [x] 2026-07-29：Windows Native Picker/WebView2/Restart Evidence 通过。
  Native Theme/Font Picker、受限 Font Protocol、Mounted xterm、Snippet SSH
  Marker、重启恢复和 Browser Error Log 均由真实 EXE 验证。
- [x] 2026-07-29：Head
  `471bbd6f6dc54ebf3d78330cc99c86674aaedd62` 的 GitHub Actions Run
  `30457692061` 九个 Job 全部通过。最终 Evidence 为：
  - Browser `smoke-1785332929`
  - X11 `smoke-1785333012-5856`
  - Wayland `smoke-1785333277-9106`
  - Windows `smoke-20260729-135106-1056`
  - Linux Build `build-1785333255-1`
  - Android Build `build-1785333496-1`
- [x] 2026-07-29：最终 CI Build SHA-256：
  - Linux `anyssh-client`：
    `c777b95d36220629e623841863fc8c71c13b6d42efd5733548841eecf4012b9b`
  - Android `AnySSH-arm64-debug.apk`：
    `53a219617d9284bb3084706d9a13ea55c74ca89f82c4ad03aa22a5428a651f95`
  - Windows `anyssh-client.exe`：
    `04496523aae3835a6a0c0e36e298faca5eb7550262a2336909f5337070b810d6`
- [x] 2026-07-29：人工检查 Browser Desktop/Mobile、X11、Wayland 和 Windows
  Appearance、Imported Font、Snippet Preview/Output 与 Restart Screenshot。
  Error Log 为空，Font Path/Bytes、Snippet Body/Variable 和测试 Secret 扫描通过。
- [x] 2026-07-29：ADR-0020 接受，本计划移动到 `completed/`。

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
- 2026-07-29：Tabby、Electerm 和 Wave 等 Web Renderer 客户端主要使用系统已
  安装字体或随应用同源打包的字体；Termora、WezTerm 等 Native Renderer 则在
  WebView 外枚举、定位和塑形字体。AnySSH 的 Vault-scoped Managed Font Import
  不能照搬系统安装方案，因此继续保留 Rust-owned Asset Store 和受限 Protocol。
- 2026-07-29：`@xterm/addon-ligatures` 在 Chromium 暴露 Local Font Access API
  时会尝试申请 `local-fonts` Permission；WebView2 无用户激活会产生
  `SecurityError`。Terminal Adapter 在该 API 存在时改用 pinned Addon 的有界
  Fallback Ligature 集和 xterm Character Joiner，不再隐式申请权限。
- 2026-07-29：Windows Imported Font 最终失败是 QA False Negative，不是
  Protocol 或 `FontFace` 失败。Select Option Value 为 `imported:{font_id}`，
  实际 CSS Family 为 `AnySSH Imported {font_id}`；测试误把前缀当作 ID，导致
  `document.fonts` 永远匹配不到。规范化 Option Value 后真实 WebView2 流程通过。
- 2026-07-29：把 Wry Windows Custom Protocol 临时切到 HTTPS 没有改善上述
  False Negative，且不是必要条件；最终恢复并验证默认
  `http://anyssh-font.localhost` 映射。

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
- 2026-07-29：Ligature 支持不得依赖 WebView Local Font Access Permission。
  不暴露该 API 的 Runtime 使用官方 Addon；暴露 Permission-gated API 的 Chromium/
  WebView2 使用等价的有界 Fallback Character Joiner。
- 2026-07-29：Windows/Android 保持 Wry 默认 HTTP Custom Protocol 映射；
  Linux 使用 `anyssh-font://localhost`。两者都只接受 Opaque ID、Digest 和受控
  Format，不回退到 File URL、Path IPC 或系统级 Font 安装。
- 2026-07-29：同 Commit CI 通过后接受 ADR-0020。

## Outcomes & Retrospective

Terminal Appearance、Font 和 Snippet Productization 已完成。Schema v8、
Vault-wide Appearance、Strict Theme JSON、System/Imported Font、Managed Asset
Integrity、Mounted xterm 原地更新、Snippet Record AEAD、Literal Variable、
Multi-line Confirmation 和 selected SSH PTY Run 均由本地与同 Commit CI 验证。

Head `471bbd6f6dc54ebf3d78330cc99c86674aaedd62` 的 Run `30457692061`
九个 Job 全绿。Browser、X11、Wayland、Windows、Linux 和 Android Artifact 已
人工检查；Windows WebView2 已证明 Imported Font 真正进入 `document.fonts`、
更新 Mounted Terminal 并跨进程重启恢复。Ligature 路径不再依赖隐式
Local Font Access Permission。ADR-0020 因此从 Proposed 变为 Accepted。

Android/iOS Custom Font Picker、Per-Host/Group Appearance、Secret Variable、
Runbook、Plugin 和远程 Font 继续留在后续范围；iOS 仍等待 macOS/Xcode 环境。
