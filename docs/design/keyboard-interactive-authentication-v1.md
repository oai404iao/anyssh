# Keyboard-interactive Authentication v1

> 状态：已实现；ADR-0016 已接受
> 日期：2026-07-28

本文定义 Phase 1 Desktop MVP 的 RFC 4256 Keyboard-interactive/OTP
Authentication、Partial-success 第二因子、Session-bound Response 和产品 UI。
长期决策见 Accepted ADR-0016。

## 目标

- 支持只使用 Keyboard-interactive 的 SSH Server。
- 支持 Password、Private Key 或 System Agent 第一因子后的 OTP/MFA。
- 支持多轮、多 Prompt、`echo=true/false` 和零 Prompt Round。
- Quick、Saved、Jump 和 Target 共用同一有界状态机。
- Response 不持久化、不自动匹配 Saved Secret。
- Linux 使用真实 OpenSSH PAM Fixture 验证协议兼容。

## 非目标

- 保存 TOTP/HOTP Seed 或在 AnySSH 内生成验证码。
- 自动识别 `Password:`/`OTP:` 并填充 Credential。
- Duo Push/WebAuthn/FIDO2 专用协议或外部浏览器流程。
- SSH Password Change Request。
- 把所有 Prompt 改为平台原生 Dialog。
- Keyboard-interactive Device/Submethod 产品配置。

## 安全不变量

1. Server Prompt 是不可信纯文本，不能执行 HTML、ANSI 或原生命令。
2. Response 只能用于创建它的 Session/Request/Hop/Round。
3. 普通第一因子失败不能自动回退到 Keyboard-interactive。
4. 只有 `partial_success` 且 Server 继续提供该方法时才进入第二因子。
5. Interactive Credential 不保存 OTP Seed、Response 或 Prompt Rule。
6. Response 不进入前端全局状态、Repository、日志、错误或 QA Evidence。
7. 超限、过期、重复、数量不匹配、取消和 UI 丢失均 Fail Closed。
8. Vault Lock 必须取消 Pending Challenge 并断开 Session。

## Credential 与 Schema v7

新增：

```rust
enum CredentialKind {
    Password,
    PrivateKey,
    SystemAgent,
    KeyboardInteractive,
}

enum CredentialSecret {
    Password { password: Zeroizing<String> },
    PrivateKey {
        private_key: Zeroizing<String>,
        passphrase: Option<Zeroizing<String>>,
    },
    SystemAgent {
        identity_fingerprint_sha256: Zeroizing<String>,
    },
    KeyboardInteractive,
}
```

Schema v7 重建 `credentials`，允许 `keyboard_interactive` 的
`secret_nonce/secret_ciphertext` 和 Passphrase 列全部为空；其他 Kind 继续要求
Secret Nonce/Ciphertext，只有 Private Key 可以有 Passphrase。

Interactive Credential Summary 只包含 ID、Label、Username 和 Kind。Host/Group
继续通过不透明 Credential ID 引用；Saved Host IPC 仍只提交 Target Host ID。

## SSH Core 状态机

```text
Host Key accepted
  -> Primary Authentication
    -> Success
    -> Failure(partial=false): fail
    -> Failure(partial=true, keyboard-interactive offered)
         -> Keyboard-interactive rounds

KeyboardInteractive Credential
  -> Keyboard-interactive rounds directly
```

每个 Round：

```text
russh InfoRequest
  -> validate/sanitize/bound challenge
    -> SessionEvent::AuthenticationChallenge
      -> wait for matching response
        -> russh AuthInfoResponse
          -> Success / Failure / next InfoRequest
```

`SessionAuthentication` 增加 `KeyboardInteractive`。现有三种认证返回内部
`PrimaryAuthenticationOutcome`，保留 `remaining_methods` 和 `partial_success`，
不再只检查 `AuthResult::success()`。

## Challenge Event

```rust
struct AuthenticationChallengeInfo {
    request_id: u64,
    hop: SessionHop,
    endpoint: SshEndpoint,
    name: String,
    instructions: String,
    prompts: Vec<AuthenticationPrompt>,
}

struct AuthenticationPrompt {
    text: String,
    echo: bool,
}
```

建议上限：

- 最多 8 Round。
- 每轮最多 16 Prompt。
- Name 1 KiB。
- Instructions 4 KiB。
- 单 Prompt 1 KiB。
- 单 Response 16 KiB。
- 每轮 Response 总量 64 KiB。
- 等待用户响应 120 秒。

零 Prompt Round 不打开 UI，SSH Core 自动发送空 Response，但仍计入 Round。

Challenge 类型自定义 Debug，只记录 Request ID、Hop、Endpoint、Round 和 Prompt
数量，不记录完整 Prompt/Response。

## Session Control

新增有界通道和 Pending Request：

```text
SessionControl::respond_authentication(
    request_id,
    Option<Vec<Zeroizing<String>>>,
)
```

- `Some(responses)` 表示提交。
- `None` 表示取消。
- Control 先原子消费 Pending Request，再发送 Response。
- SSH Worker 校验 Response 数量和大小后才调用 russh。
- Stale/Duplicate Request 返回稳定错误。
- Disconnect/Drop/Lock 唤醒等待并返回 Cancelled。

Host Key 和 Authentication 使用独立 Request Namespace/Channel，但同一 Session
的协议顺序保证二者不会同时 Pending。

## Tauri IPC

新增：

- `ssh_respond_authentication`
- `SshEvent::AuthenticationChallenge`

Request 只包含：

```text
sessionId
requestId
responses | null
```

Rust 侧使用 `deny_unknown_fields`，立即把 Response 转入 `Zeroizing<String>`。
IPC/Event 不包含 Credential ID、Saved Password、Private Key、Agent Payload 或
Host Public Key。

## React UI

- Challenge 使用独立、按 Request ID 重建的局部表单 Modal，标题显示 Target
  或 Jump Index。
- 显示受限 Name/Instructions 和 Prompt Label。
- `echo=false` 使用 Password Input；`echo=true` 使用 Text Input。
- 主操作为 `Continue`，次操作为 `Cancel authentication`。
- Submit 后先清空本地 Response，再等待下一轮/认证结果。
- Cancel、Disconnect、Lock、Session Close、Route Change 和 Unmount 时清空。
- 不使用全局 Store、Local Storage、Autocomplete History 或 Browser Fixture
  Secret。
- 多 Prompt 在 Desktop/Mobile 都必须完整可滚动，Focus 顺序稳定。

Browser QA 使用固定 Prompt 元数据和占位输入，不把测试 Response 写入 Snapshot、
Console 或 Report。

## OpenSSH PAM Fixture

Linux Fixture 使用 Alpine `openssh-server-pam`、`UsePAM yes` 和
`KbdInteractiveAuthentication yes`。测试 PAM Stack 通过
`pam_exec.so expose_authtok type=auth` 验证固定一次性 Token，并使用
`pam_permit.so` 提供 Account/Session。

至少覆盖：

- `AuthenticationMethods keyboard-interactive:pam`。
- `AuthenticationMethods publickey,keyboard-interactive:pam`。
- 错误 Token、正确 Token和第二次连接。
- Direct、Jump Target 和 Jump Hop 中至少两种归属。

多 Prompt、多 Round、Echo 和异常上限由受控 russh Test Server 覆盖，因为普通
PAM Stack 不稳定表达全部协议边界。

## 失败与错误

- Keyboard-interactive authentication failed。
- Authentication challenge expired。
- Authentication challenge is invalid。
- Authentication challenge is too large。
- Authentication response count does not match。
- Authentication response is too large。

错误不得包含 Prompt Response、Saved Secret、PAM 内部消息或底层 Packet。

## 验证

### Storage/Application

- Schema v6 -> v7 Migration、重启、中断回滚和引用保持。
- Interactive Credential CRUD、Summary/Debug/IPC 脱敏。
- Saved Host/Group/Route 解析为 Interactive Authentication。

### SSH

- 纯 Interactive、多 Prompt、多 Round、零 Prompt、Echo。
- Password/Private Key/Agent Partial Success。
- 普通失败不降级。
- Stale/Duplicate/Count/Size/Timeout/Cancel。
- Jump 1、Jump 2 和 Target Prompt 归属。

### UI/Native

- Vitest/Playwright/agent-browser Desktop/Mobile。
- X11 真实 OpenSSH PAM OTP、Known Host、Encrypted Key、Agent 和 4 MiB 回归。
- Wayland/IBus 回归，并至少完成一次 Interactive Challenge。
- Windows 真实 EXE/WebView2 使用受控 Test Server 完成 Challenge。
- Android/Linux Container、Workspace 和同 Commit CI。

## 当前实现结果

- Schema v7、Interactive Credential CRUD/Resolve、Quick/Saved/Jump/Target
  Application Boundary 已实现。
- russh Core 已覆盖多 Round、多 Prompt、`echo`、零 Prompt、Count/Size/
  Timeout/Cancel、Stale Request 和普通失败不降级。
- Alpine OpenSSH PAM 已覆盖纯 Interactive、Password/Private Key/System Agent
  Partial-success + OTP、Saved Host，以及 Jump 1、Jump 2 和 Target Prompt
  归属。
- Browser、Playwright、agent-browser、X11 与无 `DISPLAY` Wayland/IBus Native
  Evidence 已通过并检查。
- Head `0ceb5b332967a9b1fc7fdf73967ae49bf44505d7` 的同 Commit GitHub Actions
  Run `30360000884` 九个 Job 全部通过。Windows 真实 EXE/WebView2 已使用
  controlled russh Server 完成 masked Challenge、Interactive Credential 重启、
  远端 Marker 和 Response/Vault/Evidence 扫描。
- Browser、X11、Wayland 和 Windows Challenge/Connected Screenshot、Error Log
  与 Android/Linux/Windows Build Hash 已人工检查；ADR-0016 已接受。

## 相关文档

- [ADR-0016](../adr/0016-keyboard-interactive-responses-are-session-bound.md)
- [Threat Model v1](threat-model-v1.md)
- [Credential Repository v1](credential-repository-v1.md)
- [Saved Host Connection Plan v1](saved-host-connection-plan-v1.md)
- [Known Host Repository v1](known-host-repository-v1.md)
- [ExecPlan 0006](../execplans/completed/0006-keyboard-interactive-and-otp.md)
