# ADR-0011：原生私钥导入完全留在 Rust 边界

- 状态：Accepted
- 日期：2026-07-27
- 决策人：项目维护者

## 背景

AnySSH 需要从本地文件导入 SSH Private Key。若使用 HTML File Input、由
JavaScript 调用文件选择器后读取内容，或让 WebView 把任意 Path 传给 Tauri
Command，Private Key 与文件系统能力都会越过既定安全边界。Key Passphrase 同样
不得为了方便而加入 Tauri IPC。

## 决策

- WebView 只提交 Credential Label 和 Username。
- Tauri Command 在 Rust 内发起 Native File Picker；选中的 Path/URI 不返回
  WebView。
- Rust 在保存前检查文件类型、大小、UTF-8 和 OpenSSH Private Key 可解析性。
- Import Error 不包含 Path、Private Key 内容或解析器底层 Secret。
- 用户取消选择返回无 Credential 的正常结果，不作为错误。
- 首版仅导入无需 Passphrase 即可解析的 Private Key。
- 加密 Private Key 必须等待原生安全 Passphrase Prompt；`passphrase` 字段不得
  加入当前或后续普通 WebView Import Request。
- 移动端 Content URI 读取在有独立平台适配和测试前保持不支持。

## 备选方案

- HTML File Input 读取 Key：Private Key 会进入 React/WebView 内存，拒绝。
- WebView 传任意 Path 给 Tauri：扩大文件系统能力并违反受限选择边界，拒绝。
- WebView 输入 Passphrase：违反现有 Secret IPC 约束，拒绝。
- 不校验就保存任意文本：把错误延迟到连接阶段并允许无效 Secret 污染 Vault，
  拒绝。

## 后果

### 正面

- Private Key 内容、Path 和未来 Passphrase 保持在 Rust/原生边界内。
- 文件能力只来自一次 Native Picker 结果。
- 无效或过大的文件在写入 Vault 前被拒绝。

### 代价与风险

- 首版不能导入加密 Private Key。
- Desktop Path 与移动端 Content URI 需要不同平台适配。
- Native Picker 的真实运行证据需要与纯浏览器 UI QA 分开。

## 验证

- Tauri Import Request 只反序列化 Label/Username，并拒绝 `path`、
  `privateKey` 和 `passphrase`。
- Rust 测试覆盖成功导入、取消、Symlink、非普通文件、过大、非 UTF-8 和无效/
  加密 Key。
- Credential Summary、Debug、错误和 UI 不包含 Path 或 Key 内容。
- Android/Windows/Linux 构建继续通过。

### 当前证据

- 2026-07-27：`ApplicationCore::import_private_key_credential_from_path`
  测试覆盖成功导入、Symlink、目录、空文件、超过 1 MiB、非 UTF-8、无效 Key、
  加密 Key、Unix Socket 和错误脱敏；Unix/Android 打开最终路径组件时使用
  `O_NOFOLLOW | O_CLOEXEC`。
- 2026-07-27：Tauri Import Request 使用 `deny_unknown_fields`，并明确拒绝
  `path`、`privateKey` 和 `passphrase`。
- 2026-07-27：Playwright 与 agent-browser 验证 Browser QA DOM 没有 File
  Input 或 Passphrase Input，只创建 metadata-only Preview Summary。
- 2026-07-27：原生 Xvfb 通过真实 GTK Native File Picker 导入未加密 Ed25519
  Key；截图只显示 Label、Username 和 Private Key Kind，源文件在继续 SSH 前
  删除，Vault 文件明文扫描未发现 OpenSSH Key Header。
- 2026-07-27：Linux Container 和 Android ARM64 Container Build 均包含
  `tauri-plugin-dialog` 并通过。
- 2026-07-27：Commit `780059d` 的 GitHub Actions Run `30258051366` 九个 Job
  全部通过；远端 X11 Artifact 实际打开 GTK Native File Picker，导入后截图只
  显示 Metadata，Vault 明文扫描和临时源文件删除检查同时通过。
- 2026-07-27：Windows WebView2 Runtime 与 Android Build 继续通过该 Command
  的编译和应用启动；Windows Native Picker 交互、Reparse Point 和移动 Content
  URI 保留为平台专项测试，不改变“Path/Key/Passphrase 不进入 WebView”的已接受
  边界。

## 相关文档

- [Credential Repository v1](../design/credential-repository-v1.md)
- [原生私钥导入 v1](../design/native-private-key-import-v1.md)
- [ADR-0006](0006-secrets-stay-out-of-webview.md)
- [Phase 0 ExecPlan](../execplans/completed/0001-phase-0-technical-validation.md)
