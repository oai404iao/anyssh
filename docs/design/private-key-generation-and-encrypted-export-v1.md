# Private Key Generation and Encrypted Export v1

> 状态：设计中
> 日期：2026-07-29

本文定义 Desktop MVP 的 Rust-owned Private Key 生成、Public Key Reveal 和
Encrypted Private Key Export。长期决策见 Proposed ADR-0019。

## 目标

- 在 AnySSH 内生成 Ed25519 或 RSA 4096 Private Key Credential。
- 对 Imported/Generated Private Key 显示 Algorithm、SHA-256 Fingerprint 和
  OpenSSH Public Key。
- 经 Native PIN Step-up 和新 Passphrase 把 Private Key 导出为加密 OpenSSH
  文件。
- 保持 Private Key、Stored Passphrase、PIN、Export Passphrase 和 Path 不进入
  WebView、普通 IPC、日志、遥测或 Evidence。

## 非目标

- WebView 内 Private Key 明文 Reveal 或 Clipboard Copy。
- 未加密 Private Key Export。
- 覆盖已有文件、批量 Export、PKCS#8/PEM/PPK 转换。
- SSH Certificate、FIDO2、PKCS#11、应用内 Agent 或 Key Rotation。
- Password Credential Reveal。
- Android Document Provider、iOS Share Sheet 或移动端生物识别 Step-up。

## 类型与边界

```text
PrivateKeyGenerationAlgorithm
  = Ed25519
  | Rsa4096

PrivateKeyPublicSummary
  credential_id
  algorithm
  fingerprint_sha256
  openssh_public_key

PrivateKeyExportSummary
  file_name
  algorithm
  fingerprint_sha256
  encrypted = true
```

`PrivateKeyPublicSummary` 是 Public Metadata，可以序列化给 WebView。Export
Summary 只返回文件名，不返回完整 Path。

禁止出现在 Tauri Request/Response：

- Private Key。
- Stored/Export Passphrase。
- PIN。
- File Path、File URI 或 File Handle。
- RNG Seed。

## Key Generation

```text
React { label, username, algorithm }
  -> Tauri typed metadata request
    -> ApplicationCore
      -> spawn_blocking + CSPRNG
        -> ssh-key PrivateKey
          -> OpenSSH text in Zeroizing<String>
            -> DatabaseActorHandle::create_credential
              -> SQLCipher + Record AEAD
```

- 默认 Algorithm 为 Ed25519。
- RSA 使用 4096 bit，作为兼容选项。
- Label/Username 使用现有 Credential 上限；Comment 使用经过清理的 Label，最多
  128 个字符。
- 生成任务不得占用 DB Actor Thread。只有完成后的 `Zeroizing<String>` 进入
  `CreateCredential`。
- 生成失败不得创建部分 Credential。
- Schema 保持 v7；Generated Key 与 Imported Key 都是 `private_key` Kind。

## Public Key Reveal

```text
React { credentialId }
  -> Tauri
    -> ApplicationCore
      -> DatabaseActor resolve_credential
        -> parse/decrypt in Rust
          -> Public Key + SHA-256 Fingerprint
            -> metadata-only response
```

- 只接受 `private_key` Credential。
- Imported Encrypted Key 使用 Vault 中已有 Passphrase 在 Rust 内解密。
- OpenSSH Public Key 为单行、无控制字符、最大 16 KiB。
- UI 提供 Select/Copy Public Key；不得把 Private Key 或 Stored Passphrase
  混入错误、Debug 或 React State。
- Vault Lock、Credential 删除或 Kind 不匹配立即 Fail Closed。

## Native Step-up

Export 前执行原生 PIN Step-up：

1. Tauri Command 只收到 Credential ID。
2. Linux GTK Secure Entry 或 Windows Credential UI 获取 PIN。
3. PIN 立即进入 `Zeroizing<String>`。
4. DB Actor 使用当前 Bootstrap 验证 PIN 对应当前已解锁 Vault。
5. 最多三次；取消或失败终止 Export。

PIN 不使用 React Input，不写环境变量，不进入命令行、日志或 Crash Context。
Step-up 成功只对当前 Export Request 有效，不产生长期 Token。

## Export Passphrase

- Linux 使用进程内 GTK Dialog 的两个 Password Entry。
- Windows 使用两次不持久化 Credential UI Prompt。
- Passphrase 长度为 1 到 1024 Byte；两个值必须一致。
- 最多三轮确认；取消或不匹配不创建文件。
- Passphrase 只存在于 `Zeroizing<String>`。
- Stored Key Passphrase 只用于解密原始 Key，不显示、不复用。

## Encrypted Export

```text
Resolved Private Key
  -> decrypt if needed
    -> Native Export Passphrase
      -> ssh-key AES-256-CTR + bcrypt-pbkdf
        -> OpenSSH Zeroizing<String>
          -> Rust Native Save Picker
            -> create_new + bounded write + fsync
```

- v1 输出固定为加密 OpenSSH Private Key。
- Destination 由 Native Save Picker 选择；WebView 不提供 Path。
- 文件必须不存在。拒绝 Symlink/Reparse Point、目录、Device、FIFO 和 Socket。
- Unix 使用 `0600`；Windows 创建当前用户受限文件。
- 写入失败删除部分文件；成功后清空序列化 Buffer。
- Result 只返回 sanitized File Name、Algorithm、Fingerprint 和
  `encrypted: true`。

## UI

Credentials Workspace 增加：

- `Generate key`。
- Private Key Card 的 `Public key`。
- Private Key Card 的 `Export encrypted…`。

Generation Dialog：

- Label。
- Username。
- Algorithm：Ed25519（Recommended）/RSA 4096。

Public Key Dialog：

- Algorithm。
- SHA-256 Fingerprint。
- 单行 OpenSSH Public Key。
- Copy Public Key。

Export：

- UI 只显示“等待原生确认”和最终 Metadata。
- PIN、Passphrase、Path 不显示在 React 表单。
- Browser QA 明确标记为 Metadata Preview，不生成或写出 Key。

## Error Model

固定分类：

- `InvalidRequest`
- `UnsupportedPlatform`
- `VaultLocked`
- `CredentialNotFound`
- `CredentialKindMismatch`
- `GenerationFailed`
- `StepUpCancelled`
- `StepUpRejected`
- `PassphraseCancelled`
- `PassphraseMismatch`
- `DestinationCancelled`
- `DestinationExists`
- `ExportFailed`

错误不得包含 PIN、Passphrase、Private Key、完整 Path 或底层解析内容。

## Limits

| 项目 | 上限 |
| --- | --- |
| Generation Algorithm | 2 |
| RSA Size | 固定 4096 |
| Comment | 128 Character |
| Public Key Projection | 16 KiB |
| Export Passphrase | 1024 Byte |
| PIN/Passphrase Attempts | 3 |
| Exported OpenSSH File | 1 MiB |

## Validation

- Unit：
  - Ed25519/RSA 4096 生成、序列化、解析和 Public Projection。
  - Imported Encrypted Key Public Projection。
  - PIN Step-up Success/Failure/Cancel。
  - Export Re-encryption、Passphrase Mismatch、Existing File 和 Partial Cleanup。
  - Debug/IPC Redaction。
- OpenSSH：
  - Generated Ed25519 与 RSA Credential Direct/Saved/Jump Authentication。
  - Exported Key 由 OpenSSH/ssh-key 使用新 Passphrase 解密。
- Browser：
  - Metadata-only Generation、Public Key Dialog 和 Native-only Export Notice。
  - Desktop/Mobile/Compact 与 Browser Error Log。
- Native：
  - X11 Native Generate、Public Key、PIN Step-up、Passphrase、Save Picker、
    Source Delete 和真实 SSH Marker。
  - Windows 真实 EXE/WebView2 + Credential UI + Save Dialog + OpenSSH Marker。
  - Wayland/IBus 回归，且不出现 Private Key/Passphrase/PIN。
- Build：
  - Linux Container、Android ARM64 和 Windows。
  - 同 Commit CI 与 Artifact Secret Scan。

## 相关文档

- [Credential Repository v1](credential-repository-v1.md)
- [Native Private Key Import v1](native-private-key-import-v1.md)
- [Native Encrypted Private Key Passphrase v1](native-encrypted-private-key-passphrase-v1.md)
- [Threat Model v1](threat-model-v1.md)
- [ADR-0019](../adr/0019-private-key-generation-and-export-stay-in-rust.md)
- [ExecPlan 0009](../execplans/active/0009-private-key-generation-and-encrypted-export.md)
