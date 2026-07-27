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
- `direct-tcpip` + `Channel::into_stream` + `connect_stream` 两跳 Jump Host。
- Jump Host 与 Internal Target 独立 Host Key 确认、目标认证失败、握手超时、
  取消和第一跳断开测试。
- 未加密与口令保护 Ed25519 OpenSSH 私钥认证、错误口令、未授权 Key，以及
  Password Jump Host -> Private Key Target 混合认证。
- TOFU Fingerprint 重用、Host Key 轮换硬阻断且不重新弹出信任确认。
- 4 MiB 连续终端输出下队列达到 64 项上限并在恢复消费后无截断完成。
- 原生 Tauri/WebKitGTK 使用最多 8 个未确认 Chunk；xterm `write` Callback Ack 后
  才继续读取 Core Event，并在 4 MiB 输出后成功执行后续远端命令。
- 原生 Wayland 启动时移除 `DISPLAY` 并强制 `GDK_BACKEND=wayland`；Weston、
  IBus/libpinyin、xterm.js 和真实 SSH Shell 的中文组合输入链路已通过。
- Android SDK 36、JDK 17、NDK 29.0.13846066 下完成 ARM64 Debug APK 构建；
  Rust SSH、Vault 和 bundled SQLCipher 均已交叉编译进 APK。
- Linux 与 Android Build 已迁入独立 Docker Target Image；构建容器不继承宿主
  环境变量，只接收 Git 已跟踪或未忽略的工作树文件。
- GitHub Actions Windows Runner 已成功产出 x86-64 Debug EXE；Linux 与
  Android Container Build Evidence 也已由远端 Runner 验证。
- 2026-07-27 的 GitHub Actions Run `30235453657` 中全部九个 Job 通过。
- CI 工作流。

当前仓库尚未完成：

- Jump Route 持久化和产品配置 UI。
- 私钥 Credential 的 Vault/Tauri 产品集成和 SSH Agent 认证。
- Windows 原生运行验证。
- Linux 真实桌面、GPU/WebGL 回退与更多桌面环境检查。
- iOS 构建验证；当前没有 macOS/Xcode 环境，按维护者指示暂缓。
- Host/Group 持久化。

## 待项目负责人确认

1. 首个公开版本是否以 Linux + Windows 为主要交付平台。

## 已确认项目身份

- 产品和仓库名称：`AnySSH`。
- Bundle/Application ID：`com.spiredive.anyssh`。
- Android/iOS Phase 0 验收标准：成功构建，不要求模拟器或真机运行。
- 2026-07-26：由于当前没有 Mac，iOS 构建验证暂缓，不在 Linux 上使用伪交叉
  编译结果替代 Xcode Build。
- 主许可证：GNU Affero General Public License v3.0 only，
  SPDX 标识为 `AGPL-3.0-only`。
- 第三方资源继续使用各自许可证，例如内置 Nerd Font 使用 OFL-1.1。

## 当前活动计划

- [`Phase 0：技术风险验证`](../execplans/active/0001-phase-0-technical-validation.md)
