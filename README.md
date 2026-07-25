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
- Docker OpenSSH 真实协议测试。
- Vitest、Playwright 和 agent-browser 测试路径。

SQLCipher Vault、Jump Host 和移动平台验证尚未完成。

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
```

## Linux 原生依赖

当前 Tauri 2 Linux 壳依赖 WebKitGTK 4.1 ABI。在 Arch Linux 上安装：

```bash
sudo pacman -S --needed webkit2gtk-4.1
pnpm check:native
```

`webkitgtk-6.0` 可以与其共存，但不能替代 `webkit2gtk-4.1`。无桌面环境仍可完成
编译检查；启动原生窗口还需要可用的 X11 或 Wayland Display，CI 可使用 Xvfb
进行虚拟显示验证。

安装 Xvfb、XTest 开发文件和 Docker 后，可以运行真实原生 SSH 检查：

```bash
pnpm qa:native:xvfb
```

## 许可证

AnySSH 使用 `AGPL-3.0-only`。第三方依赖和字体继续适用各自的许可证。
