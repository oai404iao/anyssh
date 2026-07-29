# AnySSH Vault Bootstrap v1

> 状态：Phase 1 当前已实现格式
> 日期：2026-07-27

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

## SQLCipher Schema v8

数据库当前 `user_version` 为 `8`。Schema v1 创建：

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

Schema v2 新增独立 `credentials` 表。Schema v3 把旧 `hosts` Password 迁移为
Credential，并新增规范化 Host、Jump Route 和有序 Route Step。完整定义见：

- [Credential Repository v1](credential-repository-v1.md)
- [Host 与 Jump Route Repository v1](host-jump-route-repository-v1.md)

Schema v4 新增 `host_groups`、`group_overrides`，并把 Host Credential/Route
引用改为带 CHECK Constraint 的 `Inherit / Set / Clear` 状态。完整定义见：

- [Group Persistence and Three-State Inheritance v1](group-inheritance-v1.md)

Schema v5 扩展 Credential Kind CHECK，增加 `system_agent`。System Agent 只保存
所选 Public Key 的 SHA-256 Fingerprint 作为认证选择器，并继续使用 Credential
Record AEAD；Private Key 和签名仍留在外部 Agent。完整定义见：

- [System SSH Agent Authentication v1](system-ssh-agent-authentication-v1.md)

Schema v6 新增 Endpoint-scoped `known_hosts` 与 `known_host_keys`。完整 Public
Key、Algorithm 和 SHA-256 Fingerprint 由 SQLCipher 保护，并在读写时重算校验；
Public Host Key 不增加 Credential Record AEAD。完整定义见：

- [Known Host Repository v1](known-host-repository-v1.md)

Schema v7 重建 `credentials` 及引用它的 Group/Host/Route Step 表，新增
`keyboard_interactive` Kind。该 Kind 只保存 Label/Username，Secret 和
Passphrase 四列必须全部为 `NULL`；其他 Kind 继续要求现有 Record AEAD
Ciphertext。完整定义见：

- [Keyboard-interactive Authentication v1](keyboard-interactive-authentication-v1.md)

Schema v8 新增 Vault-wide Appearance Settings、Custom Terminal Theme、
Imported Font Metadata 和 Snippet Repository。Snippet Body 使用 Record AEAD；
Font Binary 位于完整性校验的应用管理 Asset Store，不进入 SQLCipher BLOB。完整
定义见：

- [Terminal Appearance, Font, and Snippet v1](terminal-appearance-font-and-snippet-v1.md)

## 创建和迁移

- 新 Vault 先在同文件系统的私有临时目录中完整创建 Bootstrap 和数据库。
- 文件同步后，通过目录 Rename 原子发布为正式 Vault。
- 已存在或不完整的 Vault 不得被自动覆盖。
- Schema migration 在 `IMMEDIATE` Transaction 中执行；失败或中断必须回滚。
- 已有 Schema v1 Vault 在解锁时依次迁移到 v2、v3、v4、v5、v6、v7、v8。
- Schema v2 -> v3 在同一 Transaction 中把旧 Host Password 重新加密为
  Credential；中断后保持完整 v2 并可重试。
- Schema v3 -> v4 重建 Host/Route Step 表，把非空引用迁移为 `Set`、空引用
  迁移为 `Inherit`，并保持现有 Effective Value；中断后保持完整 v3。
- Schema v4 -> v5 重建 Credential 及引用它的 Group/Host/Route Step 表，扩展
  Kind CHECK 且保持 ID、Ciphertext、Override 和 Foreign Key；中断后保持完整
  v4。
- Schema v5 -> v6 只新增 Known Host/Key 表；中断后保持完整 v5 并可重试。
- Schema v6 -> v7 重建 Credential 及其引用表，保持 ID、Ciphertext、
  Override、Known Host 和 Foreign Key；中断后保持完整 v6 并可重试。
- Schema v7 -> v8 新增 Appearance/Theme/Font/Snippet 表和默认 Appearance；
  保持全部旧 Repository 与引用，中断后保持完整 v7 并可重试。
- 每个 Migration 显式写入自己的版本号，不能用最新 Schema 常量代替中间版本。
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
- Schema v3 Host/Jump Route 重启恢复、旧 Host Password 无损迁移、引用占用、
  顺序和循环检测通过。
- Schema v4 Group/三态 Override 重启恢复、v3 语义保持、State/Value CHECK、
  Parent/Route 循环、32 层限制、引用占用和中断回滚通过。
- Schema v5 System Agent Credential 重启恢复、Record AEAD、v4 引用保持和中断
  回滚通过。
- Schema v6 Known Host Repository、v5 数据恢复、Endpoint 规范化、Key 字段
  一致性、并发 TOFU 和中断回滚通过。
- Schema v7 Interactive Credential、v6 引用/Known Host 保持、重启、
  Secret 列约束、明文扫描和中断回滚通过。
- Schema v8 Appearance/Theme/Font/Snippet、v7 Repository 保持、Snippet
  Record AEAD、Managed Font Integrity、重启和中断回滚通过。
- Windows 与 Android 构建已经验证；iOS 仍等待 macOS/Xcode 环境。
