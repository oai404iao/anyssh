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

仓库当前处于 **Phase 0 技术验证实施阶段**。已经存在可构建的 React 前端、
Rust Workspace、russh SSH 原型、SQLCipher/PIN Vault、Tauri IPC 适配、
OpenSSH Fixture、Playwright E2E、agent-browser 与原生 Xvfb 真实交互检查。

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
|   |-- anyssh-domain/                # Endpoint、TerminalSize 等领域值对象
|   |-- anyssh-ssh/                   # russh Session、Host Key、PTY、背压
|   |-- anyssh-vault/                 # VMK、PIN Key Slot、HKDF、Bootstrap
|   `-- anyssh-storage/               # SQLCipher、Schema、Record AEAD
|-- scripts/
|   |-- test-ssh-smoke.sh             # Docker OpenSSH 真实协议检查
|   |-- check-doc-links.py            # 本地 Markdown 链接检查
|   `-- qa/
|       |-- agent-browser-smoke.sh    # Agent 驱动的浏览器 UI 检查
|       `-- native-xvfb-smoke.sh      # 无桌面环境的原生 Tauri SSH 检查
|-- tests/fixtures/openssh/           # 隔离 OpenSSH Docker Fixture
`-- docs/
    |-- README.md                     # 全部文档导航
    |-- project/                      # 产品目标、范围、状态、路线图
    |-- design/                       # 系统如何实现的设计文档
    |-- adr/                          # 架构决策及其状态
    |-- execplans/                    # 可执行、可持续更新的工作计划
    `-- reference/                    # 外部技术基线、术语等参考资料
```

后续计划中的 App、Platform、Sync 等目录见
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

# 文档链接
pnpm docs:check

# 全部格式化/检查
pnpm format
pnpm format:check
```

Tauri Linux 原生编译额外需要 WebKitGTK 4.1、JavaScriptCoreGTK 4.1、GTK3 等系统依赖。当前 CI 的 `native-linux-check` 是规范验证路径。

## 测试要求

### 单元与静态检查

- React/Bridge：Vitest，路径 `apps/client/src/**/*.test.{ts,tsx}`。
- Rust：普通单元测试和 integration test。
- Playwright 文件必须放在 `apps/client/e2e/`，不得被 Vitest 收集。

### OpenSSH 协议检查

`pnpm test:ssh:smoke`：

- 构建隔离 Alpine/OpenSSH Fixture。
- 真实完成 TCP、SSH Handshake、Host Key 确认、密码认证、PTY 和命令输出。
- Fixture 凭据只能用于测试，不得替换为真实主机或真实密钥。

### Vault 检查

`pnpm test:rust` 必须覆盖：

- PIN Slot 创建、正确/错误 PIN、损坏 Slot。
- SQLCipher 重启解锁和 Credential 字段 AEAD。
- 数据库、WAL、Sidecar 与 Bootstrap 明文扫描。
- Schema migration 中断回滚。

`pnpm qa:native:xvfb` 还必须覆盖原生 Vault 创建、错误 PIN、锁定和重新解锁。

### Playwright E2E

`pnpm test:e2e` 验证标准浏览器工作流。Browser QA mode 是 UI 模拟，不打开网络连接；它不能替代 OpenSSH smoke。

### agent-browser 真实检查

UI/交互变更除了传统 E2E，还必须运行：

```bash
pnpm qa:browser
```

该检查必须：

- 实际点击和填写页面，而不只断言 DOM。
- 覆盖 Host Key Dialog、密码显示/隐藏、终端真实键盘输入和 Disconnect。
- 检查桌面与移动视口。
- 检查 Browser Errors。
- 生成并人工查看 `artifacts/agent-browser/` 中的截图和报告。

只看到脚本退出码为 0 不算完成；Agent 必须查看关键截图，确认不存在截断、遮挡、字体或响应式问题。录制视频需要系统 `ffmpeg`，没有时截图仍是必需证据。

### 文档

文档修改至少运行 `pnpm docs:check`，并确认 ADR/ExecPlan 索引和移动后的路径同步。

## 架构约束

以下约束来自当前设计基线；对应 Proposed ADR 在 Phase 0 后确认状态。

### 1. 秘密不得长期进入 WebView

- React 只持有展示模型、页面状态和终端数据。
- 密码、私钥、VMK、KEK、数据库密钥和长期 Token 留在 Rust/原生层。
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
5. 更新 threat model；当前正式文件将在 Phase 0 创建。

## 当前下一步

当前唯一活动计划是：

- [`0001-phase-0-technical-validation.md`](docs/execplans/active/0001-phase-0-technical-validation.md)

除非用户明确改变优先级，应先完成 Phase 0，而不是直接开发完整 UI、WebDAV 或高级脚本系统。

## 关键文件

| 内容 | 路径 |
| --- | --- |
| 根开发命令 | `package.json` |
| Rust Workspace | `Cargo.toml` |
| React 入口 | `apps/client/src/App.tsx` |
| Terminal Adapter | `apps/client/src/components/TerminalPane.tsx` |
| Browser/Native Bridge | `apps/client/src/lib/ssh-bridge.ts` |
| Tauri IPC | `apps/client/src-tauri/src/lib.rs` |
| SSH Core | `crates/anyssh-ssh/src/lib.rs` |
| OpenSSH Fixture | `tests/fixtures/openssh/` |
| Playwright E2E | `apps/client/e2e/connect-preview.spec.ts` |
| agent-browser 检查 | `scripts/qa/agent-browser-smoke.sh` |
| CI | `.github/workflows/ci.yml` |
| 产品目标 | `docs/project/product-brief.md` |
| 项目状态 | `docs/project/status.md` |
| 路线图 | `docs/project/roadmap.md` |
| 总体技术设计 | `docs/design/technical-architecture-2026.md` |
| ADR 索引 | `docs/adr/README.md` |
| ExecPlan 规范 | `docs/execplans/README.md` |
| 当前活动计划 | `docs/execplans/active/0001-phase-0-technical-validation.md` |
| 2026 技术基线 | `docs/reference/technology-baseline-2026.md` |
| 术语表 | `docs/reference/glossary.md` |

## 维护本文件

新增或修改根脚本、Workspace、CI、入口文件或测试层时，必须同步本文件。命令示例必须和 `package.json`、Cargo Workspace、CI 实际调用一致。

本文件只记录可从仓库验证的事实和真正影响 Agent 决策的规则。
