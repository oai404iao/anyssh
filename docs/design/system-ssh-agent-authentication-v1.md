# System SSH Agent Authentication v1

> 状态：设计中
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
- Fingerprint 使用 Public Key 的 SHA-256 Fingerprint。
- Comment 去除首尾空白、拒绝控制字符并设置长度上限。
- Comment 只用于展示，不参与引用或认证。
- 不返回 Public Key Blob、Private Key、签名或 Agent Socket Path。
- 空 Agent 返回空列表；Agent 不可用返回稳定分类错误。

## Credential 模型

Schema v5 扩展 Credential Kind：

```text
password
private_key
system_agent
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

## 外部参考

- [russh 0.62.4 AgentClient](https://docs.rs/russh/0.62.4/russh/keys/agent/client/struct.AgentClient.html)
- [OpenBSD ssh-agent(1)](https://man.openbsd.org/ssh-agent.1)
- [Microsoft OpenSSH Key Management](https://learn.microsoft.com/en-us/windows-server/administration/openssh/openssh_keymanagement)
