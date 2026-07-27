# ADR-0013：系统 SSH Agent 使用 Fingerprint 选择的外部签名身份

- 状态：Accepted
- 日期：2026-07-27
- 决策人：项目维护者

## 背景

Desktop MVP 需要使用 Linux 和 Windows 系统 SSH Agent。Agent 持有 Private
Key 并执行签名；AnySSH 不应读取、复制或持久化这些 Private Key。

一个 Agent 可能同时加载多个 Identity。若客户端无界地逐个尝试，可能触发
Server `MaxAuthTries`、泄露不必要的 Public Key 列表，并使认证结果依赖 Agent
当前顺序。

## 决策

- AnySSH 通过 russh 的 SSH Agent Protocol Client 与系统 Agent 通信，不启动
  系统 `ssh` 子进程。
- Linux v1 使用当前进程继承的 `SSH_AUTH_SOCK`。
- Windows v1 使用 OpenSSH Authentication Agent Named Pipe
  `\\.\pipe\openssh-ssh-agent`。
- Pageant、Agent Forwarding 和应用内 Agent 不属于 v1。
- Rust 可以枚举最多 64 个 Agent Identity。v1 只返回普通 Public Key Identity，
  并向 WebView 返回 Algorithm、SHA-256 Fingerprint 和经过长度/控制字符约束的
  Comment；Certificate Identity 等待后续 OpenSSH Certificate 能力。
- Comment 只用于展示；Credential 使用 SHA-256 Fingerprint 选择唯一 Identity。
- System Agent Credential 保存 Label、Username 和所选 Fingerprint；它不保存
  Private Key、Passphrase 或签名结果。
- 连接时 Rust 重新枚举 Agent，精确匹配 Fingerprint，再通过
  `authenticate_publickey_with` 委托签名。
- Agent 不可用、Identity 消失、Agent 拒绝签名或认证失败时必须 Fail Closed，
  不自动回退到其他 Identity。
- Android/iOS v1 返回明确 Unsupported；Browser QA 只使用 metadata-only 模拟。

## 备选方案

- 启动系统 `ssh`：违反 Embedded russh 决策和 Session/Host Key 边界，拒绝。
- 自动尝试 Agent 中全部 Identity：结果不确定并可能触发认证次数上限，拒绝。
- 把 Agent Private Key 导入 Vault：破坏外部签名和 Key 不离开 Agent 的边界，
  拒绝。
- 以 Comment 或列表序号标识 Identity：不稳定且可重复，拒绝。

## 后果

### 正面

- Agent Private Key 不进入 AnySSH Vault、WebView 或日志。
- Saved Host 仍通过 Credential ID 选择确定的认证方式。
- 同一个 Credential 在 Agent Identity 顺序改变后仍选择相同 Public Key。

### 代价与风险

- Agent Credential 在目标 Identity 被删除或轮换后需要用户重新选择。
- Linux Sandbox/Flatpak 访问 Agent Socket 需要后续权限适配。
- Windows OpenSSH Agent Service 可能未启用，必须提供稳定错误和修复提示。
- Agent 本身可能弹出确认或拒绝签名；AnySSH 不能绕过其策略。

## 验证

- Linux 真实 `ssh-agent` + OpenSSH Fixture 完成 Direct 与 Jump/Target 混合认证。
- Fingerprint 选择、Identity 消失、错误 Fingerprint、空 Agent 和超过 64 个
  Identity 的行为有测试。
- Windows OpenSSH Agent Named Pipe 完成真实 EXE/WebView2 认证 Smoke。
- IPC、Debug、日志、Vault 和 QA Evidence 不包含 Private Key 或签名 Payload。
- Android/iOS 编译通过并返回明确 Unsupported。

## 当前证据

- 2026-07-27：russh Core 已实现 Linux `SSH_AUTH_SOCK`、Windows OpenSSH Named
  Pipe、64 Identity 上限、Public Key Fingerprint 选择、RSA SHA-2 协商和
  Fail-Closed 错误。
- 2026-07-27：Schema v5 `system_agent` Credential、v4 -> v5 原子 Migration、
  中断回滚、Record AEAD、Actor/Application/Tauri metadata-only Command 已通过。
- 2026-07-27：Docker OpenSSH 已验证 Direct Agent、Password Jump -> Agent
  Target 和 Agent Jump -> Private Key Target；错误 Fingerprint 不回退。
- 2026-07-27：Playwright、agent-browser Desktop/Mobile、X11 真实
  `SSH_AUTH_SOCK` UI、Wayland、Android Host 和 Linux/Android Container 本地
  回归通过。
- 2026-07-27：Head `123e684c9328b87f6001a10de48e2c3bed8134e6` 的 Run
  `30287139254` 全部九个 Job 通过。Windows 真实 EXE/WebView2 通过 OpenSSH
  Agent Named Pipe 连接 standalone `sshd.exe`，创建远端 Marker，并完成
  Vault 错误 PIN、Lock/Unlock、重启恢复和明文扫描；Windows/Browser Error Log
  为空。
- 2026-07-27：X11、Wayland、Windows、Android ARM64 和 Linux Container
  Artifact 已人工检查，未发现 Private Key Header、签名 Payload、布局截断或
  Browser Runtime Error。

Linux 与 Windows Runtime、Direct/Jump OpenSSH、Schema v5、日志边界和移动端
Unsupported Build 证据已满足本决策的验收条件，因此接受 ADR-0013。
iOS 编译仍按项目既有决定等待 macOS/Xcode，不阻塞 Desktop System Agent v1。

## 相关文档

- [System SSH Agent Authentication v1](../design/system-ssh-agent-authentication-v1.md)
- [ADR-0002](0002-russh-as-default-ssh-engine.md)
- [ADR-0006](0006-secrets-stay-out-of-webview.md)
- [ADR-0010](0010-saved-host-plans-resolve-in-rust.md)
- [Phase 1 System Agent ExecPlan](../execplans/completed/0003-system-ssh-agent-authentication.md)
