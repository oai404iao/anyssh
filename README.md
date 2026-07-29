# AnySSH

AnySSH 是一个已完成 Phase 0 技术验证、正在进入 Phase 1 Desktop MVP 的开源
跨平台 SSH 客户端项目。

项目名称已确定为 **AnySSH**。除单独标注许可证的第三方资源外，项目采用
[GNU Affero General Public License v3.0 only](LICENSE)。

正式应用 Identifier / Bundle ID 为 `com.spiredive.anyssh`。

目标平台：

- Linux（Wayland/X11）
- Windows
- Android
- iOS

## 当前状态

仓库当前已经包含：

- Tauri 2 + React + xterm.js 应用原型。
- Rust Workspace 与 russh SSH Core。
- Host Key 确认、密码认证、PTY、输入、Resize 和 Disconnect。
- 基于 `direct-tcpip` + 嵌套 russh Transport 的两跳 Jump Host Core。
- 未加密/口令保护 OpenSSH 私钥认证，以及密码 Jump + 私钥 Target 混合路由。
- Endpoint-scoped Known Host Repository、Durable TOFU、重启免提示、原生
  Forget 和 Changed-Key 硬阻断。
- RFC 4256 Keyboard-interactive/OTP、多轮 Challenge/Response、Partial-success
  第二因子和 Session-bound Response。
- 最多 8 个独立 Session Tab；每 Tab 隔离 Rust Session ID、Generation、
  xterm.js、Status、Resize、Host Key 和 Authentication Challenge。Inactive
  Terminal 保持 Mounted 并继续 Ack Output，Disconnect 保留 Scrollback，Close
  只移除目标 Tab。
- 4 MiB 终端输出下的 64 项 Core Queue、8 项 WebView Window 和 xterm Ack 背压。
- 随机 VMK、Argon2id PIN Slot、HKDF 子密钥和版本化 Bootstrap。
- SQLCipher 4.10 整库加密与 XChaCha20-Poly1305 Credential 字段加密。
- 专用 DB Actor Thread、16 项有界 Command Queue 和 oneshot Response。
- Schema v7 Credential/Group/Host/Jump Route/Known Host Repository；Group 与 Host 使用显式
  `Inherit / Set / Clear` 引用状态，Route 只保存有序 Host ID。
- Group Parent Chain 最多 32 层；有效 Credential/Route 在 DB Actor 内解析，
  WebView 只获得 metadata-only Summary。
- Saved Host 连接 IPC 只提交 Host ID；DB Actor 在 Rust 内解析 Credential 和
  Route，SSH Core 最多执行 32 个有序 Jump Host。
- Schema v2 Credential Repository，以及 Credential ID -> Vault -> SSH Core
  的 Rust-only Private Key/System Agent/Keyboard-interactive 路径。
- Linux `SSH_AUTH_SOCK` 和 Windows OpenSSH Agent Named Pipe 的
  Fingerprint-selected 外部签名；Agent Private Key 不进入 Vault/WebView。
- Group、Host、Password/Private Key/System Agent/Keyboard-interactive
  Credential 和有序 Jump Route 的产品配置 UI。
- Rust-owned Native File Picker 私钥导入；Path 和 Key 内容不进入 WebView IPC，
  Linux/Windows Desktop 的加密 OpenSSH Key 使用原生 Secure Passphrase Prompt。
- 原生 Vault 创建、锁定和解锁界面。
- Docker OpenSSH 真实协议测试。
- Vitest、Playwright 和 agent-browser 测试路径。
- 原生 Wayland + IBus/libpinyin + xterm.js 中文组合输入到真实 SSH Shell。
- Android ARM64 Debug APK 构建，包含 Rust SSH、Vault 与 bundled SQLCipher。
- GitHub Actions Windows Runner 已成功产出 x86-64 Debug EXE。
- Windows 2025 Runner 已实际启动 EXE/WebView2，并验证 Vault、Repository、
  错误 PIN、锁定/解锁与进程重启恢复。
- Windows 2025 Runner 已通过 OpenSSH Agent Named Pipe 和 standalone
  `sshd.exe` 完成真实 System Agent SSH，且源 Private Key 在 AnySSH 启动前删除。
- Windows 2025 Runner 已通过 Native Picker 和 Credential UI 导入加密
  OpenSSH Key；错误 Passphrase 重试、源文件删除、真实 SSH 和重启恢复均通过。
- Linux X11 和 Windows 2025 Runner 已验证首次 TOFU、二次免提示、原生 Forget、
  重新 TOFU、进程重启和同 Endpoint OpenSSH Host Key Rotation 硬阻断。
- Linux X11/Wayland 已通过真实 OpenSSH PAM 完成纯 Keyboard-interactive
  Challenge；Docker OpenSSH 还覆盖 Password/Private Key/System Agent
  Partial-success + OTP、Saved Host 和 Interactive Jump Hop。
- Windows 2025 Runner 已通过 controlled russh Server 和真实 EXE/WebView2
  完成 masked Keyboard-interactive Challenge、Interactive Credential 重启和
  Response/Vault/Evidence 扫描。
- 本地 Browser、X11 和无 `DISPLAY` Wayland 已验证双 Session、同时 Pending
  Challenge、单 Tab Close、Inactive Tab 4 MiB Output 和双 Session Vault Lock；
  Windows 真实 EXE/WebView2 已验证 Agent Session 与第二个
  Keyboard-interactive Tab 并发及单 Tab Close。
- SSH Port Forwarding 的 Rust Core、Metadata-only Tauri/React UI 和 Browser
  Preview 已实现。真实 OpenSSH Protocol 已覆盖 Direct/Jump Local、Dynamic
  SOCKS5、Remote、4 MiB/Half-close、16 Forward/64 Connection 和 Cleanup；
-  X11/Wayland/Windows 原生 UI 已通过真实 Local/Dynamic/Remote Marker、Tab
  Close、Disconnect、Vault Lock 与 Payload Evidence Scan。
- Head `6fcb1a68d5d791d164f3ed43209aa3a9613b5acf` 的 GitHub Actions Run
  `30416305300` 九个 Job 全部通过；Forwarding 的 Browser/Linux/Windows
  Screenshot、Error Log、Payload Scan 和 Android/Linux/Windows Build Hash 已
  人工检查。ADR-0018 已接受。
- Head `56b37a10bf91c2c7bb20c88bb99041ca404c5691` 的 GitHub Actions Run
  `30368134792` 九个 Job 全部通过，Browser、X11、Wayland、Windows、Android、
  Linux 的 Multi Tab 截图、Error Log、Build Hash 和 Secret Scan 已人工检查。
- Head `0ceb5b332967a9b1fc7fdf73967ae49bf44505d7` 的 GitHub Actions Run
  `30360000884` 九个 Job 全部通过，Browser、X11、Wayland、Windows、Android、
  Linux 的关键截图、Error Log、Build Hash 和测试 Secret Scan 已人工检查。

iOS 因当前没有 macOS/Xcode 环境暂缓。

当前活动计划：

- [`Phase 1：Private Key Generation and Encrypted Export`](docs/execplans/active/0009-private-key-generation-and-encrypted-export.md)

已完成计划：

- [`Phase 0：技术风险验证`](docs/execplans/completed/0001-phase-0-technical-validation.md)
- [`Phase 1：Group 持久化与三态继承`](docs/execplans/completed/0002-group-persistence-and-inheritance.md)
- [`Phase 1：系统 SSH Agent 认证`](docs/execplans/completed/0003-system-ssh-agent-authentication.md)
- [`Phase 1：Native Encrypted Private Key Passphrase`](docs/execplans/completed/0004-native-encrypted-private-key-passphrase.md)
- [`Phase 1：Known Host Repository and Durable TOFU`](docs/execplans/completed/0005-known-host-repository-and-durable-tofu.md)
- [`Phase 1：Keyboard-interactive and OTP`](docs/execplans/completed/0006-keyboard-interactive-and-otp.md)
- [`Phase 1：Multi Tab Terminal and Session Lifecycle`](docs/execplans/completed/0007-multi-tab-terminal-and-session-lifecycle.md)
- [`Phase 1：SSH Port Forwarding`](docs/execplans/completed/0008-ssh-port-forwarding.md)

## 文档入口

- [文档导航](docs/README.md)
- [产品构想](docs/project/product-brief.md)
- [2026 技术架构](docs/design/technical-architecture-2026.md)
- [架构决策记录](docs/adr/README.md)
- [执行计划](docs/execplans/README.md)
- [Agent 工作说明](AGENTS.md)

## 开发说明

AI Agent 或贡献者在修改本仓库前，应先阅读根目录的 [`AGENTS.md`](AGENTS.md)。

常用命令：

```bash
pnpm install
pnpm dev
pnpm test
pnpm test:ssh:smoke
pnpm test:e2e
pnpm qa:browser
pnpm qa:native:xvfb
pnpm qa:native:wayland
pnpm qa:native:windows # Windows only
pnpm check:android
pnpm check:container:linux
pnpm check:container:android
```

## Linux 原生依赖

当前 Tauri 2 Linux 壳依赖 WebKitGTK 4.1 ABI。在 Arch Linux 上安装：

```bash
sudo pacman -S --needed \
  webkit2gtk-4.1 \
  weston \
  wayland-utils \
  ibus \
  ibus-libpinyin \
  libappindicator \
  patchelf
pnpm check:native
```

`webkitgtk-6.0` 可以与其共存，但不能替代 `webkit2gtk-4.1`。无桌面环境仍可完成
编译检查；启动原生窗口还需要可用的 X11 或 Wayland Display，CI 可使用 Xvfb
进行虚拟显示验证。

安装 Xvfb、XTest 开发文件、OpenSSH Client 和 Docker 后，可以运行真实原生
Vault、Private Key Import 与 SSH 检查：

```bash
pnpm qa:native:xvfb
pnpm qa:native:wayland
```

Wayland 检查把 Weston 嵌套在 Xvfb 中用于自动化，但 AnySSH 进程本身不继承
`DISPLAY`，并强制使用 `GDK_BACKEND=wayland`。测试会通过 IBus/libpinyin 把
`中文` 输入 xterm.js，并验证远端 OpenSSH Fixture 收到包含该 UTF-8 前缀的
文件名。精确候选后缀可能因发行版提供的 libpinyin 版本而不同。

## Android 构建

Android Phase 0 使用 JDK 17、Android SDK 36、Build Tools 35.0.0 和
NDK 29.0.13846066：

```bash
pnpm check:container:android
```

该命令在独立 Docker Image 中构建 ARM64 Debug APK，并检查 Application ID 和
Rust Native Library。宿主机已经具备完整 Android Toolchain 时，也可直接运行
`pnpm check:android`。

Linux 独立构建环境使用：

```bash
pnpm check:container:linux
```

Windows 仍使用原生 Windows CI 验证 MSVC/WebView2。iOS 构建必须等待可用的
macOS/Xcode 环境，Linux Docker 不能替代。

Windows 上的 `pnpm qa:native:windows` 会构建 Debug EXE，启动真实 Tauri/
WebView2 Runtime，并通过独立 `tauri.windows-qa.conf.json` 使用仅限 QA Build
的 Loopback CDP Port 验证 Vault、Repository、Native Encrypted Key
Picker/Prompt、OpenSSH 和重启恢复。该端口不进入 Canonical 或 Release 配置。

## 许可证

AnySSH 使用 `AGPL-3.0-only`。第三方依赖和字体继续适用各自的许可证。
