# 项目状态

> 更新日期：2026-07-27

## 当前阶段

**Phase 0 completed; Phase 1 Desktop MVP in progress**

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
- `anyssh-storage` 专用 DB Actor：Actor Thread 独占 `Option<LocalVault>`，
  使用 16 项有界 Command Queue 和 oneshot Response；Tauri 不再持有
  `LocalVault` 或使用 Vault `spawn_blocking`；Android ARM64 重新构建通过。
- SQLCipher Schema v2 Credential Repository：Password/Private Key/Passphrase
  Record AEAD、CRUD、Summary-only IPC、v1 -> v2 Migration 和中断回滚。
- SQLCipher Schema v3 Host/Jump Route Repository：CSPRNG ID、metadata-only
  CRUD、有序 Host ID Step、Credential/Route ID 引用、Restrict 删除、循环检测，
  以及旧 Host Password -> Credential 的原子迁移。
- Rust-only Saved Host Connection Plan：WebView 只提交 Target Host ID，
  DB Actor 递归展开 Route 并解析 Credential，russh 最多执行 32 个 Jump Host。
- Host、Credential 和 Jump Route 配置 UI：metadata-only 列表、引用选择、有序
  Route Builder、Restrict 删除错误和 Saved Host 打开入口。
- Rust-owned Native Private Key Import：WebView 只提交 Label/Username，原生
  Picker、文件读取、1 MiB 上限、UTF-8/OpenSSH 校验和 Vault 写入均留在 Rust；
  Xvfb 已验证真实文件选择器和导入结果。
- 隔离 OpenSSH 已验证 Password Jump 1 -> Password Jump 2 -> Private Key
  Target、三跳 Host Key 顺序和 Jump 2 认证失败归属。
- `anyssh-app` Application Service：Credential ID 经 DB Actor 解密后直接进入
  SSH Core；Docker OpenSSH 已验证加密 Private Key 在 Lock/Unlock 后成功认证。
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
- DB Actor Commit `f2fc360` 的 Run `30238710937` 也已全部通过。
- Credential Repository Commit `9f14940` 的 Run `30243415893` 全部通过。
- Host/Jump Route Repository Commit `5e366fd` 的 Run `30245997616` 全部通过。
- Saved Host 多跳 Runtime Commit `98182b3` 与原生 QA 稳定化 Commit
  `563c09e` 的 Run `30250776234` 全部通过。
- Host/Credential/Jump Route 配置 UI 与原生 Private Key Import Commit
  `780059d` 的 Run `30258051366` 全部九个 Job 通过；远端 Artifact 已人工检查
  X11 Native Picker、4 MiB 输出、Wayland/IBus 中文输入以及桌面/移动配置页面。
- Windows WebView2 Runtime Commit `99b71ec` 的 Run `30270414706` 全部九个
  Job 通过；真实 EXE 获得非零窗口句柄并完成 Vault、Repository、错误 PIN、
  Lock/Unlock 和进程重启恢复，截图及 Browser Error Log 已人工检查。
- Phase 0 Threat Model、Outcomes 和 ADR 状态评审已完成。
- Phase 1 首个计划已确定为 Group Schema v4 与 `Inherit / Set / Clear` 三态继承。
- SQLCipher Schema v4 已引入 Group Repository、32 层 Parent 限制、三态
  Override、Rust-only Effective Connection Plan 和 Group/Host 配置 UI；当前
  Schema v5 继续保留该模型。
- Group 继承的 Credential/Jump Route 已通过两跳 OpenSSH Saved Host Smoke；
  Browser、X11、Wayland/IBus 和 Workspace 回归通过。
- Group Feature Commit `ece4fe7` 的 GitHub Actions Run `30279500562` 全部
  九个 Job 通过；Windows Group/Inherited Host/Route 重启恢复、Android ARM64、
  Linux Container、X11、Wayland 和 agent-browser Evidence 已人工检查。
- ADR-0012 已接受，Group ExecPlan 已完成。
- SQLCipher Schema v5、`system_agent` Credential、Linux `SSH_AUTH_SOCK`、
  Windows OpenSSH Named Pipe Connector、64 Identity 上限和精确 Fingerprint
  选择已实现。
- Docker OpenSSH 已验证 Direct Agent、Password Jump -> Agent Target 和 Agent
  Jump -> Private Key Target；Agent Key/签名不进入 Vault 或 WebView。
- X11 原生 Tauri UI 已从真实 `SSH_AUTH_SOCK` 枚举 Identity 并创建 Credential；
  agent-browser Desktop/Mobile、Wayland、Android Host、Linux/Android Container
  本地回归通过。
- Head `123e684c9328b87f6001a10de48e2c3bed8134e6` 的 GitHub Actions Run
  `30287139254` 全部九个 Job 通过；Windows 真实 EXE/WebView2 使用 OpenSSH
  Agent Named Pipe 连接 standalone `sshd.exe` 并创建远端 Marker。X11、
  Wayland、Windows、Browser、Android 和 Linux Artifact 已人工检查。
- ADR-0013 已接受，System Agent ExecPlan 已完成。
- CI 工作流。

当前仓库尚未完成：

- 加密 Private Key 的原生 Passphrase Prompt。
- Windows Native Picker。
- Linux 真实桌面、GPU/WebGL 回退与更多桌面环境检查。
- iOS 构建验证；当前没有 macOS/Xcode 环境，按维护者指示暂缓。

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

- [`Phase 1：Native Encrypted Private Key Passphrase`](../execplans/active/0004-native-encrypted-private-key-passphrase.md)

## 已完成计划

- [`Phase 0：技术风险验证`](../execplans/completed/0001-phase-0-technical-validation.md)
- [`Phase 1：Group 持久化与三态继承`](../execplans/completed/0002-group-persistence-and-inheritance.md)
- [`Phase 1：系统 SSH Agent 认证`](../execplans/completed/0003-system-ssh-agent-authentication.md)
