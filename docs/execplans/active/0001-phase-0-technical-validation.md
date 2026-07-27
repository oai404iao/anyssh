# ExecPlan 0001：Phase 0 技术风险验证

- 状态：Active
- 创建日期：2026-07-25
- 最后更新：2026-07-27
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
- Android/iOS 最小构建验证。
- 单 Host SSH PTY 原型。
- Host Key TOFU 与变化阻断。
- 密码和私钥认证。
- xterm.js 二进制输出、Resize 和背压。
- SQLCipher + PIN 解锁最小 Vault。
- 两跳 Jump Host。
- 基础 CI、lint、format 和 test 命令。
- 用户明确追加的 Host/Jump Route 持久化验证；Host/Route 只保存 ID 引用。
- 用户明确追加的 Saved Host ID 连接与任意长度 Jump Route Runtime 验证。
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
- [x] 2026-07-26：确认 Android/iOS 在 Phase 0 仅要求成功构建。
- [x] 2026-07-26：完成 SQLCipher + PIN Slot、重启解锁、Record AEAD、
  明文扫描和迁移中断原型。
- [x] 2026-07-26：完成两跳 Jump Host 原型。
- [x] 2026-07-26：完成 OpenSSH 私钥与加密私钥认证原型。
- [x] 2026-07-26：完成 Host Key 变化阻断和大输出背压验证。
- [x] 2026-07-26：完成原生 Wayland + IBus/libpinyin + SSH Terminal IME 验证。
- [x] 2026-07-26：完成 Android ARM64 Debug APK 与 bundled SQLCipher 构建。
- [x] 2026-07-26：建立独立 Linux/Android Docker Build Image 与 Evidence 导出。
- [x] 2026-07-27：GitHub Actions Windows Runner 成功产出 x86-64 Debug EXE。
- [x] 2026-07-27：首次远端 CI 验证 Frontend、Rust、OpenSSH、Browser、
  Windows、Linux Container 和 Android Container Job。
- [x] 2026-07-27：远端 Run `30235453657` 的全部九个 CI Job 通过，包括
  原生 X11、Wayland/IBus、Windows、Linux Container 和 Android Container。
- [x] 2026-07-27：将 Tauri `VaultManager` 迁移到 `anyssh-storage` 专用
  DB Actor；使用有界 Command Queue、oneshot Response，并由 Actor 独占
  `Option<LocalVault>`；远端 Run `30238710937` 的九个 CI Job 全部通过。
- [x] 2026-07-27：实现 SQLCipher Schema v2 Credential Repository Commands，
  增加 `anyssh-app` Application Service，并验证 Private Key 只通过 Credential ID
  在 Rust 内部进入 SSH Core；远端 Run `30243415893` 的九个 CI Job 全部通过。
- [x] 2026-07-27：实现 Schema v3 Host/Jump Route Repository、v2 数据迁移、
  引用占用和循环检测，并开放 metadata-only Actor/Application/Tauri Commands。
- [x] 2026-07-27：Schema v3 通过 Workspace、Clippy、OpenSSH、Playwright、
  agent-browser、原生 X11、原生 Wayland/IBus 和 Android ARM64 本地回归；
  远端 Run `30245997616` 的九个 CI Job 全部通过。
- [x] 2026-07-27：实现 Rust-only Saved Host Connection Plan、
  `ssh_connect_saved_host` IPC 和最多 32 跳的 russh Jump Route Runtime。
- [x] 2026-07-27：Saved Host Runtime 通过 Workspace、Clippy、两 Jump
  OpenSSH、Playwright、agent-browser、原生 X11、Wayland/IBus 和 Android ARM64
  本地回归。
- [ ] Windows 运行证据仍待补充。
- [ ] iOS Build 因当前没有 macOS/Xcode 环境暂缓。
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

项目身份和 Phase 0 移动端验收标准均已确认。

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

状态：已完成。

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

- 已验证真实 OpenSSH 密码认证、未加密/口令保护 Ed25519 私钥、Host Key
  人工确认、PTY、命令输出和 Disconnect。
- 已验证错误私钥口令、未授权 Private Key，以及 Password Jump Host ->
  Private Key Target 混合认证。
- 已建立有界 Rust Event Channel 和 Tauri Raw Binary Channel。
- 已验证已保存 Fingerprint 匹配时不重复提示，Fixture 轮换 Host Key 后直接
  阻断且不允许重新 TOFU。
- 4 MiB 连续输出会填满 64 项 Event Queue；停止消费时 SSH Window Flow Control
  产生背压，恢复消费后输出与结束标记完整到达。
- Tauri 到 WebView 额外限制最多 8 个未确认 Binary Chunk；xterm `write`
  Callback 通过 `ssh_ack_output` 归还额度。原生 Xvfb 已验证 4 MiB 输出完成后
  仍能创建后续远端 Marker 并正常 Disconnect。
- 已通过 `pnpm qa:native:xvfb` 验证原生 WebView -> Tauri IPC -> Rust SSH Core
  -> Docker OpenSSH 的完整交互链路。

状态：已完成。

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

当前结果：

- 已创建 `anyssh-vault` 和 `anyssh-storage`。
- Bootstrap 只包含格式版本、随机 Vault/Slot ID、Argon2id 参数和加密 VMK。
- SQLCipher 4.10.0 community 与 XChaCha20-Poly1305 Credential 字段加密通过。
- 正确 PIN、错误 PIN、损坏 Slot、重启解锁和迁移中断回滚测试通过。
- 原生 Xvfb 流程已验证创建、锁定、错误 PIN、重新解锁和后续 SSH Session。
- 数据库、WAL、Sidecar、Bootstrap 中未检出测试 Host、用户名、密码、PIN 或
  `SQLite format 3` Header。
- `anyssh-storage` 专用 DB Actor Thread 现独占 `Option<LocalVault>`；
  Cloneable Handle 使用容量 16 的有界 Tokio `mpsc` Command Queue 和 oneshot
  Response。Tauri Vault Command 只做 IPC 转换，不再使用 `spawn_blocking` 或
  `Mutex<Option<LocalVault>>`。
- Actor 单元测试覆盖 Queue Backpressure、Create/Lock/Wrong-PIN/Unlock 顺序、
  Shutdown 和不可用状态。原生 X11 与 Wayland QA 均通过现有 Vault 创建链路；
  Android ARM64 Debug APK 也已重新构建。Private Key 原生导入 UI 仍留在
  下一步骤。
- Schema v2 已增加独立 Credential Repository。Actor Commands 覆盖
  Create/Update/List/Delete/Resolve；Tauri 只暴露 Password CRUD 和
  Summary-only List/Delete，不暴露 Private Key Import。
- `anyssh-app` 负责把 Credential ID 解析为 Rust-only `ResolvedCredential`，
  再直接 move 到 SSH Core。Docker OpenSSH 已验证加密 Private Key 在 Vault
  Lock/Unlock 后通过 ID 成功认证。
- Schema v3 已增加 Host/Jump Route Repository。旧 Host Password 在单个
  `IMMEDIATE` Transaction 中转为 Credential；新 Host 只保存 Credential/Route
  ID，Route 只保存有序 Host ID。Actor、Application Core 和 Tauri IPC 均已接入。
- DB Actor 现可从 Target Host ID 递归生成 Rust-only Connection Plan；缺失
  Credential、重复 Host 和超过 32 跳会在网络连接前失败。Application Core 把
  Plan 转换为 SSH Config，Tauri Saved Host IPC 不接收 endpoint 或 Credential。

状态：已完成；iOS SQLCipher 构建仍等待 macOS/Xcode 环境。

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

当前结果：

- Docker Fixture 使用独立 Edge/Internal 网络；Internal Target 不发布端口，
  Client 无法解析目标别名，且 Target 只允许来自 Jump 容器内部地址的认证，
  直接使用容器 IP 登录同样失败。
- Jump Host 通过 `channel_open_direct_tcpip` 打开目标 Channel，再使用 russh
  `Channel::into_stream()` 和 `client::connect_stream()` 建立第二层 SSH。
- Jump Host 与 Target 使用独立运行时 Host Key、请求 ID、Hop 和 Endpoint 确认。
- 已验证 Internal Target PTY 命令、取消 Host Key 等待、Target 密码错误、
  Target 握手超时和第一跳容器终止。
- 测试路径只启动 Fixture 容器和当前 Rust 测试进程，不调用系统 `ssh` 客户端。
- 显式 endpoint Tauri IPC 仍接受可选单 Jump Host；Saved Host IPC 可执行最多
  32 跳。Phase 0 表单尚未暴露配置 UI。

状态：已完成；有序 Jump Route 持久化和最多 32 跳的 Runtime 已实现，产品 UI
留待后续阶段。

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
- Linux Wayland：Weston 嵌套在 Xvfb 仅承担自动化输入和截图；AnySSH 进程移除
  `DISPLAY`，强制 `GDK_BACKEND=wayland`，并通过真实 Wayland Socket 启动。
- Linux IME：IBus/libpinyin 在 xterm.js 中提交 `中文`，经 Tauri IPC 和 Rust
  SSH Core 到达 Docker OpenSSH，并创建以 `/tmp/anyssh-ime-中文` 开头的远端
  UTF-8 文件 Marker，同时记录精确文件名 Byte。
- Android：JDK 17、SDK/Target SDK 36、Build Tools 35.0.0、
  NDK 29.0.13846066 下成功产出 ARM64 Debug APK；APK 包含
  `arm64-v8a/libanyssh_client_lib.so` 和 bundled SQLCipher。
- Build Isolation：`infra/build/Dockerfile` 使用独立 `linux`/`android` Target；
  容器从 Git 已跟踪和未忽略的文件生成隔离工作树，不继承宿主 Token，并仅复制
  `artifacts/linux-build` 或 `artifacts/android-build` 回仓库。
- Windows：GitHub Actions `windows-2025` Runner 已成功产出 33,130,496 Byte
  的 x86-64 PE32+ Debug EXE；SHA-256 为
  `83b3d4510a0084a371fa54bce87bb72301dc6b28be97650415335983a4f70c60`。
  该证据只证明 MSVC/WebView2 Application Shell 可链接，运行验证仍待补充。
- iOS：维护者当前没有 Mac，Build 验证暂缓；Linux 结果不得冒充 Xcode Build。

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
- Android 构建记录。
- iOS 构建记录暂缓，直到可用的 macOS/Xcode 环境。

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
- 2026-07-26：`rusqlite 0.40.x` 对应的 `libsqlite3-sys 0.38.x` 构建脚本使用
  Rust 1.93 尚未稳定的 `cfg_select`；Phase 0 暂时固定
  `rusqlite 0.39.x`/`libsqlite3-sys 0.37.x`。
- 2026-07-26：bundled SQLCipher 在 Linux 报告版本
  `4.10.0 community`；原生 Xvfb 截图确认 Lock/Unlock UI 可用。
- 2026-07-26：Tauri/WebKitGTK 进程还会间接加载系统 `libsqlite3.so`，同时
  AnySSH 可执行文件包含 bundled SQLCipher 的 SQLite 符号。当前运行测试通过，
  但符号可见性和跨平台共存仍需在接受 ADR-0003 前专项验证。
- 2026-07-26：russh 0.62.4 已提供 `Channel::into_stream()`，无需 AnySSH 自行
  实现底层 `AsyncRead + AsyncWrite` 状态机；仍需在 `anyssh-ssh` 中封装生命周期、
  超时、取消和逐跳 Host Key。
- 2026-07-26：Serde 的 enum `rename_all` 未重命名 struct variant 内的
  `request_id`/`fingerprint_sha256` 字段，原生 Xvfb 暴露了空指纹和缺失
  `requestId`。现已为 IPC 字段显式重命名并增加序列化回归测试。
- 2026-07-26：OpenSSH 私钥解码是同步操作，加密 Key 还可能执行较重的 KDF；
  AnySSH 将 `decode_secret_key` 放入 `spawn_blocking`，避免阻塞 Tokio Worker。
- 2026-07-26：Docker 容器使用随机 Host Port 时，`docker restart` 后 Host Port
  可能重新分配。Host Key 变化 Fixture 改为在容器内生成新 Key 并向 sshd 发送
  `SIGHUP`，保持 Endpoint 不变。
- 2026-07-26：Tauri CLI 2.11.4 的 Android 模板使用 SDK/Target SDK 36，并固定
  NDK 29.0.13846066。Android ARM64 构建证明 bundled SQLCipher 可与 Tauri、
  russh 和当前 Rust Core 一起交叉编译。
- 2026-07-26：Arch 的 Android Command-line Tools 安装在 root-owned
  `/opt/android-sdk`，普通用户无法安装 SDK Component；本地验证改用可写的
  `$HOME/Android/Sdk`。
- 2026-07-26：Wayland 自动化可把 Weston 作为 Xvfb 中的嵌套 Compositor，
  同时从 AnySSH 进程环境移除 `DISPLAY`。这样既能使用 XTest 注入输入和截图，
  又能确保 GTK/WebKitGTK 不会回退到 X11。
- 2026-07-26：QA 初版曾准备记录完整进程环境用于证明 Wayland Backend，这会
  把与测试无关的宿主 Token 带入 Evidence。实现已改为清空 Session Environment，
  并只保存 `GDK_BACKEND`、`GTK_IM_MODULE`、`WAYLAND_DISPLAY` 及必要的
  `XDG_*` Wayland Session 白名单字段。
- 2026-07-26：Android Container 首次构建暴露 `openssl-src` 会寻找
  `aarch64-linux-android-ranlib`，而 NDK 仅提供 `llvm-ranlib`。Android Check
  现显式设置 Target `AR`/`RANLIB` 并把 NDK LLVM Toolchain 加入 `PATH`。
- 2026-07-26：Tauri Android 模板生成的 Gradle Wrapper JAR 来自较早的官方
  Wrapper 版本。仓库将其替换为 Gradle 8.14.3 的官方 Wrapper JAR，并同时固定
  Wrapper JAR 与 Distribution SHA-256，避免只校验下载的 Distribution。
- 2026-07-26：独立 Build Image 与 Debug Compiler Cache 体积较大，尤其 Android
  NDK Image。缓存按平台隔离到 `~/.cache/anyssh-build/`，需要在开发文档中明确
  清理路径，不能把这些缓存纳入仓库或构建证据。
- 2026-07-27：首次 GitHub Actions Run `30227796601` 中，九个 Job 仅
  `native-linux-check` 的 Wayland Step 失败，其余均通过，包括 Windows、
  Android Container、Linux Container 和完整 OpenSSH Smoke。
- 2026-07-27：Wayland QA 的隔离 Session 修改 `XDG_DATA_HOME` 后，
  `pnpm dev` 会认为现有 `node_modules` 来自另一个 pnpm Store，并在无 TTY
  环境拒绝自动重装。QA 改为直接启动已安装的 Vite Binary，避免包管理器状态
  检查污染隔离的应用数据目录。
- 2026-07-27：GitHub Artifact 上传曾遇到 XDG Runtime Socket 的
  `ENTRYNOTSUPPORTED` 警告。Wayland QA 退出时现删除 XDG Cache、Config、
  Data 和 Runtime 临时目录，仅保留白名单 Evidence。
- 2026-07-27：Ubuntu 24.04 的 `ibus-libpinyin 1.15.7` 在 XTest 自动选词并立即
  切换 Engine 时会把末字再次提交，产生合法 UTF-8 `中文文`；Arch 环境提交
  `中文`。QA 不再把发行版相关的候选后缀当作 AnySSH 编码错误，而是验证远端
  文件名具有 `中文` UTF-8 前缀并保存完整 Byte Evidence。
- 2026-07-27：GitHub Actions Run `30235453657` 在 Commit `91641e4` 上九个
  Job 全部通过；Wayland Artifact 包含 CJK 可读截图、远端 UTF-8 Byte、Backend
  Environment 白名单和 Disconnect Evidence。
- 2026-07-27：GitHub Runner 会提示部分 `@v4` Action 仍以 Node 20 为目标并被
  强制运行在 Node 24。当前不影响验证结果，但后续应核验并升级到官方
  Node 24 Native Action Major。
- 2026-07-27：DB Actor Handle 的最后一个 Clone 释放时需要先关闭 Command
  Sender，再 Join 专用线程；否则 Actor 会在 `blocking_recv` 等待并造成退出
  Deadlock。实现使用单个 `Arc<Inner>` 显式保证这一销毁顺序，Vault 本身不进入
  Mutex。
- 2026-07-27：Serde Tagged Enum 的 `rename_all` 不负责保护额外 Secret 字段。
  SSH Authentication IPC 使用 `deny_unknown_fields`，并对 `credentialId`
  显式 Rename；携带 `privateKey` 的 Payload 序列化回归测试确认会被拒绝。
- 2026-07-27：旧 Host Password 使用 v1 Host AAD，而新 Credential 使用 v2
  Credential AAD，因此 Schema v2 -> v3 不能只复制 Ciphertext。Migration 先在
  Actor-owned Vault 中解密并重新加密，再把 Schema 切换、Credential/Host 插入和
  `user_version = 3` 放在同一个 `IMMEDIATE` Transaction；中断测试确认完整回滚。
- 2026-07-27：Schema v3 本地证据包括 Xvfb
  `artifacts/native-xvfb/smoke-1785136617-620576`、Wayland
  `artifacts/native-wayland/smoke-1785136671-622298`、agent-browser
  `artifacts/agent-browser/smoke-1785136586` 和 Android
  `artifacts/android-build/build-1785136726-623481`。Android APK SHA-256 为
  `635fb9ce105dc7ff073a0a967a4b83d1a6716243c8758b278a4369da46c744ae`。
- 2026-07-27：Commit `5e366fd` 的 GitHub Actions Run `30245997616` 九个 Job
  全部通过。Rust Core Log 明确执行 Schema v3 Migration、Host/Route 引用完整性
  和 Actor Repository 测试；Windows、Android/Linux Container、原生 X11/
  Wayland、OpenSSH 和浏览器路径同时通过。
- 2026-07-27：russh `Channel::into_stream()` 不借用上游 Handle，但上游
  Session 必须保持存活。任意长度 Runtime 使用 `Vec<Handle<_>>` 持有全部 Jump，
  Target 结束后逆序 Disconnect；当前三跳真实协议测试通过。
- 2026-07-27：单个 Route 内 Host ID 唯一仍不足以保证递归展开唯一。例如
  `Target -> [Jump 1, Jump 2]` 且 `Jump 2 -> [Jump 1]` 会重复。Connection Plan
  因此在展开时另行维护 emitted set，并在网络连接前拒绝重复或超过 32 跳。
- 2026-07-27：Saved Host 本地证据包括 agent-browser
  `artifacts/agent-browser/smoke-1785140189`、Xvfb
  `artifacts/native-xvfb/smoke-1785140219-681840`、Wayland
  `artifacts/native-wayland/smoke-1785140269-683942` 和 Android
  `artifacts/android-build/build-1785140209-679737`。Android APK SHA-256 为
  `ea6f1d68bb6251827d32251465027f633c6cf1cdc487e0be5bc97a2d2e571ad8`。
- 2026-07-27：Commit `98182b3` 的 Run `30249598725` 中，OpenSSH 多跳、
  Rust Core、Frontend、E2E 和 agent-browser 通过，但原生 X11 在 Vault 创建时
  失败。Artifact 截图显示 Ubuntu WebKitGTK 布局把 Confirm PIN 输入框上移，
  硬编码 `y=522` 点击落在输入框下方，确认 PIN 为空。QA 改为在共同有效的第一
  输入框坐标聚焦，再使用 Tab/Enter 完成表单；本地完整 Xvfb 回归重新通过。
- 2026-07-27：第一次稳定化提交的 Run `30249990549` 暴露了另一个独立时序：
  X11 窗口已经 mapped，但 `01-vault-create.bmp` 与
  `02-vault-pin-entered.bmp` 仍是逐像素黑屏，约 15 秒后的失败截图才显示 React
  Vault Gate。X11 Driver 的 `probe` 因此增加目标窗口中心像素检查，只有 WebView
  已实际绘制非黑内容才报告 ready；Wayland Vault 输入也同步改用 Tab/Enter。
- 2026-07-27：Commit `9f14940` 的 GitHub Actions Run `30243415893` 九个 Job
  全部通过。OpenSSH Log 明确执行
  `encrypted_private_key_flows_from_credential_id_to_ssh_core`，Windows、Android、
  Linux Container、原生 X11/Wayland、浏览器和 Rust Core 同时通过。
- 2026-07-27：Commit `f2fc360` 的 GitHub Actions Run `30238710937` 九个 Job
  全部通过，包含 Windows Build、Android/Linux Container、原生 X11/Wayland
  Vault QA、OpenSSH、浏览器和 Rust Core。
- 其余发现待执行过程中持续补充。

## Decision Log

- 2026-07-25：将原始想法移动到 `docs/project/product-brief.md`，总体方案移动到 `docs/design/technical-architecture-2026.md`。
- 2026-07-25：目录统一使用 `execplans` 表示 ExecPlan；活动与完成计划分开保存。
- 2026-07-25：Tauri、russh、SQLCipher 和 WebDAV Operation Log 暂时保持 Proposed，等待 Phase 0 验证。
- 2026-07-25：正式项目名称确认为 `AnySSH`，主项目许可证确认为
  `AGPL-3.0-only`。
- 2026-07-25：正式 Bundle/Application ID 确认为 `com.spiredive.anyssh`。
- 2026-07-26：Android/iOS Phase 0 只要求成功构建，不要求模拟器或真机运行。
- 2026-07-25：Browser QA mode 只模拟终端交互，不允许打开网络；真实 SSH 行为由 Docker OpenSSH smoke 覆盖。
- 2026-07-25：任何 UI 变更除 Playwright 外，还必须运行 agent-browser 脚本并人工查看截图。
- 2026-07-26：Phase 0 Jump Host 使用 russh 内置 `Channel::into_stream()` 接入
  下一层 `client::connect_stream()`；Host Key 决策必须携带 Request ID，防止
  延迟或重复操作被后续 Hop 消费。
- 2026-07-26：SSH Core 使用统一的 `SessionAuthentication` 和
  `SshConnectionConfig` 表达逐跳密码/私钥 Credential。原始私钥暂不加入
  WebView/Tauri IPC；产品集成必须从 Rust Vault 按 Credential ID 解密后直接交给
  SSH Core。
- 2026-07-26：已保存 Host Key 使用 `HostKeyPolicy::RequireSha256`；Fingerprint
  不匹配时直接返回 changed-key 错误，不再提供“信任本次”路径。
- 2026-07-26：Tauri Binary Channel 本身不提供 xterm 消费完成语义，因此增加
  8-Chunk Credit Window；只有 xterm `write` Callback 返回后 WebView 才发送
  `ssh_ack_output`，额度耗尽会停止读取 Core Event。
- 2026-07-26：Android Phase 0 固定验证 ARM64 Debug APK，不要求 Emulator 或
  真机；生成的 `src-tauri/gen/android` Source 纳入版本控制，Gradle Build
  Output 继续由其局部 `.gitignore` 排除。
- 2026-07-26：维护者当前没有 Mac，iOS Build 暂缓。Phase 0 保留缺失证据，
  不在 Linux 上宣称 iOS 已验证。
- 2026-07-26：任何 QA Evidence 禁止转储完整宿主进程环境；采用最小白名单。
- 2026-07-26：Linux 与 Android Build 使用独立 Docker Target Image；Windows
  保留原生 Windows Runner，因为 Linux Cross Build 不能替代 MSVC/WebView2
  验证；iOS 仍必须等待 macOS/Xcode。
- 2026-07-27：Wayland QA 的 Vite Server 直接使用 Workspace 已安装 Binary，
  不在清空宿主环境后的 Session 内重新调用 pnpm；应用进程仍使用隔离的 XDG
  路径，保证 Vault 和 IBus Evidence 不进入宿主用户目录。
- 2026-07-27：DB Actor 位于 `anyssh-storage`，在专用 OS 线程中顺序处理同步
  SQLCipher、Argon2id 和文件系统操作。Tauri 只持有 Cloneable Handle；Handle
  使用有界 Tokio `mpsc` 施加背压，每条 Command 使用 `oneshot` 返回结果。
  `Option<LocalVault>` 不跨线程或 IPC，只由 Actor State 持有。
- 2026-07-27：Credential Repository 使用 Schema v2 独立 `credentials` 表。
  WebView 只能获取非敏感 Summary，并以 Credential ID 发起连接。Private Key
  Create/Update 暂时只提供 Rust Trusted API；不增加接收 Key 文本或任意文件路径
  的 Tauri Command。`anyssh-app` 负责 Resolve ID 并直接构造 SSH
  `SessionAuthentication`。
- 2026-07-27：根据用户明确优先级，在 Phase 0 关闭前追加 Host/Jump Route
  持久化。Schema v3 将旧 Host 内嵌 Password 原子迁移为 Credential；新 Host
  只保存 Credential/Route ID，Route Step 只保存 Host ID，并使用 Restrict 删除
  与全图循环检测。
- 2026-07-27：Saved Host 连接只允许 WebView 提交 Target Host ID 和 Terminal
  Size。DB Actor 原子展开 Route 并解析 Credential，Application Core 转换为
  `Vec<SshConnectionConfig>`；现有显式 endpoint IPC 仅保留给 Phase 0 QA。
- 2026-07-27：递归 Route 使用 DFS 后序展开，先加入 Step Host 自身的上游
  Route，再加入 Step Host；最大展开长度与持久化单 Route 上限统一为 32。

## Outcomes & Retrospective

尚未完成。

计划结束时必须记录：

- 哪些技术选择获得验证。
- 哪些 ADR 被拒绝或替代。
- 各平台真实限制。
- 性能和安全测试结果。
- Phase 1 的剩余风险。
