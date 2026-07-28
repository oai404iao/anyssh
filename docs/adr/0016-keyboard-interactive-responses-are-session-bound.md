# ADR-0016：Keyboard-interactive 响应是 Session-bound 临时秘密

- 状态：Proposed
- 日期：2026-07-28
- 决策人：项目维护者

## 背景

AnySSH 已支持 Password、Private Key 和 System Agent，但很多 SSH Server 通过
RFC 4256 `keyboard-interactive` 完成 OTP/MFA，或在 Public Key/Password 第一
因子成功后继续要求一次性验证码。

Server 可以发送多轮、多个 Prompt，并决定每个响应是否回显。Prompt 文本来自不
可信网络；响应可能是一次性验证码，也可能是可复用 Password。把响应持久化或按
Prompt 文本自动匹配 Saved Secret 会扩大 WebView 和远端 Server 对 Credential
的控制面。

## 决策

- 新增 `keyboard_interactive` Credential Kind。它只保存 Label 和 Username，
  不保存 Password、OTP Seed、响应模板或 Prompt 匹配规则。
- SQLCipher Schema v7 允许该 Kind 的 Secret/Passphrase 列为空；现有
  Password、Private Key 和 System Agent 记录继续要求完整 Record AEAD 字段。
- `SessionAuthentication::KeyboardInteractive` 直接启动 RFC 4256 流程。
- Password、Private Key 或 System Agent 第一因子只有在 Server 返回
  `partial_success = true` 且明确继续提供 `keyboard-interactive` 时，才进入
  第二因子；普通认证失败不得自动降级到 Interactive。
- Server Challenge 通过 metadata-only Session Event 发送：
  - Request ID。
  - Hop 和 Endpoint。
  - 受限 Name/Instructions。
  - 受限 Prompt 列表和每项 `echo`。
- Response 通过 Typed Tauri IPC 返回，并绑定 Session、Request ID、Hop 和当前
  Round。过期、重复、数量不匹配或超限响应必须拒绝。
- Keyboard-interactive Response 是 Session-bound 临时秘密：
  - 可以像 Quick Connection 临时 Password 一样短暂存在于当前 React 表单和
    当前 IPC Request。
  - 不得进入全局状态、Repository、日志、错误、遥测、截图或 Browser Fixture。
  - 提交、取消、断开、锁定和切页时必须立即清空。
- `echo = false` 使用掩码输入；`echo = true` 使用普通文本输入。AnySSH 不根据
  Prompt 文本猜测 Secret 类型，也不自动填入 Saved Password。
- 支持最多 8 个 Challenge Round、每轮 16 个 Prompt、单项和总响应大小上限。
  Server 的零 Prompt Round 自动返回空列表，但仍计入 Round 上限。
- Prompt Name/Instructions/Text 作为不可信纯文本处理，限制长度并移除危险
  控制字符；不得解释 HTML、ANSI、Markdown Link 或原生命令。
- Cancel、Timeout、Vault Lock、Session Close 和 UI 丢失均使当前认证失败；
  不缓存 Response 供下一次连接使用。
- Quick、Saved、Jump 和 Target 使用同一 Challenge/Response 状态机，每次只
  允许一个 Pending Authentication Request。

## 备选方案

- 把 OTP Seed 或固定 Response 保存为 Credential：扩大 Vault 数据模型并鼓励
  把第二因子与第一因子放在同一应用中，拒绝。
- 根据 `Password:`、`Verification code:` 等 Prompt 文本自动填充 Saved
  Password：Prompt 完全由 Server 控制，可诱导 AnySSH 泄露 Credential，拒绝。
- 第一因子普通失败后自动回退到 Keyboard-interactive：可能把强认证配置静默
  降级为 Password/Prompt，拒绝。
- 所有 Prompt 都使用平台原生 Dialog：多轮、多字段、Jump Hop、移动端和无障碍
  组合复杂，且 Quick 临时 Password 已允许局部 WebView 输入，v1 不采用。
- 只支持单 Prompt OTP：不兼容多字段 PAM/Duo/企业 Challenge，拒绝。

## 后果

### 正面

- 支持纯 Keyboard-interactive 和 Public Key/Password/Agent + OTP。
- Saved Credential 不包含 OTP Seed 或可复用 Interactive Response。
- Request/Hop/Round 绑定避免延迟 Response 应用到错误 Jump 或后续 Session。
- 不会因 Server 提供较弱替代方法而自动降级既有 Credential。

### 代价与风险

- 当前 Session 的临时 Response 会短暂存在于 WebView 和 russh 内存；被攻陷的
  当前 Renderer/进程仍可能读取它。
- Schema 升级到 v7，需要重建 Credential CHECK/Nullable Constraint 及其引用表。
- SSH Core、Tauri Session Registry 和 React 都需要新的有界 Pending Request
  状态。
- Windows Native QA 需要可控的 Keyboard-interactive Server Fixture；真实 PAM
  兼容性主要由 Linux OpenSSH Fixture 证明。

## 验证

- Schema v6 -> v7 成功、重启和中断回滚。
- Interactive Credential Summary/IPC 不含 Secret 字段。
- 纯 Keyboard-interactive、错误/正确 OTP、多 Prompt、多 Round、零 Prompt。
- Password/Private Key/System Agent Partial Success 后继续 OTP。
- 普通第一因子失败不回退。
- Request ID、Hop、Round、Prompt Count、Response Count 和大小上限。
- Cancel、Timeout、Disconnect、Vault Lock 和 Stale Response Fail Closed。
- Browser、X11、Wayland、Windows、OpenSSH PAM、Android/Linux Container 和
  同 Commit CI。

## 相关文档

- Design：[Keyboard-interactive Authentication v1](../design/keyboard-interactive-authentication-v1.md)
- ExecPlan：[Keyboard-interactive and OTP](../execplans/active/0006-keyboard-interactive-and-otp.md)
- ADR：[ADR-0006](0006-secrets-stay-out-of-webview.md)
- ADR：[ADR-0010](0010-saved-host-plans-resolve-in-rust.md)
- ADR：[ADR-0015](0015-known-host-trust-is-endpoint-scoped.md)
- Supersedes：
- Superseded by：
