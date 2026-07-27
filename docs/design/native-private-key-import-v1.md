# AnySSH 原生私钥导入 v1

> 状态：已实现
> 日期：2026-07-27

本文定义 Desktop MVP 的受约束 SSH Private Key 文件导入。它不定义 Secret
Reveal/Export、Key 生成、加密私钥 Passphrase Prompt 或移动端 Content URI。

## 数据流

```text
React
  -> { label, username }
    -> Tauri credential_import_private_key
      -> Rust Native File Picker
        -> selected FilePath remains in Rust
          -> ApplicationCore::import_private_key_credential_from_path
            -> bounded file read + OpenSSH decode validation
              -> DatabaseActorHandle::create_credential
```

Import Request 使用 `deny_unknown_fields`。它不得出现：

- `path`
- `privateKey`
- `passphrase`

取消文件选择返回 `null`，不创建 Credential。

## 文件约束

Rust 在 Actor 写入前执行：

1. `symlink_metadata` 在打开前拒绝 Symlink 和非普通文件，避免 FIFO/Socket
   等特殊文件阻塞读取。
2. Unix/Android 使用 `O_NOFOLLOW | O_CLOEXEC` 打开最终路径组件，打开后再次
   检查必须为普通文件。
3. 文件必须为 1 Byte 到 1 MiB。
4. 使用有界 Reader 读取 UTF-8 文本。
5. 使用与 SSH Core 相同的 russh/OpenSSH Decoder 在无 Passphrase 条件下解析。
6. 解析成功后将 `Zeroizing<String>` 直接交给 ApplicationCore/DB Actor。

错误使用固定分类消息，不包含文件 Path、文件名、Key 文本或底层解析内容。

## UI

Credential 页面提供：

- Password Credential：Label、Username、Password。
- Private Key Import：Label、Username、打开原生文件选择器。

Private Key Summary 只显示 Label、Username 和 Kind。首版不提供编辑 Key 内容；
用户可以删除未被 Host 引用的 Credential 后重新导入。

## 平台边界

- Linux/Windows Desktop：Native Picker 返回 Path，由 Rust 有界读取。
- Android/iOS：构建可以包含该 Command，但 Content URI 在有专用读取适配前返回
  不支持错误。
- Browser QA：只创建 metadata-only Preview Summary，不打开文件或网络；它只
  验证 UI 行为，不能替代 Rust Import Test。

## 验证

- ApplicationCore 文件导入成功后，Credential List 只返回 metadata。
- Key 明文不出现在 Debug、JSON、错误或浏览器状态。
- Invalid/Encrypted Key 不写入 Vault。
- Host 可以引用导入后的 Credential，并通过 Saved Host ID 连接。
- Playwright/agent-browser 覆盖 Credential、Host、Route 配置 UI。
