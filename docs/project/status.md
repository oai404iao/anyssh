# 项目状态

> 更新日期：2026-07-26

## 当前阶段

**Phase 0 implementation in progress**

当前仓库已完成：

- 正式项目名称确认为 `AnySSH`。
- 主项目许可证确认为 `AGPL-3.0-only`。
- 初始产品构想整理。
- 2026 技术架构设计。
- ADR 候选决策拆分。
- Phase 0 技术验证计划。
- Agent 文档工作流。
- Git 仓库初始化。
- Cargo 与 pnpm Workspace。
- Tauri 2 + React + xterm.js 原型。
- russh 密码认证、Host Key 确认和交互式 PTY。
- Tauri 二进制终端输出 Channel。
- OpenSSH Docker Fixture 真实协议测试。
- Vitest、Playwright 和 agent-browser 检查。
- Linux Tauri 原生依赖容器内 `cargo check`。
- 本机 WebKitGTK 4.1 原生 `cargo check`。
- 无桌面环境下通过 Xvfb 启动 Tauri，并经原生 WebView 连接 Docker OpenSSH、
  确认 Host Key、输入远端命令和断开。
- `anyssh-vault`：随机 256-bit VMK、Argon2id PIN Slot、XChaCha20-Poly1305
  包装和 HKDF-SHA-256 子密钥。
- `anyssh-storage`：SQLCipher 4.10、版本化 Schema、Credential 字段 AEAD、
  重启解锁和明文泄漏检查。
- 原生 Vault 创建、错误 PIN、锁定和重新解锁流程。
- CI 工作流。

当前仓库尚未完成：

- Jump Host。
- 私钥和 SSH Agent 认证。
- Windows、Android 和 iOS 构建验证。
- Linux Wayland、IME 和真实桌面环境检查。
- Host/Group 持久化。

## 待项目负责人确认

1. 首个公开版本是否以 Linux + Windows 为主要交付平台。

## 已确认项目身份

- 产品和仓库名称：`AnySSH`。
- Bundle/Application ID：`com.spiredive.anyssh`。
- Android/iOS Phase 0 验收标准：成功构建，不要求模拟器或真机运行。
- 主许可证：GNU Affero General Public License v3.0 only，
  SPDX 标识为 `AGPL-3.0-only`。
- 第三方资源继续使用各自许可证，例如内置 Nerd Font 使用 OFL-1.1。

## 当前活动计划

- [`Phase 0：技术风险验证`](../execplans/active/0001-phase-0-technical-validation.md)
