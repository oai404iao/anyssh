# AnySSH Threat Model v1

> 状态：Phase 1 Baseline
> 日期：2026-07-28
> 范围：当前 Tauri/React Client、Rust Core、Vault、SQLCipher Repository、
> Group Inheritance、russh Session、Native Encrypted Private Key Import 和
> Endpoint-scoped Known Host Trust、QA/Build Evidence。

## 1. 安全目标

AnySSH 当前必须保持以下不变量：

1. VMK、KEK、SQLCipher Key、Record Key、Private Key 和 Passphrase 不返回
   WebView。
2. 保存的 SSH 认证通过不透明 Credential ID 在 Rust 内解析。
3. Group、Host 和 Jump Route 只保存 ID 引用，不复制 Credential Secret。
4. 锁定 Vault 后，DB Actor 不继续持有解锁后的 `LocalVault`。
5. 未确认或变化的 Host Key 不得静默放行。
6. 大量远端输出、Route 展开和 DB Command 必须有明确上限。
7. 数据库、WAL、Bootstrap、日志和 QA Evidence 不包含测试 Secret 明文。
8. QA-only Debug 能力不得进入 Canonical/Release 配置。

## 2. 资产

| 资产 | 保护目标 |
| --- | --- |
| VMK、KEK、数据库 Key、Record Key | 机密性、完整性、最短可用生命周期 |
| Password、Private Key、Passphrase | 不持久进入 WebView、日志或明文文件 |
| System SSH Agent Signing Capability | 只允许用户选择的 Fingerprint、无 Key 导出、无隐式 Forward |
| Host Key Fingerprint | 防止 MITM 和静默信任变化 |
| Group/Host/Route/Credential 图 | 引用完整性、确定顺序、无循环 |
| Vault Bootstrap 和 Schema | 版本完整性、可恢复迁移、拒绝静默覆盖 |
| Terminal 输入输出 | 有界内存、正确 Session 归属、无跨 Session 注入 |
| QA/CI Artifact | 不包含宿主 Secret、测试 Secret 或不必要环境信息 |

## 3. 信任边界

```text
Untrusted/less-trusted WebView
  -> Typed Tauri IPC
    -> ApplicationCore
      -> bounded DB Actor
        -> LocalVault / SQLCipher / Record AEAD

ApplicationCore
  -> anyssh-ssh
    -> untrusted network / SSH server / Jump Hosts

Native Picker
  -> Rust-only selected Path/URI
    -> bounded validation
      -> Credential Repository

Native Secure Passphrase Prompt
  -> Zeroizing<String>
    -> encrypted Key validation
      -> independent Key/Passphrase Record AEAD

System SSH Agent Socket / Named Pipe
  -> Rust-only identity enumeration
    -> exact SHA-256 fingerprint selection
      -> external signing
```

- React/WebView 只属于展示与交互边界，不能成为 Secret Repository。
- Tauri Command 只校验、转换并调用 ApplicationCore。
- DB Actor 是同步数据库、Argon2id 和 Vault 文件系统操作的串行边界。
- SSH Server、Jump Host、网络和未来 WebDAV 服务均视为不可信。
- 本地操作系统、管理员、内核和已攻陷的当前用户 Session 不在本版本可完全防御
  的范围内。

## 4. 攻击者与假设

当前考虑：

- 能读取被盗设备磁盘或备份的离线攻击者。
- 能控制 SSH Server、Jump Host 或网络路径的远端攻击者。
- 能触发 WebView XSS、恶意页面状态或异常 IPC Payload 的攻击者。
- 能提供恶意 Private Key 文件、Symlink、特殊文件或超大文件的本地输入。
- 能触发进程中断、Schema Migration 中断和重复并发命令的故障环境。
- 能读取公开 CI Artifact 的观察者。

当前不承诺防御：

- 已控制内核、管理员权限或 AnySSH 进程内存的攻击者。
- Vault 解锁期间的物理内存取证。
- 被恶意替换且仍通过供应链校验的编译器或平台 Runtime。
- iOS Runtime；当前没有 macOS/Xcode Evidence。

## 5. 主要威胁与控制

| ID | 威胁 | 当前控制 | 剩余风险/后续 |
| --- | --- | --- | --- |
| T-01 | WebView 获得保存的 Password/Key | Summary-only DTO、Credential ID、`deny_unknown_fields`、Rust-only Resolve | Quick Connection 临时密码仍短暂存在于 WebView；Secret Reveal 尚未实现 |
| T-02 | VMK/DB Key 跨 IPC 或日志泄漏 | Actor-owned `LocalVault`、Zeroizing、无 Serialize 类型、日志脱敏 | 解锁期间 Rust 进程内存仍包含必要 Key |
| T-03 | 被盗数据库泄漏业务数据 | SQLCipher 整库加密、Credential Record AEAD、随机 VMK/HKDF | Platform/Recovery Slot 尚未实现；PIN 仍面对离线猜测 |
| T-04 | PIN 直接成为数据库 Key | Argon2id KEK 只解包随机 VMK | 尚无平台级重试限制或硬件 Slot |
| T-05 | Migration 中断损坏或丢失数据 | `IMMEDIATE` Transaction、中断回滚、旧数据恢复测试 | 后续每次 Schema 变更必须继续提供版本和恢复测试 |
| T-06 | Host Key MITM/轮换被静默接受 | Accepted ADR-0015：Endpoint-scoped SQLCipher Trust、persist-before-continue、Request ID、SHA-256 Set 和 Changed-Key 硬阻断 | OpenSSH 文件导入/导出、Host Certificate 和自动 Rotation 后续 |
| T-07 | 延迟 Host Key 决策应用到错误 Hop | Request ID + Hop + Endpoint 绑定；过期 Request 拒绝 | UI 必须继续展示准确 Hop |
| T-08 | Group/Jump Route 循环或膨胀导致 DoS | Group Parent 与 Effective Host Route 全图检测、最多 32 层 Group/32 Jump、Runtime 重验 | 深层故障归属仍需保持逐 Hop 测试 |
| T-09 | 大输出耗尽内存 | 64 项 Core Queue、8 Chunk WebView Credit、xterm Ack、SSH Window Flow Control | Scrollback 与未来多 Tab 需要全局预算 |
| T-10 | DB 并发和关闭死锁 | 单 OS Thread、16 项有界 Queue、oneshot、关闭 Sender 后 Join | 长操作仍会串行阻塞，需继续监控可取消性 |
| T-11 | WebView 指定任意文件或读取 Key | Native Picker 在 Rust 内发起；IPC 无 Path/Key/Passphrase；Linux/Windows 真实 Picker Evidence | Windows Reparse Point 和移动 Content URI 尚未实现 |
| T-12 | Symlink/FIFO/超大 Key 文件 | 打开前类型检查、1 MiB 上限、UTF-8、Unix `O_NOFOLLOW`、russh Decoder | Windows Reparse Point 的专项恶意 Fixture 尚未覆盖 |
| T-13 | QA Artifact 泄漏 Secret/Token | `env -i`/白名单环境、截图前清空 Secret、Vault 明文扫描、不上传 Vault/Profile | Agent 必须继续人工检查截图和 Error Log |
| T-14 | QA CDP 成为 Release 调试后门 | 独立 `tauri.windows-qa.conf.json`、仅 Debug Smoke 使用、Loopback Port、Canonical Config 无 CDP | QA Debug EXE 不得作为发布 Artifact 分发 |
| T-15 | 恶意 WebDAV 删除、回滚或分叉数据 | 计划使用加密不可变 Operation、Snapshot、ETag CAS | Sync 尚未实现；ADR-0004 保持 Proposed |
| T-16 | 任意本地脚本获得文件/网络/Secret | MVP 禁止任意 Shell、`eval` 和第三方插件 | Runbook Engine 尚未实现，需要 Phase 1/后续测试 |
| T-17 | WebView、日志或恶意配置滥用系统 Agent | IPC 不接受 Socket/Pipe/Key/签名；最多 64 Identity；Credential 精确 Fingerprint；不自动回退；Agent Forwarding 关闭；依赖 `log` 静态上限为 Info | Agent 本身和已解锁用户 Session 仍是外部信任边界；Flatpak/确认策略待验证 |
| T-18 | 加密 Key Passphrase 经 WebView、外部进程或 Prompt Buffer 泄露 | Accepted ADR-0014：进程内 GTK/Windows Credential UI、无 Passphrase IPC、`Zeroizing<String>`、三次上限、失败不落库、Artifact 明文扫描 | Toolkit/OS 和解锁进程内存仍短暂持有 Secret；Android/iOS Adapter 尚未实现 |
| T-19 | 被攻陷的 WebView 删除 Known Host 后自动接受 MITM Key | Accepted ADR-0015：Forget 只提交 ID，由 ApplicationCore 解析并通过 WebView 外 Linux/Windows 原生确认后删除；Changed-Key 无接受入口 | Android/iOS 原生确认 Adapter 尚未实现；OS/Toolkit Prompt 仍是平台信任边界 |
| T-20 | 恶意 Server Prompt 或延迟 OTP Response 泄露 Saved Secret/应用到错误 Hop | Accepted ADR-0016：不自动填充 Saved Secret；Request-scoped 局部表单、Typed IPC、`Zeroizing<String>`、Request/Hop/Round 绑定、Count/Size/Timeout 上限和普通失败不降级；OpenSSH PAM/X11/Wayland/Windows Evidence | 当前 Renderer 被攻陷时仍可能读取本次临时 Response |
| T-21 | 多 Tab 把 Output/Input/Host Key/OTP 路由到错误 Session，或关闭 UI 后留下孤儿连接 | Proposed ADR-0017 的本地实现：Tab ID/Generation 与 Rust Session ID 分离；每 Tab Event/Data Channel、Mounted xterm/Ack/Pending Request；Late Return Disconnect；Close、Channel Loss、Vault Lock 和 App Exit Fail Closed | Browser、X11、Wayland 已覆盖 Close-during-connect、同时 Challenge、非活动 4 MiB 输出和双 Session Vault Lock；Windows Multi Tab Evidence 等待同 Commit CI |

## 6. 平台结论

- Linux X11：真实 Tauri/WebKitGTK、Vault、加密 Key GTK Prompt/错误重试、
  Native Picker、`SSH_AUTH_SOCK` Identity UI、Durable TOFU、原生 Forget、
  Host Key Rotation 硬阻断、OpenSSH PAM Keyboard-interactive、双 SSH Session、
  非活动 Tab 4 MiB 输出、单 Tab Close 和双 Session Vault Lock 已验证。
- Linux Wayland：无 `DISPLAY`、Weston、IBus/libpinyin、xterm、SSH 和
  OpenSSH PAM Keyboard-interactive 已验证；一个 Connected Tab 与一个
  Challenge Tab 的并发路由和单 Tab Close 也已验证。
- Windows：真实 EXE/WebView2、非零窗口句柄、Vault/Repository、Durable TOFU、
  原生 Forget、重启恢复和 Changed-Key 硬阻断已验证。Run `30360000884` 还覆盖
  Native Picker、Credential UI、加密 Key SSH、System Agent Named Pipe、
  controlled russh Keyboard-interactive、standalone OpenSSH Host Key Rotation、
  远端 Marker 和明文扫描。
- Android：ARM64 Debug APK、Rust Core 和 bundled SQLCipher 构建已验证；Runtime
  与 Content URI 尚未验证。
- iOS：因无 macOS/Xcode 环境而明确延期。

当前 xterm.js 使用默认 Renderer，而不是 WebGL Addon。Phase 0 因此验证的是稳定
非 WebGL 路径；启用 WebGL 及 Context Lost 回退属于后续显式里程碑。

## 7. 验证映射

```bash
pnpm lint
pnpm test
pnpm test:ssh:smoke
pnpm test:e2e
pnpm qa:browser
pnpm qa:native:xvfb
pnpm qa:native:wayland
pnpm qa:native:windows   # Windows only
pnpm check:container:linux
pnpm check:container:android
```

关键测试包括：

- Vault PIN/损坏 Slot/重启/明文扫描和 Migration 回滚。
- Credential/Group/Host/Route Summary 脱敏、三态解析、引用完整性和循环检测。
- System Agent Identity 上限、Fingerprint 选择、错误 Identity、Direct/Jump
  外部签名和 IPC 脱敏。
- Private Key Import 文件类型、大小、编码、Symlink、加密状态、空/错误
  Passphrase、取消、三次上限和成功 SSH。
- Endpoint-scoped Known Host Migration、First-writer-wins、persist failure、
  Host Key 变化、原生 Forget、两 Jump Route、取消、超时和 4 MiB 背压。
- Schema v7 Interactive Credential、Partial-success、多 Round/Prompt、
  Request/Hop/Timeout/Cancel、OpenSSH PAM 和 Response 明文扫描。
- Browser、X11、Wayland/IME、Windows WebView2 和 Android Build Evidence。

Group Feature Commit `ece4fe7` 的 Run `30279500562` 全部九个 Job 通过；关键
Desktop/Mobile、X11、Wayland 和 Windows 截图及 Error Log 已人工检查。

Encrypted Key Prompt Head `dac51ffd079d56ab1d7f7a5837d6bf6b89b1c333`
的 Run `30325359607` 全部九个 Job 通过；Linux/Windows Native Prompt、
Browser Error Log、OpenSSH Marker、重启和 Artifact 明文扫描已人工检查。

Known Host Head `a75da9cf6d4ba73f8b93257c683fb97ad2c0b90f` 的 Run
`30344638562` 全部九个 Job 通过；Browser、X11、Wayland、Windows、
Android/Linux Build Hash、Error Log、SQLCipher 明文扫描和测试 Secret 已人工
复核。

## 8. 复审触发条件

以下变化必须更新本文件，并在需要时新增 ADR：

- VMK/Key Slot、KDF、SQLCipher Key 或备份格式变化。
- Secret Reveal、剪贴板、Key Export 或加密 Key Passphrase Prompt。
- Group 继承、Known Host、Proxy、Forward 或 Agent 引入新的 Secret/权限边界。
- WebDAV Operation/Snapshot/HLC 格式。
- 启用 WebGL、第三方脚本 Runtime、插件或远程内容。
- 新平台 Runtime Evidence 或当前假设被反例推翻。
