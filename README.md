# AnySSH

AnySSH 是一个处于 Phase 0 技术验证阶段的开源跨平台 SSH 客户端项目。

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
- 保存的 SHA-256 Host Key 匹配与变化硬阻断。
- 4 MiB 终端输出下的 64 项 Core Queue、8 项 WebView Window 和 xterm Ack 背压。
- 随机 VMK、Argon2id PIN Slot、HKDF 子密钥和版本化 Bootstrap。
- SQLCipher 4.10 整库加密与 XChaCha20-Poly1305 Credential 字段加密。
- 专用 DB Actor Thread、16 项有界 Command Queue 和 oneshot Response。
- Schema v3 Host/Jump Route Repository；Host 只保存 Credential/Route ID，
  Route 只保存有序 Host ID。
- Saved Host 连接 IPC 只提交 Host ID；DB Actor 在 Rust 内解析 Credential 和
  Route，SSH Core 最多执行 32 个有序 Jump Host。
- Schema v2 Credential Repository，以及 Credential ID -> Vault -> SSH Core
  的 Rust-only Private Key 路径。
- 原生 Vault 创建、锁定和解锁界面。
- Docker OpenSSH 真实协议测试。
- Vitest、Playwright 和 agent-browser 测试路径。
- 原生 Wayland + IBus/libpinyin + xterm.js 中文组合输入到真实 SSH Shell。
- Android ARM64 Debug APK 构建，包含 Rust SSH、Vault 与 bundled SQLCipher。
- GitHub Actions Windows Runner 已成功产出 x86-64 Debug EXE。

Host/Jump Route 产品配置 UI、原生 Private Key 导入、SSH Agent 和 Windows
运行验证尚未完成。
iOS 因当前没有 macOS/Xcode 环境暂缓。

当前活动计划：

- [`Phase 0：技术风险验证`](docs/execplans/active/0001-phase-0-technical-validation.md)

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

安装 Xvfb、XTest 开发文件和 Docker 后，可以运行真实原生 SSH 检查：

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

## 许可证

AnySSH 使用 `AGPL-3.0-only`。第三方依赖和字体继续适用各自的许可证。
