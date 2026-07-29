# AnySSH 路线图

> 路线图描述阶段顺序，不替代每个阶段的 ExecPlan。

## Phase 0：技术风险验证

状态：已完成（iOS Build 因无 macOS/Xcode 环境延期并明确记录）。

目标：

- 初始化 Agent 友好的 Monorepo。
- 完成 russh + xterm.js 单 Host 垂直链路。
- 验证 SQLCipher 和 VMK 解锁。
- 验证两跳 Jump Host。
- 验证 Linux X11/Wayland、Windows、Android、iOS 的构建路径。

出口：

- Proposed ADR 根据实验结果变为 Accepted、Rejected 或 Superseded。
- 仓库拥有可重复的 build/test/lint/format 命令。
- AGENTS.md 与 CI 使用相同命令。

## Phase 1：桌面 MVP

状态：进行中；Group、System Agent、加密 Private Key 原生 Passphrase Prompt、
Known Host Repository/Durable TOFU、Keyboard-interactive/OTP 和 Multi Tab
Terminal、SSH Port Forwarding、Private Key Generation/Public Reveal/
Encrypted Export、Terminal Appearance/Font/Snippet Productization 已完成。

- Host 与 Group 继承（已完成）。
- 密码、私钥（含 Linux/Windows 加密 Key Prompt）、系统 Agent（已完成）。
- known_hosts（Endpoint-scoped Durable TOFU 已完成；OpenSSH 文件导入/导出
  后续实施）。
- Keyboard-interactive/OTP（已完成）。
- 多 Tab Terminal（已完成）。
- Jump Host（任意长度 Route 与两跳 Native/Protocol 验证已完成）。
- Local/Remote/Dynamic Forward（已完成）。
- Key 管理（Native Import 已完成；Generation/Public Reveal/Encrypted Export
  已完成）。
- Theme/Font（已完成）。
- Snippet（已完成）。
- 加密本地 Vault（PIN/SQLCipher/Record AEAD Core 已完成；Platform Slot 待实施）。

当前建议顺序：

1. 评审 Desktop MVP 剩余 Platform Slot、OpenSSH `known_hosts` Import/Export
   与 Release Packaging 优先级。
2. 项目负责人确认优先级后创建下一份 ExecPlan；在此之前不直接进入 WebDAV、
   SFTP、Runbook 或 Plugin。

## Phase 2：WebDAV E2EE

- Vault Header。
- Operation Log。
- Snapshot。
- HLC 合并。
- 冲突副本。
- 新设备恢复。
- 常见 WebDAV 服务兼容测试。

## Phase 3：Android/iOS

- 移动端导航和终端键盘。
- Keychain/Keystore/Biometric。
- 自动锁和生命周期。
- 网络切换、断线恢复。
- 应用商店发布流程。

## Phase 4：高级能力

- SFTP。
- OpenSSH Certificate。
- FIDO2/PKCS#11。
- 批量 Runbook。
- OpenSSH Config 导入。
- 更多同步 Provider。
- 可选 libghostty 终端后端。
