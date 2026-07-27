# AnySSH Credential Repository v1

> 状态：已实现
> 日期：2026-07-27

本文定义 Vault-backed Credential Repository、SQLCipher Schema v2 和 SSH
Credential ID 解析边界。它不定义 Credential 管理 UI、Secret Reveal 或平台文件
选择器。

## 安全目标

- Credential 摘要只包含 ID、Label、Username 和 Kind。
- Password、Private Key 和 Key Passphrase 使用 Record AEAD 二次加密。
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

Private Key 的创建与更新只提供给 Rust Trusted Service。当前不提供
`credential_import_private_key` Tauri Command，也不接受 WebView 指定的任意文件
路径。后续原生文件选择器必须返回受限 Token，由 Rust 读取并验证文件。

Password Credential 可以通过 Typed IPC 创建或更新，因为用户输入密码本来就会
短暂经过 WebView；IPC Adapter 必须立即把它移动到 `Zeroizing<String>`，不得保存
到前端全局状态或日志。

## SQLCipher Schema v2

Schema v2 保留 Phase 0 的 `vault_meta` 和 `hosts` 表，并新增：

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

## Record AEAD

所有字段使用 XChaCha20-Poly1305 和独立随机 Nonce：

```text
anyssh/record/v2|<vault_id>|credential|<credential_id>|password|secret
anyssh/record/v2|<vault_id>|credential|<credential_id>|private_key|secret
anyssh/record/v2|<vault_id>|credential|<credential_id>|private_key|passphrase
```

Label、Username 和 Kind 由 SQLCipher 整库保护；Password、Private Key 和
Passphrase 额外使用上述 Record AEAD。

## Repository Commands

DB Actor 顺序处理：

- `CreateCredential`
- `UpdateCredential`
- `ListCredentials`
- `DeleteCredential`
- `ResolveCredential`

`ListCredentials` 永不解密 Secret。`ResolveCredential` 返回的 Rust-only 类型不
实现 Serialize，Debug 始终脱敏。

## Schema v1 到 v2

- 解锁 Schema v1 Vault 时，在 `IMMEDIATE` Transaction 中创建
  `credentials` 表并把 `user_version` 更新为 `2`。
- Migration 中断必须回滚到完整 Schema v1；下次解锁可安全重试。
- Schema `0` 只允许用于新 Vault 初始化，不在已有 Vault 解锁时自动初始化。
- 迁移不修改现有 Bootstrap、VMK、Key Slot 或 `hosts` 记录。

## 验证

- Password、Private Key 和 Passphrase 重启后可恢复。
- 数据库、WAL、Bootstrap 和 Sidecar 不包含测试 Secret 明文。
- Credential List/Debug/IPC JSON 不包含 Secret。
- Locked Vault 拒绝 Repository Command。
- Schema v1 到 v2 的成功迁移和中断回滚。
- Docker OpenSSH 使用 Credential ID 完成加密 Private Key 认证。
