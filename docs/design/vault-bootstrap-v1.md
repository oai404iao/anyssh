# AnySSH Vault Bootstrap v1

> 状态：Phase 0 已实现格式
> 日期：2026-07-26

本文记录当前 Vault Bootstrap、PIN Slot、SQLCipher Key 派生和最小数据库
Schema。任何不向后兼容的修改都必须增加格式版本并更新 ADR/ExecPlan。

## 文件布局

应用数据目录中的 Vault 子目录只包含：

```text
vault/
├── vault.bootstrap.json
├── vault.db
├── vault.db-wal       # SQLite 按需创建
└── vault.db-shm       # SQLite 按需创建
```

Bootstrap 可以明文保存格式与 KDF 元数据，但不得包含 Host、用户名、密码、
WebDAV URL 或脚本名称。数据库和 WAL 由 SQLCipher 整体加密。

## Bootstrap JSON

```json
{
  "format_version": 1,
  "vault_id": "<16 random bytes, base64url without padding>",
  "key_slots": [
    {
      "id": "<16 random bytes, base64url without padding>",
      "kind": "pin",
      "kdf": {
        "algorithm": "argon2id",
        "version": 19,
        "memory_kib": 65536,
        "iterations": 3,
        "parallelism": 1,
        "salt": "<16 random bytes, base64url without padding>"
      },
      "wrapping": {
        "algorithm": "xchacha20poly1305",
        "nonce": "<24 random bytes, base64url without padding>",
        "ciphertext": "<48 bytes, base64url without padding>"
      }
    }
  ]
}
```

解析使用 `deny_unknown_fields`。当前最多接受 16 个 Slot，Bootstrap 最大
64 KiB。Argon2id 参数在执行前必须通过上下限检查，防止被篡改的文件造成
不受控资源消耗。

## PIN Slot

1. 操作系统 CSPRNG 生成 32-byte VMK。
2. Argon2id 从 PIN 和 16-byte Salt 派生 32-byte KEK。
3. XChaCha20-Poly1305 使用 KEK 包装 VMK。
4. AAD 为：

```text
anyssh/pin-slot/v1|<vault_id>|<slot_id>|argon2id|19|<memory>|<iterations>|<parallelism>
```

错误 PIN、损坏 Ciphertext 和认证 Tag 错误统一返回 `vault unlock failed`，
不得包含 PIN 或底层加密数据。

Phase 0 默认 Argon2id 参数：

| 参数 | 值 |
| --- | ---: |
| Memory | 65,536 KiB |
| Iterations | 3 |
| Parallelism | 1 |
| Output | 32 bytes |

未来可以按设备校准，但已保存 Slot 的参数不可被隐式替换。

## HKDF 子密钥

HKDF-SHA-256 使用 Vault ID 的 UTF-8 表示作为 Salt，VMK 作为输入密钥材料。

| 用途 | HKDF Info |
| --- | --- |
| SQLCipher DB Key | `anyssh/v1/sqlcipher-database` |
| Record AEAD Root | `anyssh/v1/record-encryption` |

两个输出均为 32 bytes，必须分别持有和清零。

## SQLCipher Schema v2

数据库当前 `user_version` 为 `2`。Schema v1 创建：

```sql
CREATE TABLE vault_meta(
    key TEXT PRIMARY KEY NOT NULL,
    value BLOB NOT NULL
) WITHOUT ROWID;

CREATE TABLE hosts(
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
    username TEXT NOT NULL,
    password_nonce BLOB NOT NULL,
    password_ciphertext BLOB NOT NULL
) WITHOUT ROWID;
```

`vault_meta.vault_id` 必须与 Bootstrap 一致。该 `hosts` 表是 Phase 0
兼容记录；Host 元数据依赖 SQLCipher，Password 另外使用
XChaCha20-Poly1305 加密；Password AAD 为：

```text
anyssh/record/v1|<vault_id>|host|<host_id>|password
```

Schema v2 新增独立 `credentials` 表。完整字段、Record AAD 和 Rust-only Private
Key 数据流见 [Credential Repository v1](credential-repository-v1.md)。

## 创建和迁移

- 新 Vault 先在同文件系统的私有临时目录中完整创建 Bootstrap 和数据库。
- 文件同步后，通过目录 Rename 原子发布为正式 Vault。
- 已存在或不完整的 Vault 不得被自动覆盖。
- Schema migration 在 `IMMEDIATE` Transaction 中执行；失败或中断必须回滚。
- 已有 Schema v1 Vault 在解锁时迁移到 v2；中断后保持完整 v1 并可重试。
- Schema `0` 只用于新 Vault 初始化，不在已有文件上自动创建 Schema。
- Linux 目录权限为 `0700`，Bootstrap 和数据库权限为 `0600`。

## 当前验证

- bundled SQLCipher 报告 `4.10.0 community`。
- 正确 PIN、错误 PIN、损坏 Slot、重启解锁和迁移回滚测试通过。
- 数据库、WAL、Sidecar 和 Bootstrap 的业务明文扫描通过。
- 原生 Xvfb 流程验证了创建、锁定、错误 PIN 和重新解锁。
- Vault 生命周期现由专用 DB Actor Thread 串行管理；有界 Command Queue、
  oneshot Response、Shutdown 和 Thread Join 测试通过。
- Schema v2 Credential Password、Private Key 与 Passphrase 重启恢复、明文扫描、
  v1 -> v2 迁移和中断回滚通过。
- Windows 与 Android 构建已经验证；iOS 仍等待 macOS/Xcode 环境。
