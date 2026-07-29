# Native Encrypted Private Key Passphrase v1

> 状态：已实现
> 日期：2026-07-28

本文扩展 [Native Private Key Import v1](native-private-key-import-v1.md)，让
Linux/Windows Desktop 能导入加密 OpenSSH Private Key，同时保持 Passphrase
不进入 WebView。

## 范围

- 已有 Rust-owned Native File Picker。
- OpenSSH Private Key 加密状态检测。
- Linux GTK 和 Windows 原生 Secure Prompt。
- 最多三次 Passphrase 尝试、取消和稳定错误。
- 复用 Schema v5 引入并由当前 Schema v8 保留的 Private Key/Passphrase
  Record AEAD。
- X11 与 Windows 真实交互证据。

不包含：

- Secret Reveal/Export。
- 每次连接重新提示或“不记住 Passphrase”模式。
- PEM/PKCS#8 专用导入 UI。
- Android/iOS Content URI。

## 数据流

```text
React Import Request
  -> { label, username }
    -> Tauri Command
      -> Native File Picker
        -> ApplicationCore bounded read
          -> parse OpenSSH Private Key
            -> unencrypted: validate and store
            -> encrypted:
                 NativeSecretPrompt(sanitized context)
                   -> Zeroizing<String>
                     -> Rust decrypt/validate
                       -> store encrypted Key + encrypted Passphrase
                         -> CredentialSummary
```

WebView 不接收 Path、Key、Passphrase、解密结果或 Prompt Handle。

## Application Boundary

`anyssh-app` 定义平台无关的 Prompt Provider：

```rust
trait PrivateKeyPassphrasePrompt {
    fn request(
        &self,
        context: PrivateKeyPromptContext,
    ) -> impl Future<
        Output = Result<Option<Zeroizing<String>>, PrivateKeyPromptError>,
    > + Send;
}
```

`PrivateKeyPromptContext` 只允许：

- 经过 Credential Label 长度/控制字符校验的展示文本。
- `attempt` / `max_attempts`。
- 是否为上一次 Passphrase 错误。

它不得包含 Path、Key Blob、Username、Host 或 Fingerprint。

ApplicationCore 负责文件读取、加密状态识别、最多三次循环、Key 验证和
Repository 写入。Tauri 只实现 Prompt Provider 并把 Platform Result 返回 Core；
业务逻辑不进入 Command。

## Platform Prompt

### Linux

- 在 Tauri/GTK Main Thread 创建 Modal Dialog。
- Entry 使用不可见字符显示且禁止日志/自动填充。
- 完成后立即清空 Entry 的可控文本。
- 不调用 `zenity`、Shell 或任意外部进程。

### Windows

- 使用系统原生 Secure Password/Credential Prompt。
- Prompt 只显示 AnySSH 和受限 Credential Label。
- 返回 Buffer 转入 `Zeroizing<String>` 后立即清零并释放平台 Buffer。
- QA-only CDP 不能读取 Native Prompt 内容。

### Android/iOS

v1 返回稳定 Unsupported。后续需要 Activity/ViewController 与 Content URI
生命周期设计，不复用 Desktop Prompt。

## 错误与取消

- Picker 取消或 Prompt 取消：返回 `null` Summary，不创建 Credential。
- 错误 Passphrase：通用提示，不包含 Path、Algorithm 或解析内部错误。
- 三次失败：结束本次 Import，必须重新选择文件。
- Prompt 初始化失败：稳定 Platform Prompt Unavailable。
- 任意失败都必须清零当前 Passphrase，并且不得持久化半成品记录。

## 存储

不新增 Schema：

- `secret_ciphertext` 保存原始加密 OpenSSH Private Key。
- `passphrase_ciphertext` 保存 Passphrase。
- 两者使用不同 Nonce 和 AAD。

SSH 连接时仍由 Rust-only Credential Resolution 把两者移动到
`SessionAuthentication::PrivateKey`。

## 验证

- Unit/Integration：加密检测、正确/错误/空 Passphrase、取消、三次上限、
  Record AEAD 和 Debug/Error 脱敏。
- Browser QA：只验证 metadata-only UI，不能模拟文件或 Secret 输入。
- X11：真实 GTK Picker、Secure Prompt、导入、源文件删除、SSH 和 Vault 扫描。
- Windows：真实 Native Picker、Secure Prompt、EXE/WebView2、OpenSSH 和重启。
- Android/Linux Container 与 Windows Build 回归。

Head `dac51ffd079d56ab1d7f7a5837d6bf6b89b1c333` 的 CI Run
`30325359607` 已完成上述 Linux/Windows/Browser/OpenSSH/Container 验证；
ADR-0014 已接受。

## 相关文档

- [Native Private Key Import v1](native-private-key-import-v1.md)
- [Credential Repository v1](credential-repository-v1.md)
- [Threat Model v1](threat-model-v1.md)
- [ADR-0014](../adr/0014-encrypted-private-key-passphrase-stays-out-of-webview.md)
