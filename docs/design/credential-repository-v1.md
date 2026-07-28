# AnySSH Credential Repository v1

> 状态：已实现
> 日期：2026-07-28

本文定义在 SQLCipher Schema v2 引入、并由当前 Schema v7 继续使用的
Vault-backed Credential Repository，以及 SSH Credential ID 解析边界。当前产品
已实现 metadata-only Credential 管理 UI 和 Rust-owned Native File Picker；本文
仍不定义 Secret Reveal/Export。加密 Key Passphrase Prompt 由
[Native Encrypted Private Key Passphrase v1](native-encrypted-private-key-passphrase-v1.md)
定义，Linux/Windows Desktop 已实现。

## 安全目标

- Credential 摘要只包含 ID、Label、Username 和 Kind。
- Password、Private Key、Key Passphrase 和 System Agent Fingerprint Selector
  使用 Record AEAD 二次加密；Keyboard-interactive Credential 不含 Secret
  Payload。
- Private Key 明文不得出现在 React State、Tauri IPC Request、日志或错误中。
- SSH Connect Request 只携带 Credential ID；Rust 解密后直接把
  `Zeroizing<String>` 移交给 `anyssh-ssh`。
- Vault Locked 时所有 Repository Command 必须失败。

## Rust 数据流

```text
React Connect Request
  -> { kind: "credential", credentialId }
    -> Tauri typed conversion
      -> anyssh-app ApplicationCore
        -> DatabaseActorHandle::resolve_credential(id)
          -> Actor-owned LocalVault / SQLCipher
            -> ResolvedCredential (Rust-only, redacted Debug)
              -> anyssh_ssh::SessionAuthentication
```

Private Key 的创建与更新只提供给 Rust Trusted Service。产品导入由
`credential_import_private_key` Command 在 Rust 内打开 Native File Picker；
Command 不接受 WebView 指定的文件路径，选中的 Path 和 Key 内容不返回
WebView。未加密 Key 直接验证；加密 Key 通过 Native Secure Prompt 获取
Passphrase，详细边界见
[原生私钥导入 v1](native-private-key-import-v1.md) 和
[Native Encrypted Private Key Passphrase v1](native-encrypted-private-key-passphrase-v1.md)。

Password Credential 可以通过 Typed IPC 创建或更新，因为用户输入密码本来就会
短暂经过 WebView；IPC Adapter 必须立即把它移动到 `Zeroizing<String>`，不得保存
到前端全局状态或日志。

System Agent Credential 由 Rust 枚举当前平台 Agent 的普通 Public Key Identity。
WebView 只选择 SHA-256 Fingerprint；Socket/Pipe Path、Public Key Blob、Private
Key 和签名 Payload 不进入 IPC。详细边界见
[System SSH Agent Authentication v1](system-ssh-agent-authentication-v1.md)。

Keyboard-interactive Credential 只保存 Label 和 Username。OTP Seed、Response、
Prompt Rule 或 Saved Password 匹配规则不得写入 Repository；每轮 Response 只在
当前 Session 的局部 React 表单、Typed IPC 和 Rust `Zeroizing<String>` 中短暂
存在。详细边界见
[Keyboard-interactive Authentication v1](keyboard-interactive-authentication-v1.md)。

## Schema v2 引入的 Credential 表

Schema v2 保留当时的 `vault_meta` 和兼容 `hosts` 表，并新增：

```sql
CREATE TABLE credentials(
    id TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    username TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('password', 'private_key')),
    secret_nonce BLOB NOT NULL,
    secret_ciphertext BLOB NOT NULL,
    passphrase_nonce BLOB,
    passphrase_ciphertext BLOB,
    CHECK(
        (passphrase_nonce IS NULL AND passphrase_ciphertext IS NULL)
        OR
        (passphrase_nonce IS NOT NULL AND passphrase_ciphertext IS NOT NULL)
    ),
    CHECK(
        kind = 'private_key'
        OR
        (passphrase_nonce IS NULL AND passphrase_ciphertext IS NULL)
    )
) WITHOUT ROWID;
```

Credential ID 由 Rust CSPRNG 生成 128-bit 随机值，不由 WebView 指定。
Schema v3 保留该表和 Record AAD 不变；Host 改为只引用 Credential ID。
Schema v4 继续保留该表，并允许 Group/Host 的 `Set` Override 引用 Credential。
Schema v5 在保持列和 Record AAD 结构的前提下，把 Kind CHECK 扩展为：

```text
password
private_key
system_agent
```

Schema v7 重建 Credential 及引用它的 Group/Host/Route Step 表，把 Kind CHECK
扩展为：

```text
password
private_key
system_agent
keyboard_interactive
```

`keyboard_interactive` 的 `secret_nonce`、`secret_ciphertext` 和 Passphrase
列必须全部为 `NULL`。其他 Kind 继续要求 Secret Nonce/Ciphertext；只有
`private_key` 可以有 Passphrase：

```sql
CHECK(
    (
        kind = 'keyboard_interactive'
        AND secret_nonce IS NULL
        AND secret_ciphertext IS NULL
    )
    OR
    (
        kind != 'keyboard_interactive'
        AND secret_nonce IS NOT NULL
        AND secret_ciphertext IS NOT NULL
    )
)
```

## Record AEAD

所有字段使用 XChaCha20-Poly1305 和独立随机 Nonce：

```text
anyssh/record/v2|<vault_id>|credential|<credential_id>|password|secret
anyssh/record/v2|<vault_id>|credential|<credential_id>|private_key|secret
anyssh/record/v2|<vault_id>|credential|<credential_id>|private_key|passphrase
anyssh/record/v2|<vault_id>|credential|<credential_id>|system_agent|secret
```

Label、Username 和 Kind 由 SQLCipher 整库保护；Password、Private Key、
Passphrase 和 System Agent Fingerprint Selector 额外使用上述 Record AEAD。
Keyboard-interactive 没有 Record AEAD Payload，因为没有可持久化 Response。

## Repository Commands

DB Actor 顺序处理：

- `CreateCredential`
- `UpdateCredential`
- `ListCredentials`
- `DeleteCredential`
- `ResolveCredential`

`ListCredentials` 永不解密 Secret。`ResolveCredential` 返回的 Rust-only 类型不
实现 Serialize，Debug 始终脱敏。当前 Schema v7 中，Credential 被 Group 或
Host 的 `Set` Override 引用时，`DeleteCredential` 返回占用错误，不自动清空
引用。

## Schema v1 到 v2

- 解锁 Schema v1 Vault 时，在 `IMMEDIATE` Transaction 中创建
  `credentials` 表并把 `user_version` 更新为 `2`。
- Migration 中断必须回滚到完整 Schema v1；下次解锁可安全重试。
- Schema `0` 只允许用于新 Vault 初始化，不在已有 Vault 解锁时自动初始化。
- 迁移不修改现有 Bootstrap、VMK、Key Slot 或 `hosts` 记录。
- 当前解锁流程会在完成 v1 -> v2 后继续执行 v2 -> v3 Host Migration 和
  v3 -> v4 Group/Override、v4 -> v5 Agent Kind、v5 -> v6 Known Host 和
  v6 -> v7 Interactive Kind Migration。

## 验证

- Password、Private Key、Passphrase 和 System Agent Selector 重启后可恢复。
- Interactive Credential 重启后只恢复 Label/Username/Kind，四个
  Secret/Passphrase 列保持 `NULL`。
- 数据库、WAL、Bootstrap 和 Sidecar 不包含测试 Secret 明文。
- Credential List/Debug/IPC JSON 不包含 Secret。
- Locked Vault 拒绝 Repository Command。
- Schema v1 到 v2 的成功迁移和中断回滚。
- Docker OpenSSH 使用 Credential ID 完成加密 Private Key 认证。
- Docker OpenSSH 使用 Fingerprint-selected System Agent Credential 完成 Direct
  和混合 Jump/Target 认证。
- Docker OpenSSH PAM 使用 Interactive Credential 完成 Direct、Saved Host 和
  Interactive Jump Hop；测试 Response 不出现在 Vault 文件。
