# ExecPlan 0003：系统 SSH Agent 认证

- 状态：Active
- 创建日期：2026-07-27
- 最后更新：2026-07-27
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

让 Linux 和 Windows Desktop 用户使用系统 SSH Agent 中已加载的 Key 连接
Saved Host，而无需把 Private Key 或 Passphrase 导入 AnySSH Vault。

## 范围

### 包含

- Linux `SSH_AUTH_SOCK` 与 Windows OpenSSH Agent Named Pipe。
- Rust-only Agent Identity 枚举和 Fingerprint 选择。
- SQLCipher Schema v5 `system_agent` Credential。
- DB Actor/ApplicationCore/Tauri metadata-only Commands。
- Direct、Jump 和 Target 的 System Agent Authentication。
- Credential UI、Browser QA、OpenSSH、Linux 与 Windows Native QA。
- Android/iOS 明确 Unsupported 与 Build 回归。

### 不包含

- Pageant。
- Agent Forwarding。
- 应用内 Agent。
- FIDO2/PKCS#11 的专用 UI。
- Keyboard-interactive/OTP。
- 加密 Private Key Native Passphrase Prompt。

## 上下文

Accepted ADR-0002 要求 SSH Runtime 使用 russh 而不是系统 `ssh`。Accepted
ADR-0006 要求 Private Key 和 Passphrase 不进入 WebView。Proposed ADR-0013
定义系统 Agent 只作为外部签名能力，并使用 Public Key SHA-256 Fingerprint
选择确定 Identity。

关键路径：

- `crates/anyssh-ssh/src/lib.rs`
- `crates/anyssh-storage/src/credential.rs`
- `crates/anyssh-storage/src/lib.rs`
- `crates/anyssh-storage/src/actor.rs`
- `crates/anyssh-app/src/lib.rs`
- `apps/client/src-tauri/src/lib.rs`
- `apps/client/src/lib/credential-bridge.ts`
- `apps/client/src/components/ConfigurationWorkspace.tsx`
- `scripts/test-ssh-smoke.sh`
- `scripts/qa/native-windows-smoke.ps1`

威胁与失败模式：

- WebView 不得提交 Agent Socket/Pipe Path、Public Key Blob 或签名 Payload。
- 不得无界尝试 Agent 全部 Identity。
- Identity 消失或 Agent 拒绝签名时不得自动回退到其他 Key。
- Windows Service 未启动、Linux Socket 缺失和平台不支持需要稳定错误。
- Jump Host 与 Target 的 Agent 错误必须按 Hop 归属。

## Progress

- [x] 2026-07-27：完成 Group Plan 收尾并接受 ADR-0012。
- [x] 2026-07-27：创建 ADR-0013、System Agent Design 和本 ExecPlan。
- [x] 2026-07-27：实现 Agent Connector、Identity Enumeration 和 Core
  Authentication。
- [x] 2026-07-27：实现 Schema v5、Credential Repository、
  Actor/Application/Tauri Commands。
- [x] 2026-07-27：实现 Credential UI 与 Browser QA。
- [ ] 完成 OpenSSH、Windows、Android/Linux Build 与全量回归。
- [ ] 评审 ADR-0013 并收尾本计划。

## Milestones

### Milestone 1：Agent Core

1. 平台 Agent Connector。
2. 最多 64 个 Identity 的枚举与 Fingerprint。
3. 精确 Fingerprint Authentication。
4. RSA SHA-2 Hash 协商和稳定错误。

出口：

- Linux Fake/Real Agent 单元与 OpenSSH Authentication 通过。
- Agent Frame、Key 或签名 Payload 不进入 Debug/Error。

### Milestone 2：Schema v5 与 Application Boundary

1. 增加 `CredentialKind::SystemAgent`。
2. v4 -> v5 原子 Migration 与回滚。
3. Actor/ApplicationCore CRUD/Resolve。
4. Tauri Identity List/Create Command。

出口：

- Saved Host 仍只提交 Host ID。
- Credential Summary 和 IPC 只包含 Public Metadata。

### Milestone 3：产品 UI

1. Agent Credential Editor。
2. Identity Loading/Empty/Error State。
3. Fingerprint/Algorithm/Comment 选择。
4. Desktop、Compact、Mobile 检查。

出口：

- 用户能创建 System Agent Credential 并分配给 Host/Group。

### Milestone 4：真实平台回归

1. Linux `ssh-agent` + OpenSSH Direct/Jump Smoke。
2. Windows OpenSSH Agent Service + EXE/WebView2 Smoke。
3. Android/iOS Unsupported 与 Build。
4. 全部 Workspace/Browser/Native/Container 回归。

出口：

- 同 Commit CI 通过，Evidence 人工检查。

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

## Surprises & Discoveries

- 2026-07-27：russh 0.62.4 已提供 Unix `connect_env`、Windows Named Pipe、
  Pageant、Identity Enumeration 和 `authenticate_publickey_with`；v1 只启用
  Linux Environment Socket 与 Windows OpenSSH Named Pipe。
- 2026-07-27：russh 的 Identity List 也可以包含 OpenSSH Certificate。v1 为避免
  Fingerprint/认证语义混淆只展示普通 Public Key，Certificate 留到后续专门能力。
- 2026-07-27：SQLite 不能直接修改 Credential Kind CHECK。Schema v5 在一个
  Transaction 内重建 Credentials 及其 Group/Host/Route Step 引用表，才能保持
  Foreign Key 和中断回滚。
- 2026-07-27：Windows 2025 Runner 可用系统 OpenSSH；QA 使用临时 Key、系统
  Agent Named Pipe 和高端口 standalone `sshd.exe`，不依赖 Docker。
- 2026-07-27：russh Agent Client 在 Debug Level 会记录 Identity Frame 和待签名
  Payload。Workspace 对 `log` 启用静态 `max_level_info`，编译期关闭这些依赖级
  Debug 记录，并用单元测试固定上限。

## Decision Log

- 2026-07-27：Credential 必须选择唯一 SHA-256 Fingerprint，不自动尝试全部
  Agent Identity。
- 2026-07-27：Agent Comment 只用于展示，不能作为稳定引用。
- 2026-07-27：v1 不启用 Agent Forwarding、Pageant 或应用内 Agent。
- 2026-07-27：System Agent Fingerprint 作为认证选择器进入现有 Record AEAD
  Payload；Credential Summary 不返回 Fingerprint。
- 2026-07-27：Agent Key 文件只用于 `ssh-add`/测试 Server 授权，AnySSH 启动前
  即删除；Runtime 只能请求 Agent 签名。
- 2026-07-27：不为诊断打开 russh `log::debug!`；Agent Frame 和签名 Payload
  不属于允许的日志内容。

## 本地验证证据

- Workspace Tests、Clippy、Frontend Lint/Typecheck/Vitest/Build 和 Playwright
  已通过。
- `pnpm test:ssh:smoke` 已验证真实 `ssh-agent` Direct、Password Jump -> Agent
  Target、Agent Jump -> Private Key Target，以及错误 Fingerprint Fail Closed。
- agent-browser：
  `artifacts/agent-browser/smoke-1785169032`；Desktop/Mobile Agent Credential
  截图已人工检查，Browser Error 为空。
- X11：`artifacts/native-xvfb/smoke-1785168783-1314017`；真实 Tauri UI 已从
  `SSH_AUTH_SOCK` 枚举 Identity、创建 Credential，并完成 Native Picker/SSH/
  4 MiB 回归。
- Wayland：`artifacts/native-wayland/smoke-1785169077-1339570`；无 `DISPLAY`
  的 WebKitGTK/IBus/SSH 回归通过，Marker 为 `/tmp/anyssh-ime-中文`。
- Host Android Build：
  `artifacts/android-build/build-1785169153-1344552`，APK SHA-256 为
  `124bc46dc0963bec4a972c4583b1159527b4be18cf2d6a2d4eddc086435ff5b0`。
- Linux Container：
  `artifacts/linux-build/build-1785169203-1`，ELF SHA-256 为
  `1d94cd2fde8ba2e7b148b727ca3e4a990a18560dc080f5655f4241f8cfa6fb7e`。
- Android Container：
  `artifacts/android-build/build-1785169283-1`，APK SHA-256 为
  `28950dde0621e49976a9ddee949c2fb253b574e8c1d73eee10ca00356914802f`。
- Windows PowerShell/Node QA 流程已更新；本机无 Windows，等待远端 Runner。

## Outcomes & Retrospective

尚未完成。
