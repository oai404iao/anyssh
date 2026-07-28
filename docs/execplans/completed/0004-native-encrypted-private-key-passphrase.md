# ExecPlan 0004：Native Encrypted Private Key Passphrase

- 状态：Completed
- 创建日期：2026-07-27
- 最后更新：2026-07-28
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
Accepted ADR-0006 禁止 Private Key/Passphrase 进入 WebView。ADR-0014 已根据
本计划的 Desktop Native Secure Prompt 和失败边界验证结果接受。

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
- [x] 2026-07-28：开始 Milestone 1，核验 ssh-key、GTK、Tauri Main Thread 和
  Windows Credential UI 的现有 API。
- [x] 2026-07-28：实现 Application Prompt Boundary、加密 Key 检测、取消、
  空/错误 Passphrase、三次上限和错误脱敏测试。
- [x] 2026-07-28：实现 Linux GTK Secure Prompt；本地 X11
  `smoke-1785205977-2715042` 通过错误重试、导入、明文扫描和 SSH 回归。
- [x] 2026-07-28：实现 Windows Credential UI Provider、Native Dialog QA
  Driver、加密 Key Host/SSH Marker 和重启验证。
- [x] 2026-07-28：本地 Workspace、Frontend、OpenSSH、Playwright、
  agent-browser、X11、Wayland、Android Host 与 Linux/Android Container 回归
  通过；仅待 Windows Runner 和同 Commit CI。
- [x] 2026-07-28：Head `dac51ffd079d56ab1d7f7a5837d6bf6b89b1c333`
  的 GitHub Actions Run `30325359607` 九个 Job 全部通过；人工检查 Linux、
  Windows、Browser、Android 和 Container Artifact，接受 ADR-0014 并收尾。

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

- 2026-07-28：`ssh-key 0.7.0-rc.11` 可以在不知道 Passphrase 时解析 OpenSSH
  Container，并通过 `PrivateKey::is_encrypted()` 区分加密状态；不需要依赖
  失败字符串推断。
- 2026-07-28：Tauri Linux 已使用 `gtk 0.18.2`，可直接取得 Main Window 的
  `gtk_window()`；Android Target 不应引入 GTK 依赖。
- 2026-07-28：Windows `CredUIPromptForCredentialsW` 支持 Owner HWND、
  `DO_NOT_PERSIST` 和错误 Passphrase UI，但底层调用需要隔离审计过的 Unsafe
  Wrapper。
- 2026-07-28：Linux X11 真实错误 Passphrase 后会重新创建 GTK Prompt；正确
  Passphrase 导入后，Vault 文件、截图和日志均未发现 Key Header 或测试
  Passphrase。
- 2026-07-28：Windows QA 必须在 AnySSH 进程启动后才把 Fixture Path 和
  Passphrase 放入外部 Dialog Driver 环境，避免应用进程从父环境继承测试
  Secret。
- 2026-07-28：GTK Rust Binding `0.18.2` 为 MIT，Windows Rust Binding
  `0.61.3` 为 MIT OR Apache-2.0；两者只在目标平台链接，未扩大移动端依赖。
- 2026-07-28：CI Run `30324013273` 首次 Windows 失败不是 Prompt 或 SSH
  错误；`pnpm --filter ... exec` 把 Node CWD 切到 `apps/client`，导致 QA Driver
  被错误解析为 `apps/client/scripts/...`。改为从 `import.meta.url` 解析仓库根。
- 2026-07-28：CI Run `30324346083` 已通过 Windows Rust 编译，但 Windows
  PowerShell `Add-Type` 不会把已加载 GAC Assembly 的短文件名自动解析为编译器
  Metadata Reference。QA Driver 改为传入四个实际 `Assembly.Location`。
- 2026-07-28：CI Run `30324714023` 的 Driver 已启动且 UI Automation 编译
  成功；实际 Tauri Picker 标题来自 Command 的 `Import SSH private key`，不是
  QA Driver 最初等待的 `Open SSH private key`，因此 Import 保持 `Saving…`。
- 2026-07-28：CI Run `30325041753` 已完成 Windows Native Picker、错误/正确
  Passphrase、加密 Key SSH、System Agent 和 Repository 流程；唯一失败是明文
  Evidence Scanner 在 `sshd` 仍持有重定向日志时读取文件。改为验证 Marker 后先
  停止 Fixture，再扫描已关闭的日志。

## Decision Log

- 2026-07-27：Passphrase 不进入 React/WebView，Desktop 使用进程内/系统原生
  Secure Prompt。
- 2026-07-27：v1 保存原始加密 OpenSSH Key 和 Passphrase，不在导入时转存为
  无 Passphrase Key。
- 2026-07-27：一次 Import 最多尝试三次；取消和失败不创建 Credential。
- 2026-07-28：Prompt Provider 由 `anyssh-app` 定义受限 Context；Tauri
  Provider 只在 UI Main Thread 获取平台结果，重试、验证和持久化仍由
  `ApplicationCore` 负责。
- 2026-07-28：Linux GTK 和 Windows Credential UI 依赖使用 Target-specific
  Cargo Dependency，Android/iOS 不链接 Desktop Toolkit。

## Outcomes & Retrospective

完成。

- `anyssh-ssh` 可以在不知道 Passphrase 时区分有效的加密/未加密 OpenSSH
  Container；`anyssh-app` 拥有 sanitized Prompt Context、最多三次循环、取消、
  Prompt Unavailable、通用失败和“成功后才写库”的业务边界。
- Linux 使用 Tauri/GTK Main Thread 上的进程内不可见 Entry；Windows 使用带
  Owner HWND、`DO_NOT_PERSIST` 和 Zeroizing UTF-16 Buffer 的
  `CredUIPromptForCredentialsW`。普通 Tauri IPC 仍只有 Label/Username。
- 成功导入保留原始加密 OpenSSH Key，并把 Key 和 Passphrase 分别写入既有
  Record AEAD 字段；SQLCipher Schema 仍为 v5，没有新增 Migration。
- Docker OpenSSH Integration 现在从 Native Import API 创建 Credential，再经
  Lock/Unlock、Credential ID Resolution 和 SSH Core 完成真实认证。
- 同 Commit CI `30325359607` 的关键证据：
  - agent-browser：`smoke-1785208482`。
  - X11：`smoke-1785208651-6081`，真实 GTK Picker/Prompt、错误重试、导入、
    Vault 扫描和后续 SSH/4 MiB 回归。
  - Wayland：`smoke-1785208730-7467`，无 `DISPLAY` 且 IBus 中文到达 SSH。
  - Windows：`smoke-20260728-031708-7668`，真实 Picker、两次 Credential UI、
    加密 Key SSH、System Agent SSH、错误 PIN、重启恢复和空 Browser Error Log。
  - Android ARM64 APK SHA-256：
    `4b175a2f60b32f6cfe24e2003e73280f0843460e1605e341cdd03b56aaca2640`。
  - Linux ELF SHA-256：
    `ae65290e3724aa0780dd53c0826a26111bf068db3a244cab78c6f44e3b4b0cf0`。
  - Windows EXE SHA-256：
    `5f47b35756b5ee9a202d11af2861d5b6041e05617b3d7606f003f9354effebca`。
- Artifact 二次扫描没有发现 Linux/Windows 测试 Passphrase 或
  `BEGIN OPENSSH PRIVATE KEY`；Windows `sshd` 日志分别记录了导入 Key 和
  Fingerprint-selected Agent Key 的真实 Public Key Authentication。
- Android/iOS 加密 Key Prompt 继续明确 Unsupported；Android Content URI、
  iOS Controller 生命周期、Secret Reveal/Export 和“不记住 Passphrase”模式仍
  属于后续计划。
