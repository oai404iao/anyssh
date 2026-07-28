# AGENTS.md - AnySSH

> AnySSH 仓库的 AI Coding Agent 操作手册。开始修改前先阅读本文件，再阅读活动 ExecPlan。

## 项目是什么

AnySSH 是一个采用 `AGPL-3.0-only` 的开源、跨平台、端到端加密 SSH 客户端。
目标平台是 Linux（Wayland/X11）、Windows、Android 和 iOS。

计划中的运行时结构为：

```text
React/xterm.js UI
  -> Typed Tauri IPC
    -> Rust Application Core
      -> russh / Vault / SQLCipher / Sync
        -> Platform Security APIs
```

仓库已完成 **Phase 0 技术验证**、**Phase 1 Group 持久化/三态继承**、
**System SSH Agent 认证**、**Native Encrypted Private Key Passphrase** 和
**Known Host Repository/Durable TOFU**、**Keyboard-interactive and OTP**、
**Multi Tab Terminal and Session Lifecycle**。
当前活动计划是 **SSH Port Forwarding**。
已经存在可构建的 React 前端、Rust Workspace、russh SSH/Jump Host、SQLCipher/
PIN Vault、Tauri IPC、Host/Credential/Route Repository、Windows WebView2、
OpenSSH Fixture、Playwright E2E、agent-browser 与原生 X11/Wayland 检查。

## 仓库布局

```text
any_ssh/
|-- AGENTS.md                         # Agent 的首要操作规则
|-- README.md                         # 人类入口和当前状态
|-- Cargo.toml                        # Rust Workspace；默认成员为核心 crate
|-- package.json                      # 根 canonical pnpm 命令
|-- pnpm-workspace.yaml               # pnpm Workspace 与供应链配置
|-- rust-toolchain.toml               # 固定 Rust 1.93.1
|-- .github/workflows/ci.yml          # CI 的真实验证路径
|-- apps/client/
|   |-- src/                          # React、xterm.js、Browser QA Bridge
|   |-- e2e/                          # Playwright 浏览器 E2E
|   `-- src-tauri/                    # Tauri 壳、命令与 Session Registry
|-- crates/
|   |-- anyssh-app/                   # Application Service、Saved Host Plan -> SSH
|   |-- anyssh-domain/                # Endpoint、TerminalSize 等领域值对象
|   |-- anyssh-ssh/                   # russh Session、Host Key、PTY、背压
|   |-- anyssh-vault/                 # VMK、PIN Key Slot、HKDF、Bootstrap
|   `-- anyssh-storage/               # DB Actor、Schema v7、Repository、Record AEAD
|-- scripts/
|   |-- build-in-container.sh          # 独立 Linux/Android Build Image 入口
|   |-- check-android-build.sh         # Android ARM64 APK 与 bundled SQLCipher 构建
|   |-- check-linux-build.sh           # Linux Tauri ELF 构建与链接检查
|   |-- test-ssh-smoke.sh             # Docker OpenSSH 真实协议检查
|   |-- check-doc-links.py            # 本地 Markdown 链接检查
|   `-- qa/
|       |-- agent-browser-smoke.sh    # Agent 驱动的浏览器 UI 检查
|       |-- native-xvfb-smoke.sh      # 无桌面环境的原生 Tauri X11 SSH 检查
|       |-- native-wayland-ime-smoke.sh # Wayland + IBus + SSH IME 检查
|       |-- native-windows-smoke.ps1  # Windows EXE/WebView2/OpenSSH 检查
|       `-- windows-native-dialog-driver.ps1 # Windows Picker/Prompt QA Driver
|-- tests/fixtures/openssh/           # 隔离 OpenSSH Docker Fixture
|-- infra/build/                      # 固定 Rust/Node/Android/Linux Build Images
`-- docs/
    |-- README.md                     # 全部文档导航
    |-- project/                      # 产品目标、范围、状态、路线图
    |-- design/                       # 系统如何实现的设计文档
    |-- adr/                          # 架构决策及其状态
    |-- execplans/                    # 可执行、可持续更新的工作计划
    `-- reference/                    # 外部技术基线、术语等参考资料
```

后续计划中的 Platform、Sync 等目录见
[`docs/design/technical-architecture-2026.md`](docs/design/technical-architecture-2026.md)。
不要创建没有真实代码的空 crate。

## 信息源优先级

发生冲突时按以下顺序处理：

1. 用户在当前任务中的明确指令。
2. 状态为 **Accepted** 的 ADR。
3. 当前活动 ExecPlan。
4. `docs/design/` 中的设计文档。
5. `docs/project/` 中的产品文档。
6. `docs/reference/` 中的参考信息。

Proposed ADR 是待验证方案，不是不可变事实。若 Phase 0 验证结果与 Proposed ADR 冲突，应更新 ExecPlan 的发现和决策日志，再修改或替代 ADR。

## 开始工作前

1. 阅读本文件。
2. 阅读 [`docs/README.md`](docs/README.md)。
3. 阅读与任务有关的产品文档、设计文档和 ADR。
4. 检查 `docs/execplans/active/` 是否已有覆盖该工作的计划。
5. 多步骤、跨模块或高风险工作必须先创建或更新 ExecPlan。
6. 会改变长期架构、数据格式、安全边界或依赖方向的工作必须创建 ADR。

## Workspace 与依赖规则

- JavaScript 包管理器是 pnpm；依赖命令从仓库根目录执行。
- 只提交根目录 `pnpm-lock.yaml`，不要在子项目创建额外 lockfile。
- `apps/client` 的包名是 `@anyssh/client`。
- Rust 使用根 Cargo Workspace 和 `Cargo.lock`。
- `cargo check` 默认只检查核心 crate，避免没有 WebKitGTK 的环境误编译 Tauri。
- Tauri 原生检查使用 `cargo check --package anyssh-client`。
- Rust Core 不依赖 React/Tauri；Tauri 可以依赖 Core。
- 不把业务逻辑放进 Tauri command；command 只校验、转换和调用 Core。
- Tauri 业务调用只进入 `ApplicationCore`；`ApplicationCore` 通过
  `DatabaseActorHandle` 访问 Vault/SQLCipher。不得在 Tauri Command 或其他
  Runtime State 中直接持有 `LocalVault`/`rusqlite::Connection`。
- SSH Private Key/Passphrase 不得出现在 Tauri IPC。保存的认证只传 Credential
  ID；显式临时密码仅用于当前未保存连接。`anyssh-app` 在 Rust 内解析并直接构造
  SSH Authentication。
- Native Private Key Import Request 只允许 Label/Username。Tauri Command
  必须在 Rust 内打开 Native Picker，再由 `ApplicationCore` 有界读取和验证；
  WebView 不得提交 Path、Private Key 或 Passphrase。Linux/Windows Desktop
  支持加密 OpenSSH Key；Android/iOS v1 对加密 Key 明确返回 Unsupported。
- 加密 Key Passphrase 必须通过进程内/系统原生 Secure Prompt 获取，不得使用
  React Input、普通 IPC、Shell、`zenity` 或 PowerShell 子进程。Prompt Result
  必须立即进入 `Zeroizing<String>`，取消或失败不得创建 Credential。
- System SSH Agent Identity 枚举和签名只在 Rust 内执行。WebView 不得提交
  Agent Socket/Pipe Path、Public Key Blob 或签名 Payload；Credential 必须用
  SHA-256 Fingerprint 选择唯一 Identity，不得自动尝试 Agent 中全部 Key。
- Keyboard-interactive Credential 只保存 Label/Username，不保存 OTP Seed、
  Response、Prompt Rule 或 Saved Password 映射。Response 只允许存在于当前
  Request-scoped React 表单、当前 Typed IPC 和 Rust `Zeroizing<String>`，并
  必须绑定 Session/Request/Hop/Round；提交、取消、断开、锁定或切页立即清空。
- Password/Private Key/System Agent 第一因子只有在 Server 明确返回 Partial
  Success 且继续提供 Keyboard-interactive 时才进入第二因子；普通失败不得自动
  降级或回退。
- Host 只保存可选 Credential/Jump Route ID；Jump Route Step 只保存 Host ID。
  不得在 Host 或 Route 中复制 Username、Password、Private Key 或 Passphrase。
- Saved Host Connect IPC 只传 Target Host ID 和 Terminal Size。Route 展开与
  Credential 解析必须在 DB Actor/Rust 内完成，不得由 WebView 拼装连接计划。
- VMK、KEK、数据库 Key 和解密后的 Credential 不得序列化到前端。
- Vault Bootstrap 只能包含版本、随机 ID、KDF 参数和加密 Key Slot。
- 新增依赖时检查其 AGPLv3 兼容性，并更新 lockfile。
- 第三方字体和素材必须保留各自的许可证文件或声明。

## Build、Test 与开发命令

所有命令从仓库根目录执行。

```bash
# 安装
pnpm install

# Browser QA 前端开发服务，固定端口 1420
pnpm dev

# Tauri 原生窗口；需要平台系统依赖
pnpm dev:native
pnpm check:native
pnpm check:android
pnpm check:container:linux
pnpm check:container:android

# Arch Linux 当前需要 WebKitGTK 4.1 ABI；webkitgtk-6.0 不能替代
sudo pacman -S --needed webkit2gtk-4.1

# 前端类型检查与生产构建
pnpm typecheck
pnpm build

# 前端 lint、单元测试
pnpm lint:frontend
pnpm test:frontend

# Rust 核心
pnpm lint:rust
pnpm test:rust
pnpm test:vault

# 真实 OpenSSH Docker Fixture
pnpm test:ssh:smoke

# 传统浏览器 E2E
pnpm test:e2e

# Agent 驱动的真实交互、截图和移动视口检查
pnpm qa:browser

# Xvfb 下启动真实 Tauri/WebKitGTK，并连接 Docker OpenSSH
pnpm qa:native:xvfb

# Windows 2025：构建 Debug EXE 并通过 WebView2 CDP 验证原生运行
pnpm qa:native:windows

# Weston 原生 Wayland、IBus/libpinyin、xterm 中文组合输入
pnpm qa:native:wayland

# 文档链接
pnpm docs:check

# 全部格式化/检查
pnpm format
pnpm format:check
```

Tauri Linux 原生编译额外需要 WebKitGTK 4.1、JavaScriptCoreGTK 4.1、GTK3 等系统依赖。
当前 CI 使用 `native-linux-check`、`linux-container-build`、`android-build` 和
`windows-build` 记录平台构建证据；iOS 因缺少 macOS/Xcode 环境暂缓。

Linux 和 Android 的规范 Build 路径优先使用 `infra/build/Dockerfile` 的独立
Target Image。容器入口只复制 Git 已跟踪和未忽略的工作树文件，不继承宿主环境变量，
不挂载 Docker Socket，移除 Linux Capability，并只把 `artifacts/` Build
Evidence 复制回仓库。

## 测试要求

### 单元与静态检查

- React/Bridge：Vitest，路径 `apps/client/src/**/*.test.{ts,tsx}`。
- Rust：普通单元测试和 integration test。
- Playwright 文件必须放在 `apps/client/e2e/`，不得被 Vitest 收集。

### OpenSSH 协议检查

`pnpm test:ssh:smoke`：

- 构建隔离 Alpine/OpenSSH 双 Jump Host、Internal/Deep Target 和黑洞握手
  Fixture。
- 真实完成 TCP、SSH Handshake、Host Key 确认、密码认证、PTY 和命令输出。
- 验证未加密/口令保护私钥、错误口令、未授权 Key 和密码 Jump + 私钥 Target。
- 验证 Vault Lock/Unlock 后 Credential ID -> 加密 Private Key -> SSH Core。
- 验证已保存 Host Key 匹配免提示、Host Key 变化硬阻断和 4 MiB 输出背压。
- 验证 `direct-tcpip` 两跳、逐跳 Host Key、取消、超时、Target 认证失败和
  第一跳断开。
- 验证 Saved Host ID -> Password Jump 1 -> Password Jump 2 -> Private Key
  Target，并确认 Jump 2 认证失败按 Hop 归属。
- 验证真实 `ssh-agent` Direct、Password Jump -> Agent Target、Agent Jump ->
  Private Key Target，以及错误 Fingerprint 不回退。
- 验证 OpenSSH PAM 纯 Keyboard-interactive、Password/Private Key/System Agent
  Partial-success + OTP、错误/正确 Response、Saved Host、Interactive Jump Hop
  和 Response 明文扫描。
- Fixture 凭据只能用于测试，不得替换为真实主机或真实密钥。

### Vault 检查

`pnpm test:rust` 必须覆盖：

- PIN Slot 创建、正确/错误 PIN、损坏 Slot。
- DB Actor 有界 Queue、oneshot Response、串行生命周期和 Shutdown。
- Schema v1 -> v2 Credential、Schema v2 -> v3 旧 Host Password 转
  Credential、Schema v3 -> v4 Group/三态 Override、Schema v4 -> v5 System
  Agent Credential、Schema v5 -> v6 Known Host、Schema v6 -> v7 Interactive
  Credential Migration 的成功、重启和中断回滚。
- Host/Jump Route 引用占用、顺序恢复、直接/间接循环和 Locked Repository 拒绝。
- SQLCipher 重启解锁和 Credential 字段 AEAD。
- 数据库、WAL、Sidecar 与 Bootstrap 明文扫描。
- Schema migration 中断回滚。

`pnpm qa:native:xvfb` 还必须覆盖原生 Vault 创建、错误 PIN、锁定和重新解锁。
同时必须验证 Tauri/xterm Ack 背压能排空 4 MiB 输出并继续执行后续远端命令。
该检查还必须通过真实 Native File Picker 导入测试 Private Key，确认 UI 只返回
metadata。加密 Key 必须经过进程内 GTK Secure Entry、至少一次错误 Passphrase
重试和正确 Passphrase 导入；Key Header/测试 Passphrase 不得出现在 Vault 或
Evidence，且临时源文件在 SSH 流程前删除。
Linux X11 检查还必须启动真实 `ssh-agent`，由原生 Tauri UI 枚举 Identity 并
创建 Fingerprint-selected Credential，且 Agent Key/Fingerprint 不得明文落盘。
它还必须通过独立 OpenSSH PAM Endpoint 完成 masked Keyboard-interactive
Challenge，并确认 Response 不出现在 Vault 或 Native Log。
Multi Tab 变更还必须同时连接两个真实 OpenSSH Session，证明 Inactive Tab 能
排空 4 MiB Output、关闭一个 Tab 后另一个继续接受命令，并在两个 Session
Connected 时由 Lock Vault 全量清理。

`pnpm qa:native:windows` 只在 Windows 执行。它必须启动实际构建的 EXE、确认
标题为 `AnySSH` 的非零窗口句柄，并通过
`apps/client/src-tauri/tauri.windows-qa.conf.json` 仅为该 Debug QA Build 启用
Loopback WebView2 CDP Port。Canonical Tauri Config、Capability 和 Release
Build 不得暴露 CDP。测试必须覆盖 Vault Create/Lock/Wrong-PIN/Unlock、
Repository CRUD、进程重启恢复、SQLCipher 明文扫描和截图；不得上传 WebView2
Profile 或 Vault 文件。System Agent 变更还必须通过 Windows OpenSSH Agent
Named Pipe 和临时 standalone OpenSSH Server 完成真实 EXE SSH 交互；Agent
Private Key 文件必须在 AnySSH 启动前删除。加密 Private Key 变更还必须通过
Native Picker、Windows Credential UI 错误/正确 Passphrase、源文件删除和真实
OpenSSH Marker；QA Driver 的 Path/Passphrase 环境必须在 AnySSH 启动后设置，
避免应用进程继承测试 Secret。Keyboard-interactive 变更还必须通过 controlled
russh Server 和真实 EXE/WebView2 完成 Challenge/Response；测试 Response 环境
同样只能在 AnySSH 启动后提供给外部 QA Driver。
Multi Tab 变更还必须保持一个真实 Agent Session Connected，在第二个 Tab 完成
Keyboard-interactive Challenge，关闭第二个 Tab 后再次通过第一个 Session 创建
远端 Marker。

`pnpm qa:native:wayland` 必须在 AnySSH 进程没有 `DISPLAY` 的条件下：

- 强制 `GDK_BACKEND=wayland`。
- 通过 Weston Wayland Socket 启动真实 Tauri/WebKitGTK。
- 使用 IBus/libpinyin 在 xterm.js 中提交中文组合文本。
- 通过远端 OpenSSH 文件 Marker 验证 UTF-8 文本真正到达 SSH Shell。
- 通过独立 OpenSSH PAM Endpoint 完成一次 masked Keyboard-interactive
  Challenge，并扫描 Response 不进入 Vault/App Log。
- 保持第一个 OpenSSH Session Connected，在第二个 Tab 完成 Challenge；关闭
  第二个 Tab 后第一个 Session 必须继续接受远端命令。

`pnpm check:android` 必须产出 ARM64 Debug APK，并验证：

- Application ID 为 `com.spiredive.anyssh`。
- APK 包含 `arm64-v8a/libanyssh_client_lib.so`。
- SSH、Vault 和 bundled SQLCipher 成功交叉编译。

`pnpm check:container:linux` 和 `pnpm check:container:android` 必须分别调用上述
平台检查，不允许依赖宿主已安装的 WebKitGTK、JDK、Android SDK 或 NDK。

QA 报告不得保存完整进程环境；只允许白名单式记录当前测试必需且不含秘密的
环境字段。

### Playwright E2E

`pnpm test:e2e` 验证标准浏览器工作流。Browser QA mode 是 UI 模拟，不打开网络连接；它不能替代 OpenSSH smoke。
Multi Tab 变更至少覆盖两个 Preview Session 的 Output 隔离、同时 Pending
Challenge、Close-during-connect、单 Tab Close 和 8 Tab 上限。

### agent-browser 真实检查

UI/交互变更除了传统 E2E，还必须运行：

```bash
pnpm qa:browser
```

该检查必须：

- 实际点击和填写页面，而不只断言 DOM。
- 覆盖 Host Key Dialog、密码显示/隐藏、终端真实键盘输入和 Disconnect。
- Multi Tab 变更必须实际创建两个 Preview Tab，在 Desktop/Mobile 截图中检查
  Tab Strip，并关闭一个 Tab 后继续向另一个 Terminal 输入。
- 检查桌面与移动视口。
- 检查 Browser Errors。
- 生成并人工查看 `artifacts/agent-browser/` 中的截图和报告。

只看到脚本退出码为 0 不算完成；Agent 必须查看关键截图，确认不存在截断、遮挡、字体或响应式问题。录制视频需要系统 `ffmpeg`，没有时截图仍是必需证据。

### 文档

文档修改至少运行 `pnpm docs:check`，并确认 ADR/ExecPlan 索引和移动后的路径同步。

## 架构约束

以下约束来自 Accepted ADR、当前 Proposed ADR 和 Threat Model；具体状态以
`docs/adr/README.md` 为准。

### 1. 秘密不得长期进入 WebView

- React 只持有展示模型、页面状态和终端数据。
- 保存的密码、私钥、VMK、KEK、数据库密钥和长期 Token 留在 Rust/原生层。
- Quick Connection 的一次性临时密码可以存在于局部表单并通过当前请求提交，但
  不得进入全局状态、日志或持久化，提交、取消、锁定和切页时必须清空。
- Keyboard-interactive Response 同样只能存在于按 Request ID 重建的局部表单和
  当前 IPC；所有 Prompt Response 都按临时秘密处理，即使 Server 要求回显。
- 临时显示密码必须经过 step-up authentication，并设置短 TTL。
- 秘密不得进入前端全局状态、日志、错误对象、崩溃报告或遥测。

### 2. 不直接同步 SQLite/SQLCipher 文件

- 本地存储与同步协议是两个边界。
- 每次可同步的领域修改都应产生 Operation/Outbox 记录。
- WebDAV 使用加密的不可变操作日志、快照和确定性合并。

### 3. 生物识别不是 KDF

- VMK 是随机 256-bit 密钥。
- 生物识别只授权平台密钥解包 VMK。
- PIN/同步密码使用 Argon2id 派生 KEK。
- 必须保留 PIN 或 Recovery Slot，处理生物信息变化导致的 Slot 失效。

### 4. SSH 不依赖系统子进程

- 默认 SSH Engine 为 russh。
- Jump Host 通过 `direct-tcpip` Channel 和流适配实现。
- 不通过调用系统 `ssh` 完成核心功能。
- 系统 Agent 是外部签名能力，不等于调用系统 SSH CLI。

### 5. 默认使用现代 SSH 算法

- 默认优先 ML-KEM/X25519、Curve25519、ChaCha20-Poly1305、AES-GCM 和 SHA-2。
- SHA-1、DSA、CBC 和过时 KEX 只能按 Host 显式启用。
- 不增加全局“接受所有旧算法”开关。

### 6. Group 继承使用三态

可继承字段必须能表达：

```text
Inherit
Set(value)
Clear
```

普通可空字段无法区分“继承”与“明确清除父配置”。

### 7. Known Host Trust 按 Endpoint 持久化

- 身份使用规范化逻辑 `host + explicit port`，不绑定 Host/Group/Route ID 或
  解析后的 IP。
- Quick、Saved、Jump 和 Target 必须共用 SQLCipher Known Host Repository。
- TOFU 接受必须先持久化，再继续 SSH Worker；DB Failure、Vault Lock、过期
  Request 或并发冲突均 Fail Closed。
- 相同 Endpoint 并发不同 Key 使用 First-writer-wins，不自动合并 Trust Set。
- 已知 Endpoint 的不同 Key 必须 typed hard block，不提供 Accept/Replace。
- WebView 只获得 Endpoint、Algorithm、Fingerprint 等元数据；完整 Observed
  Public Key 留在 Rust。
- Forget 只接受 Known Host ID，并必须经过 WebView 外的原生确认。
- 未来同步把 Endpoint Trust Set 当作原子状态，冲突时阻断而不是取并集。

### 8. Keyboard-interactive Response 是 Session-bound

- Interactive Credential 只保存 Label、Username 和 Kind，Schema v7 的
  Secret/Passphrase 四列必须全部为 `NULL`。
- Server Prompt 是不可信纯文本，必须清理控制字符并限制 Name、Instructions、
  Prompt Count/Text 和 Response Size。
- 每次只允许一个 Pending Authentication Request；Stale、Duplicate、超时、
  数量不匹配、取消和 UI 丢失都 Fail Closed。
- 零 Prompt Round 自动提交空 Response，但仍计入最多 8 Round 的上限。
- 不根据 Prompt 文本自动填充 Saved Password，也不保存 OTP Seed 或 Response。

### 9. Session Tab 拥有独立 Runtime Lifecycle

- Frontend Tab ID、Rust Session ID 和 Connection Generation 必须分离。
- 每 Tab 独立拥有 Event/Data Callback、xterm.js、Terminal Size、Status、
  Host Key 和 Authentication State。
- Inactive Terminal 必须保持 Mounted 并继续 xterm Write Ack；只有 Visible
  Terminal 执行 Fit/Resize。
- Disconnect 保留 Tab/Scrollback；Close 只 Disconnect 并移除目标 Tab。
- 最多 8 个 Tab，达到上限不得自动回收 Live Session。
- Late Connect Return、Stale Event、Channel Loss、Vault Lock 和 App Exit 必须
  Fail Closed；多个 Pending Action 不得持续抢焦点或交叉提交 Response。

### 10. Port Forwarding 留在 Rust 并绑定 Session

- Local、Remote、Dynamic Forward Payload 不得进入 WebView/Tauri Event、日志或
  遥测，也不得调用系统 `ssh -L/-R/-D`。
- v1 Forward 绑定 Live Rust Session；Disconnect、Tab Close、Channel Loss、
  Vault Lock 和 App Exit 必须取消 Listener、Registration 和 Connection Task。
- v1 Local/Dynamic/Remote Bind 只允许 Loopback；Wildcard/LAN/Public Bind
  Fail Closed。
- Dynamic v1 只支持无认证 SOCKS5 `CONNECT`，拒绝 SOCKS4、`BIND` 和
  `UDP ASSOCIATE`。
- 每 Session 最多 16 个 Forward、每 Forward 最多 64 个 Connection，所有
  Accept Queue、Handshake、Connect 和 Copy 必须有界并可取消。

## 文档规则

### ADR

- 路径：`docs/adr/NNNN-short-title.md`
- 状态：Proposed、Accepted、Deprecated、Superseded、Rejected。
- Accepted ADR 不直接改写历史决策；重大变更创建新 ADR 并标记 supersedes。
- 新增或变更 ADR 后更新 `docs/adr/README.md`。

### ExecPlan

- 活动计划放在 `docs/execplans/active/`。
- 完成后移动到 `docs/execplans/completed/`，不得删除决策和结果记录。
- 执行期间持续更新 Progress、Surprises、Decision Log 和 Outcomes。
- 格式要求见 `docs/execplans/README.md`。

### Design

- 设计文档描述“系统如何工作”和边界条件。
- 设计改变了长期决策时，必须同时新增或更新 ADR。
- 不把短期任务清单长期留在 design 文档；任务属于 ExecPlan。

### Reference

- Reference 记录外部事实、版本基线、协议和术语。
- Reference 不是架构决策来源。
- 带“最新”“当前”含义的内容必须标注核验日期。

## 高风险区域

### 1. Vault 与密钥迁移

任何 VMK、Key Slot、KDF 参数、SQLCipher Key 或备份格式变化，都必须包含：

- 版本号。
- 向前迁移路径。
- 中断恢复策略。
- 已有数据恢复测试。

### 2. 同步格式

任何 Operation、Snapshot、HLC、Tombstone 或文件布局变化，都必须更新：

- 同步格式设计。
- 兼容策略。
- 冲突测试。
- 恶意或异常 WebDAV 行为测试。

### 3. SSH 输出与终端 IPC

- 不逐字节发送事件。
- 不对大量输出做 Base64 JSON 包装。
- 必须有有界队列和背压。
- WebGL 必须有回退路径。

### 4. 平台能力差异

- 不承诺 iOS 后台长期保持任意 SSH/Tunnel。
- 不假设 Linux 一定存在 Secret Service 或生物识别。
- Android 长连接设计必须考虑 Foreground Service。
- 平台回退不能偷偷降低 Vault 的密码学保护。

## 常见工作流

### 新增架构能力

1. 检查现有 ADR。
2. 新建/更新 ExecPlan。
3. 必要时新增 Proposed ADR。
4. 更新 design 文档。
5. 实现和验证。
6. 根据验证结果接受、拒绝或替代 ADR。
7. 更新文档索引和 AGENTS.md 中受影响的规则。

### 新增领域对象

1. 更新领域模型设计。
2. 明确 ID、版本和 Group 继承行为。
3. 明确本地加密字段。
4. 明确同步 Operation 和冲突策略。
5. 再实现数据库与 IPC。

### 修改安全敏感功能

1. 在 ExecPlan 中写明威胁和非目标。
2. 确认秘密是否跨越 Rust/WebView 边界。
3. 确认日志、剪贴板、导出和崩溃路径。
4. 增加失败、恢复和迁移测试。
5. 更新 [`docs/design/threat-model-v1.md`](docs/design/threat-model-v1.md)。

## 当前下一步

当前唯一活动计划是：

- [`0008-ssh-port-forwarding.md`](docs/execplans/active/0008-ssh-port-forwarding.md)

除非用户明确改变优先级，应先完成 Rust-owned Session-scoped Local/Remote/
Dynamic Forward、Loopback Policy、SOCKS5、Cleanup 和 Linux/Windows Native
Evidence；不要直接跳到 WebDAV、SFTP、持久化 Forward Profile 或高级脚本系统。

## 关键文件

| 内容 | 路径 |
| --- | --- |
| 根开发命令 | `package.json` |
| Rust Workspace | `Cargo.toml` |
| React 入口 | `apps/client/src/App.tsx` |
| 配置工作区 | `apps/client/src/components/ConfigurationWorkspace.tsx` |
| Terminal Adapter | `apps/client/src/components/TerminalPane.tsx` |
| SSH Bridge | `apps/client/src/lib/ssh-bridge.ts` |
| Credential Bridge | `apps/client/src/lib/credential-bridge.ts` |
| Host/Route Bridge | `apps/client/src/lib/host-bridge.ts` |
| Known Host Bridge | `apps/client/src/lib/known-host-bridge.ts` |
| Tauri IPC | `apps/client/src-tauri/src/lib.rs` |
| Native Passphrase Provider | `apps/client/src-tauri/src/native_passphrase.rs` |
| Native Known Host Provider | `apps/client/src-tauri/src/native_known_host.rs` |
| Application Core | `crates/anyssh-app/src/lib.rs` |
| DB Actor | `crates/anyssh-storage/src/actor.rs` |
| Credential Model | `crates/anyssh-storage/src/credential.rs` |
| Host Model | `crates/anyssh-storage/src/host.rs` |
| Group Model | `crates/anyssh-storage/src/group.rs` |
| 三态 Override | `crates/anyssh-storage/src/inheritance.rs` |
| Jump Route Model | `crates/anyssh-storage/src/jump_route.rs` |
| Known Host Model | `crates/anyssh-storage/src/known_host.rs` |
| Connection Plan | `crates/anyssh-storage/src/connection_plan.rs` |
| Native Key Import Design | `docs/design/native-private-key-import-v1.md` |
| Encrypted Key Prompt Design | `docs/design/native-encrypted-private-key-passphrase-v1.md` |
| Known Host Design | `docs/design/known-host-repository-v1.md` |
| Keyboard-interactive Design | `docs/design/keyboard-interactive-authentication-v1.md` |
| Multi Tab Session Design | `docs/design/multi-tab-session-lifecycle-v1.md` |
| SSH Port Forwarding Design | `docs/design/ssh-port-forwarding-v1.md` |
| OpenSSH Known Hosts Reference | `docs/reference/openssh-known-hosts-baseline-2026.md` |
| Threat Model | `docs/design/threat-model-v1.md` |
| SSH Core | `crates/anyssh-ssh/src/lib.rs` |
| Controlled Interactive Server | `crates/anyssh-ssh/examples/keyboard_interactive_server.rs` |
| OpenSSH Fixture | `tests/fixtures/openssh/` |
| Playwright E2E | `apps/client/e2e/connect-preview.spec.ts` |
| agent-browser 检查 | `scripts/qa/agent-browser-smoke.sh` |
| Windows Runtime 检查 | `scripts/qa/native-windows-smoke.ps1` |
| Windows Native Dialog Driver | `scripts/qa/windows-native-dialog-driver.ps1` |
| CI | `.github/workflows/ci.yml` |
| 产品目标 | `docs/project/product-brief.md` |
| 项目状态 | `docs/project/status.md` |
| 路线图 | `docs/project/roadmap.md` |
| 总体技术设计 | `docs/design/technical-architecture-2026.md` |
| ADR 索引 | `docs/adr/README.md` |
| ExecPlan 规范 | `docs/execplans/README.md` |
| 当前活动计划 | `docs/execplans/active/0008-ssh-port-forwarding.md` |
| 最新完成计划 | `docs/execplans/completed/0007-multi-tab-terminal-and-session-lifecycle.md` |
| Phase 0 结果 | `docs/execplans/completed/0001-phase-0-technical-validation.md` |
| Group 结果 | `docs/execplans/completed/0002-group-persistence-and-inheritance.md` |
| System Agent 结果 | `docs/execplans/completed/0003-system-ssh-agent-authentication.md` |
| Encrypted Key Prompt 结果 | `docs/execplans/completed/0004-native-encrypted-private-key-passphrase.md` |
| Known Host 结果 | `docs/execplans/completed/0005-known-host-repository-and-durable-tofu.md` |
| Keyboard-interactive 结果 | `docs/execplans/completed/0006-keyboard-interactive-and-otp.md` |
| Multi Tab 结果 | `docs/execplans/completed/0007-multi-tab-terminal-and-session-lifecycle.md` |
| 2026 技术基线 | `docs/reference/technology-baseline-2026.md` |
| 术语表 | `docs/reference/glossary.md` |

## 维护本文件

新增或修改根脚本、Workspace、CI、入口文件或测试层时，必须同步本文件。命令示例必须和 `package.json`、Cargo Workspace、CI 实际调用一致。

本文件只记录可从仓库验证的事实和真正影响 Agent 决策的规则。
