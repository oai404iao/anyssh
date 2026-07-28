# System SSH Agent Authentication v1

> 状态：已实现并通过同 Commit CI
> 日期：2026-07-27
> 外部基线核验日期：2026-07-27

## 目标

让 Linux 和 Windows Desktop 用户选择系统 SSH Agent 中的一个确定 Identity，
并通过 Saved Host Credential ID 完成认证，同时保证 Private Key、Passphrase
和签名 Payload 不进入 WebView、Vault 或日志。

## 平台范围

| 平台 | v1 入口 |
| --- | --- |
| Linux/Unix Desktop | `SSH_AUTH_SOCK` 指向的 Unix Domain Socket |
| Windows | OpenSSH Agent Named Pipe `\\.\pipe\openssh-ssh-agent` |
| Pageant | 后续兼容适配器 |
| Android/iOS | 明确 Unsupported |
| Browser QA | metadata-only 内存模拟 |

Linux Agent Socket 来自启动 AnySSH 的受信 Session Environment。Windows 使用
OpenSSH Authentication Agent Service；v1 不扫描任意 Named Pipe，也不允许
WebView 提交 Socket/Path/Pipe 名称。

## Identity 枚举

Rust 命令：

```text
credential_list_system_agent_identities()
  -> [
       {
         algorithm,
         fingerprint_sha256,
         comment
       }
     ]
```

约束：

- 最多读取 64 个 Identity。
- v1 只返回普通 Public Key Identity；OpenSSH Certificate Identity 暂时跳过，
  等 Certificate 产品能力单独实现。
- Fingerprint 使用 Public Key 的 SHA-256 Fingerprint。
- Comment 去除首尾空白、拒绝控制字符并设置长度上限。
- Comment 只用于展示，不参与引用或认证。
- 不返回 Public Key Blob、Private Key、签名或 Agent Socket Path。
- 空 Agent 返回空列表；Agent 不可用返回稳定分类错误。
- russh Agent Client 的依赖级 Debug Frame/Sign Payload 日志通过统一 `log`
  `max_level_info`/`release_max_level_info` 编译上限关闭；AnySSH 自身继续使用
  脱敏后的 `tracing`。

## Credential 模型

Schema v5 首次增加 `system_agent`；当前 Schema v7 的完整 Credential Kind：

```text
password
private_key
system_agent
keyboard_interactive
```

System Agent Credential：

```rust
SystemAgent {
    identity_fingerprint_sha256: String,
}
```

Label 与 Username 继续属于 Credential Summary。Fingerprint 作为确定认证选择器
进入现有 Record AEAD Payload；它不是 Private Key，也不被当作 Secret Reveal
能力返回。

Schema v4 -> v5 Migration 在单个 Transaction 中重建 Credential Kind CHECK
Constraint，保持现有 Ciphertext、Nonce、ID 和引用不变，并支持中断回滚。

## 运行时数据流

```text
WebView submits Saved Host ID
  -> DB Actor resolves Credential ID
    -> SystemAgent(fingerprint)
      -> anyssh-ssh opens platform Agent endpoint
        -> request at most 64 identities
          -> exact SHA-256 fingerprint match
            -> russh authenticate_publickey_with
              -> Agent signs
```

- 不匹配时不尝试其他 Identity。
- RSA Identity 使用 Server `server-sig-algs` 选择 SHA-2 Hash。
- Agent 拒绝签名、Identity 消失或认证失败时返回当前 Hop 的稳定错误。
- Jump Host 和 Target 可以分别使用 Password、Private Key 或 System Agent。
- Agent Connection 仅在认证阶段存在，不启用 Agent Forwarding。

## UI

Credential 页面增加 `New system agent`：

1. 用户输入 Label 和 Username。
2. Rust 枚举本机 Agent Identity。
3. UI 显示 Algorithm、SHA-256 Fingerprint 和可选 Comment。
4. 用户选择一个 Identity 后创建 Credential。

Credential Summary 只显示 Kind 为 `System Agent`；Host/Group 继续引用不透明
Credential ID。Browser QA 使用固定 Public Metadata，不访问真实 Agent。

## 错误分类

- System SSH Agent is unavailable.
- System SSH Agent has no identities.
- Selected SSH Agent identity is no longer available.
- System SSH Agent refused the signing request.
- SSH Agent authentication failed.
- System SSH Agent is unsupported on this platform.

错误不得包含 Socket Path、Named Pipe、Public Key Blob、签名 Payload 或底层
Agent Frame。

## 验证

- Agent Identity DTO/Debug/JSON 不含 Key 或签名 Payload。
- Schema v4 -> v5 成功、重启、中断回滚和明文扫描。
- Linux 真实 `ssh-agent` 加载 Ed25519 Key 后连接 OpenSSH Fixture。
- Password Jump -> Agent Target 与 Agent Jump -> Private Key Target。
- 错误 Fingerprint、空 Agent、Agent 退出和拒绝认证按 Hop 归属。
- Windows QA 启动 OpenSSH Agent Service、加载 Fixture Key，并由真实 EXE/
  WebView2 连接 Fixture。
- Frontend/Playwright/agent-browser 覆盖 Identity 选择和 metadata-only UI。
- Android/Linux Container 和 Windows Build 回归。

本地证据：

- `pnpm test:ssh:smoke` 已通过真实 `ssh-agent` 验证 Direct、Password Jump ->
  Agent Target 和 Agent Jump -> Private Key Target。
- X11：
  `artifacts/native-xvfb/smoke-1785168783-1314017`；真实 Tauri UI 枚举
  `SSH_AUTH_SOCK` Identity、创建 System Agent Credential，并完成原有 Vault、
  Native Picker、SSH 与 4 MiB 回归。
- agent-browser：
  `artifacts/agent-browser/smoke-1785169032`；Desktop/Mobile Credential UI 和空
  Browser Error Log 已人工检查。
- Wayland：
  `artifacts/native-wayland/smoke-1785169077-1339570`；IME/SSH 回归通过。
- Android Host APK SHA-256：
  `124bc46dc0963bec4a972c4583b1159527b4be18cf2d6a2d4eddc086435ff5b0`。
- Linux Container ELF SHA-256：
  `1d94cd2fde8ba2e7b148b727ca3e4a990a18560dc080f5655f4241f8cfa6fb7e`。
- Android Container APK SHA-256：
  `28950dde0621e49976a9ddee949c2fb253b574e8c1d73eee10ca00356914802f`。

同 Commit 远端证据：

- Head `123e684c9328b87f6001a10de48e2c3bed8134e6` 的 Run `30287139254`
  全部九个 Job 通过。
- Windows `smoke-20260727-170152-4388` 使用 OpenSSH Agent Named Pipe 和
  standalone `sshd.exe` 完成真实 EXE/WebView2 SSH；WebView2 为
  `Edg/150.0.4078.65`，CDP 1.3，Browser Error Log 为空。
- 同 Run 的 X11 `smoke-1785171904-6176`、Wayland
  `smoke-1785171978-7509` 和 agent-browser `smoke-1785171550` 已人工检查。
- Android APK、Linux ELF 和 Windows EXE SHA-256 分别为
  `07acb10103410efef349659cfbefd75e8608f2dd957ee3095e99aef6f9ccf45a`、
  `943a30b9ecf9df4209f542c49026e5cf3fb6fa4780e00c7e013ea5348c3c08bb`、
  `adb2382bee82ec0c898f249b3766c10ee10269275b6cd311f5af2b6a04e86410`。

## 外部参考

- [russh 0.62.4 AgentClient](https://docs.rs/russh/0.62.4/russh/keys/agent/client/struct.AgentClient.html)
- [OpenBSD ssh-agent(1)](https://man.openbsd.org/ssh-agent.1)
- [Microsoft OpenSSH Key Management](https://learn.microsoft.com/en-us/windows-server/administration/openssh/openssh_keymanagement)
