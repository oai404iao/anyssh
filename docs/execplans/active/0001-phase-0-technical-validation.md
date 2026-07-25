# ExecPlan 0001：Phase 0 技术风险验证

- 状态：Active
- 创建日期：2026-07-25
- 最后更新：2026-07-25
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

在投入完整产品开发前，用最小端到端原型验证 AnySSH 的高风险技术选择：

- 同一套 Tauri 应用壳是否能覆盖目标平台。
- russh 与 xterm.js 是否能组成可用 SSH 终端。
- SQLCipher 与 VMK 多 Key Slot 是否能跨平台工作。
- Jump Host 是否能通过 SSH Channel 实现。
- 文档中的 Proposed ADR 是否有足够证据转为 Accepted。

完成后，仓库应具备一个可以持续迭代的 Monorepo、一条可运行的 SSH 垂直链路、准确的开发命令和基础 CI。

## 范围

### 包含

- 项目身份、许可证和 Bundle ID 决策。
- Cargo/pnpm/Tauri Monorepo 初始化。
- Linux/Windows 桌面应用壳。
- Android/iOS 最小构建与启动验证。
- 单 Host SSH PTY 原型。
- Host Key TOFU 与变化阻断。
- 密码和私钥认证。
- xterm.js 二进制输出、Resize 和背压。
- SQLCipher + PIN 解锁最小 Vault。
- 两跳 Jump Host。
- 基础 CI、lint、format 和 test 命令。
- Phase 0 结果驱动的 ADR 状态更新。

### 不包含

- 完整 Termius 风格 UI。
- 完整 Host/Group 管理。
- WebDAV 实际同步。
- Remote/Dynamic Forward 的完整产品 UI。
- 完整平台生物识别实现。
- 脚本、SFTP、FIDO2 和应用商店发布。

这些能力可以保留接口或测试桩，但不得扩大 Phase 0 范围。

## 上下文

仓库在本计划创建时只有文档，没有代码工程、构建脚本、CI 或 Git 元数据。

关键文档：

- [产品构想](../../project/product-brief.md)
- [项目状态](../../project/status.md)
- [总体技术设计](../../design/technical-architecture-2026.md)
- [ADR 索引](../../adr/README.md)
- [技术版本基线](../../reference/technology-baseline-2026.md)

计划中的代码布局：

```text
apps/client/                      # React + Tauri
crates/anyssh-domain/
crates/anyssh-app/
crates/anyssh-ssh/
crates/anyssh-vault/
crates/anyssh-storage/
crates/anyssh-platform/
crates/anyssh-testkit/
native/
```

Phase 0 可以只创建当前里程碑真实需要的 crate；不要创建大量没有内容的占位模块。

## Progress

- [x] 2026-07-25：整理产品构想。
- [x] 2026-07-25：完成 2026 总体技术设计。
- [x] 2026-07-25：建立 ADR、ExecPlan、Design、Reference 和 Project 文档结构。
- [x] 2026-07-25：创建根目录 AGENTS.md。
- [x] 2026-07-25：初始化 Git、Cargo Workspace、pnpm Workspace 和 Tauri Client。
- [x] 2026-07-25：建立 dev/build/test/lint/format、Playwright 和 agent-browser 命令。
- [x] 2026-07-25：完成 russh + xterm.js 密码认证单 Host 垂直原型。
- [x] 2026-07-25：使用 Docker OpenSSH Fixture 验证 Host Key、密码、PTY 和输出。
- [x] 2026-07-25：通过 agent-browser 实际验证密码显示、Host Key Dialog、终端键盘、Unicode、移动视口和 Disconnect。
- [x] 2026-07-25：在具备 WebKitGTK 4.1 的 Arch Linux 容器中完成 Tauri `cargo check`。
- [x] 2026-07-25：正式确认项目名称为 `AnySSH`，许可证为 `AGPL-3.0-only`。
- [x] 2026-07-25：正式确认 Bundle/Application ID 为 `com.spiredive.anyssh`。
- [x] 2026-07-25：本机安装 WebKitGTK 4.1 后通过原生 Tauri `cargo check`。
- [x] 2026-07-25：在无桌面环境的 Xvfb 中启动真实 Tauri/WebKitGTK，
  完成 Host Key 确认、密码认证、远端命令和 Disconnect。
- [ ] 完成 SQLCipher + PIN Slot 原型。
- [ ] 完成两跳 Jump Host 原型。
- [ ] 建立目标平台 CI/设备验证。
- [ ] 根据证据更新 ADR 状态并完成 Phase 0 报告。

## Milestones

### Milestone 0：文档与 Agent 工作流

工作：

1. 建立项目文档分类。
2. 整理总体设计。
3. 创建 Proposed ADR。
4. 创建本 ExecPlan。
5. 创建 AGENTS.md。

出口：

- 新 Agent 能从根目录找到产品目标、架构决策和当前任务。

状态：已完成。

### Milestone 1：项目身份与仓库基础

需要项目负责人确认：

1. Phase 0 的 Android/iOS 目标是仅构建，还是要求真机启动。

随后：

1. 初始化 Git。
2. 创建根 Cargo Workspace 和 pnpm Workspace。
3. 创建 `apps/client` Tauri + React 应用。
4. 固定 Rust、Node 和包管理器版本。
5. 提交 lockfile。
6. 建立最小 CI。
7. 把实际命令写入 AGENTS.md。

出口：

- 新 clone 可以使用文档中的准确命令启动、检查和构建当前平台应用。
- CI 调用与 AGENTS.md 相同的命令。

状态：项目名、许可证、Bundle ID、工程 scaffold 与 CI 已完成；Android/iOS
验收标准仍待负责人确认。

### Milestone 2：SSH Terminal 垂直链路

实现：

```text
临时 Host 配置
  -> TCP
  -> russh handshake
  -> Host Key prompt
  -> password/private key auth
  -> PTY
  -> bounded binary IPC
  -> xterm.js
```

工作：

1. 创建最小 `anyssh-ssh` crate。
2. 使用本地 OpenSSH 测试容器或隔离测试服务器。
3. 实现严格 Host Key 回调。
4. 实现密码与私钥认证。
5. 创建 PTY 和 Resize。
6. 建立 Rust 到 xterm.js 的有界二进制通道。
7. 增加取消、关闭、错误和大输出测试。
8. 验证中文、Emoji、Nerd Font 和组合字符。

出口：

- 用户可以连接测试 Host、运行命令、调整窗口并安全断开。
- Host Key 变化会阻断连接。
- 持续大量输出不会导致无限内存增长或 UI 长时间冻结。

当前结果：

- 已验证真实 OpenSSH 密码认证、Host Key 人工确认、PTY、命令输出和 Disconnect。
- 已建立有界 Rust Event Channel 和 Tauri Raw Binary Channel。
- 已通过 `pnpm qa:native:xvfb` 验证原生 WebView -> Tauri IPC -> Rust SSH Core
  -> Docker OpenSSH 的完整交互链路。
- 大输出专项基准、私钥认证和 Host Key 变化 Fixture 尚待补充。

### Milestone 3：最小加密 Vault

实现：

```text
Random VMK
  -> HKDF DB Key
  -> SQLCipher

PIN
  -> Argon2id KEK
  -> PIN Key Slot
  -> unwrap VMK
```

工作：

1. 创建 `anyssh-vault` 和 `anyssh-storage`。
2. 定义版本化 Bootstrap/Key Slot 格式。
3. 建立 PIN Slot。
4. 建立 SQLCipher 数据库。
5. 保存一个测试 Host 和 Credential。
6. 重启、解锁和读取。
7. 检查数据库、WAL、日志和错误中没有业务明文。
8. 模拟错误 PIN、损坏 Slot 和迁移中断。

出口：

- 应用重启后只能通过正确 PIN 解锁数据。
- 数据库文件中搜索不到测试 Host、用户名和密码。
- 错误和中断不会静默破坏 Vault。

### Milestone 4：两跳 Jump Host

测试拓扑：

```text
Client -> Jump Host -> Internal Target
```

工作：

1. 测试环境禁止 Client 直接访问 Internal Target。
2. 在 Jump Session 上打开 `direct-tcpip`。
3. 把 Channel 适配为下一层 russh 的 AsyncRead/AsyncWrite Stream。
4. 每一跳单独验证 Host Key 和 Credential。
5. 验证取消、超时、第一跳断开和第二跳认证失败。

出口：

- Internal Target 只能经 Jump Host 成功连接。
- 不启动系统 `ssh` 子进程。

### Milestone 5：平台与图形验证

验证：

- Linux X11。
- Linux Wayland。
- Windows WebView2。
- Android Emulator/真机。
- iOS Simulator/真机。

重点：

- WebGL 初始化和回退。
- IME/软键盘。
- 文件与 Key Store API 可接入性。
- 应用后台/前台生命周期。
- SQLCipher 构建。

出口：

- 每个平台有明确的“通过、失败或已知限制”记录。
- 失败项进入后续 ExecPlan，不以模糊描述跳过。

当前结果：

- Linux X11：在 Xvfb 虚拟显示中通过真实 Tauri 启动和 SSH 交互检查。
- Linux Wayland、真实桌面 IME、Windows、Android 和 iOS 尚待验证。

### Milestone 6：Phase 0 关闭

1. 更新每个 Proposed ADR：
   - Accepted
   - Rejected
   - Superseded
   - 保持 Proposed 并写明缺失证据
2. 更新技术基线和 AGENTS.md。
3. 将准确命令、入口和测试环境写入文档。
4. 记录性能数据和平台限制。
5. 编写 Outcomes & Retrospective。
6. 将本计划移动到 `completed/`。
7. 创建 Phase 1 ExecPlan。

## Validation

Canonical 命令已经由根目录 `package.json`、Cargo Workspace 和 `AGENTS.md`
建立；CI 调用相同脚本。新增验证路径必须同步更新这些入口。

Phase 0 最终验证必须覆盖：

### Repository

- 全新 checkout 按 README/AGENTS.md 可以启动。
- Lockfile 已提交。
- CI 与本地使用同一脚本。

### SSH

- 正确 Host Key + 密码登录成功。
- 正确 Host Key + 私钥登录成功。
- 首次 Host Key 需要确认。
- Host Key 变化必须失败。
- PTY Resize 生效。
- 大输出受有界队列约束。
- 两跳 Jump 成功。

### Vault

- 正确 PIN 解锁。
- 错误 PIN 失败。
- 重启后数据可恢复。
- 数据文件和日志无测试秘密明文。
- 损坏 Bootstrap/Slot 返回可理解错误，不覆盖原文件。

### Platform

- Linux X11/Wayland 运行记录。
- Windows 运行记录。
- Android 构建和启动记录。
- iOS 构建和启动记录。

## 测试环境建议

建立隔离 SSH 测试拓扑：

```text
ssh-target-modern
ssh-target-legacy
ssh-jump
ssh-target-internal
```

至少覆盖：

- 当前 OpenSSH。
- 一个旧算法被默认拒绝的测试目标。
- Jump 后不可直达目标。
- 大量终端输出生成器。

测试秘密只能使用专用 fixture，不得使用真实服务器和真实密钥。

## Surprises & Discoveries

- 2026-07-25：仓库开始时不是 Git 仓库，且没有任何代码或构建配置。
- 2026-07-25：本机先安装了 WebKitGTK 6.0，但 Tauri 当前依赖
  `webkit2gtk-4.1`/`javascriptcoregtk-4.1`；两套 ABI 不能互相替代。补装
  4.1 后，本机 `cargo check -p anyssh-client` 成功。
- 2026-07-25：没有桌面环境不阻止原生验证；Xvfb + XTest 可以启动真实
  Tauri/WebKitGTK 窗口，驱动密码输入、Host Key 确认、xterm 键盘和
  Disconnect，并用 Docker 内远端文件验证命令确实到达 SSH Server。
- 2026-07-25：agent-browser 能可靠检查 Browser QA UI、密码控件、Host Key Dialog、xterm 键盘输入和响应式布局；它不等于 Native Tauri SSH 协议测试，因此必须与 OpenSSH Fixture 并存。
- 2026-07-25：初次真实截图暴露了运行环境缺少 Nerd Font 的问题；已内置 JetBrains Mono Nerd Font。Emoji ZWJ 在 xterm Canvas 中仍需单独兼容验证。
- 2026-07-25：agent-browser 视频录制依赖系统 `ffmpeg`；当前环境使用逐步截图作为证据。
- 其余发现待执行过程中持续补充。

## Decision Log

- 2026-07-25：将原始想法移动到 `docs/project/product-brief.md`，总体方案移动到 `docs/design/technical-architecture-2026.md`。
- 2026-07-25：目录统一使用 `execplans` 表示 ExecPlan；活动与完成计划分开保存。
- 2026-07-25：Tauri、russh、SQLCipher 和 WebDAV Operation Log 暂时保持 Proposed，等待 Phase 0 验证。
- 2026-07-25：正式项目名称确认为 `AnySSH`，主项目许可证确认为
  `AGPL-3.0-only`。
- 2026-07-25：正式 Bundle/Application ID 确认为 `com.spiredive.anyssh`。
- 2026-07-25：Browser QA mode 只模拟终端交互，不允许打开网络；真实 SSH 行为由 Docker OpenSSH smoke 覆盖。
- 2026-07-25：任何 UI 变更除 Playwright 外，还必须运行 agent-browser 脚本并人工查看截图。

## Outcomes & Retrospective

尚未完成。

计划结束时必须记录：

- 哪些技术选择获得验证。
- 哪些 ADR 被拒绝或替代。
- 各平台真实限制。
- 性能和安全测试结果。
- Phase 1 的剩余风险。
