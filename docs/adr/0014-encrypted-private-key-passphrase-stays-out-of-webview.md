# ADR-0014：加密私钥 Passphrase 使用原生安全提示且不进入 WebView

- 状态：Accepted
- 日期：2026-07-27
- 接受日期：2026-07-28
- 决策人：项目维护者

## 背景

ADR-0011 已规定 Native Private Key Import 的 Path、文件内容和验证留在 Rust。
当前实现只接受未加密 OpenSSH Private Key。要导入加密 Key，AnySSH 必须取得
Passphrase，但把 Passphrase 放进 React 表单或普通 Tauri IPC 会破坏 ADR-0006
的 Secret Boundary。

## 决策

- Rust 在 Native Picker 选中文件后读取并识别 OpenSSH Private Key 是否加密。
- 加密 Key 的 Passphrase 只通过 Desktop 平台原生安全输入提示获取，不使用
  React/WebView 输入框。
- Prompt Context 只包含受限 Label 和当前尝试次数，不包含 Path、Key 内容、
  Fingerprint 或 Endpoint。
- Linux v1 使用进程内 GTK Secure Entry；Windows v1 使用系统原生 Secure
  Password/Credential Prompt。不得通过 Shell、`zenity`、PowerShell 子进程或
  其他外部程序收集 Secret。
- Native Prompt 返回值立即进入 `Zeroizing<String>`；取消返回无 Credential，
  错误 Passphrase 返回通用错误并在一次 Import 中最多重试三次。
- Rust 验证成功后保存原始加密 OpenSSH Key 和 Passphrase；二者分别使用现有
  Credential Record AEAD，WebView 只获得 `CredentialSummary`。
- Prompt、错误、Debug、日志、截图和 QA 报告不得包含 Passphrase 或 Key 内容。
- Android/iOS v1 明确 Unsupported，等待 Content URI 与平台安全输入适配器。

## 备选方案

- WebView Password Input：Secret 会进入 DOM/Renderer/IPC，拒绝。
- 导入时解密并只保存无 Passphrase Key：降低源 Key 的独立保护并改变现有
  Credential 语义，拒绝。
- 调用 `zenity`、PowerShell 或其他外部程序：扩大 Secret 进程边界，拒绝。
- 每次 SSH 连接都重新提示：可作为未来“不记住 Passphrase”模式，不属于 v1。

## 后果

### 正面

- Encrypted Key Import 不放宽现有 WebView Secret Boundary。
- 可以复用现有 Private Key/Passphrase Record AEAD 和 SSH Core 解码路径。
- 用户取消或输错 Passphrase 不会留下半成品 Credential。

### 代价与风险

- Linux GTK 和 Windows Native Prompt 需要独立平台适配与真实交互 QA。
- Toolkit/OS Prompt 自身会短暂持有 Secret；AnySSH 必须尽快清空可控 Buffer。
- Android/iOS 需要不同的 Activity/Controller 生命周期设计。

## 验证

- Rust 覆盖加密 Key 检测、正确/错误 Passphrase、取消、三次上限和错误脱敏。
- Tauri IPC Schema 拒绝 `path`、`privateKey` 和 `passphrase`。
- Linux X11 真实 Picker + Secure Prompt 导入并完成 SSH。
- Windows 真实 EXE/WebView2 + Native Picker + Secure Prompt 导入并完成 SSH。
- Vault、日志、截图和 QA Evidence 不含 Key Header 或测试 Passphrase。
- Android ARM64 和 Linux/Windows Build 回归；iOS 继续等待 macOS/Xcode。

Head `dac51ffd079d56ab1d7f7a5837d6bf6b89b1c333` 的 GitHub Actions Run
`30325359607` 九个 Job 全部通过。Linux X11 验证了进程内 GTK Prompt、错误
重试和导入；Windows 验证了真实 Native Picker、两次 Credential UI、源文件删除、
加密 Key SSH、System Agent SSH 和进程重启。Artifact 二次扫描未发现测试
Passphrase 或 OpenSSH Key Header，因此本 ADR 于 2026-07-28 接受。

## 相关文档

- [Native Encrypted Private Key Passphrase v1](../design/native-encrypted-private-key-passphrase-v1.md)
- [ADR-0003](0003-double-layer-local-encryption.md)
- [ADR-0006](0006-secrets-stay-out-of-webview.md)
- [ADR-0011](0011-native-private-key-import-stays-in-rust.md)
- [Phase 1 Encrypted Key ExecPlan](../execplans/completed/0004-native-encrypted-private-key-passphrase.md)
