# ExecPlan 0004：Native Encrypted Private Key Passphrase

- 状态：Active
- 创建日期：2026-07-27
- 最后更新：2026-07-27
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

让 Linux 和 Windows Desktop 用户通过原生安全提示导入加密 OpenSSH Private
Key，而不把 Passphrase、Path 或 Key 内容交给 WebView。

## 范围

### 包含

- Rust 加密 OpenSSH Key 检测和最多三次验证。
- `anyssh-app` Native Prompt Provider Boundary。
- Linux GTK Secure Prompt。
- Windows Native Secure Prompt。
- 复用现有 Private Key/Passphrase Record AEAD。
- X11 与 Windows Native Picker/Prompt/SSH QA。
- Browser、OpenSSH、Android/Linux Container 和同 Commit CI 回归。

### 不包含

- Secret Reveal/Export。
- 每次连接重新提示或“不记住 Passphrase”。
- Android/iOS Content URI。
- PEM/PKCS#8 专用格式转换。
- Keyboard-interactive/OTP。

## 上下文

Accepted ADR-0011 已把 Native Picker、Path、文件读取和 Key Validation 留在 Rust。
Accepted ADR-0006 禁止 Private Key/Passphrase 进入 WebView。Proposed ADR-0014
定义 Desktop Native Secure Prompt 和失败边界。

关键路径：

- `crates/anyssh-app/src/lib.rs`
- `crates/anyssh-ssh/src/lib.rs`
- `crates/anyssh-storage/src/credential.rs`
- `apps/client/src-tauri/src/lib.rs`
- `scripts/qa/native-xvfb-smoke.sh`
- `scripts/qa/native-windows-smoke.ps1`

威胁与失败模式：

- WebView/IPC 不得增加 Passphrase、Path 或 Key 字段。
- 不得通过 Shell、`zenity` 或 PowerShell 子进程收集 Passphrase。
- Prompt/Toolkit Buffer、Debug/Error 和 QA Artifact 不得泄露 Secret。
- 错误 Passphrase、取消和 Prompt 初始化失败不得创建 Credential。
- Linux/Windows Prompt 必须在正确 UI Thread 运行，不能阻塞 DB Actor。

## Progress

- [x] 2026-07-27：完成 System Agent Plan，接受 ADR-0013。
- [x] 2026-07-27：创建 ADR-0014、Design 和本 ExecPlan。
- [ ] 实现 Application Prompt Boundary 与加密 Key 检测。
- [ ] 实现 Linux GTK Secure Prompt 和 X11 QA。
- [ ] 实现 Windows Secure Prompt、Native Picker 和真实 SSH QA。
- [ ] 完成全部回归、评审 ADR-0014 并收尾本计划。

## Milestones

### Milestone 1：Rust Boundary

1. 检测未加密/加密 OpenSSH Key。
2. 定义 sanitized Prompt Context 和 Provider。
3. 三次尝试、取消和通用错误。
4. 保持原始加密 Key + Passphrase 独立 Record AEAD。

出口：

- Application/Storage/SSH Tests 覆盖 Secret 生命周期和错误脱敏。
- Tauri Request 继续拒绝 `path`、`privateKey`、`passphrase`。

### Milestone 2：Linux Native Prompt

1. GTK Main Thread Modal Secure Entry。
2. Prompt Buffer 清理和取消。
3. X11 自动填写、导入和 SSH。
4. Vault/日志/截图明文扫描。

出口：

- 真实 Linux Native Picker + Prompt + OpenSSH 通过。

### Milestone 3：Windows Native Prompt

1. Windows System Secure Prompt。
2. Native Picker/Prompt 自动化。
3. 真实 EXE/WebView2 导入、重启和 OpenSSH。
4. QA-only CDP 证明无法读取 Native Prompt Secret。

出口：

- Windows Runner 通过 Native Picker + Secure Prompt + SSH。

### Milestone 4：全量回归

1. Workspace/Frontend/Browser/OpenSSH。
2. X11/Wayland/Windows。
3. Android Host 与 Linux/Android Container。
4. 同 Commit CI 和 Artifact 人工检查。

出口：

- ADR-0014 状态评审，ExecPlan 收尾。

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

尚无。

## Decision Log

- 2026-07-27：Passphrase 不进入 React/WebView，Desktop 使用进程内/系统原生
  Secure Prompt。
- 2026-07-27：v1 保存原始加密 OpenSSH Key 和 Passphrase，不在导入时转存为
  无 Passphrase Key。
- 2026-07-27：一次 Import 最多尝试三次；取消和失败不创建 Credential。

## Outcomes & Retrospective

尚未完成。
