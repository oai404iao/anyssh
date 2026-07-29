# ADR-0019：Private Key 生成与导出留在 Rust/原生边界

- 状态：Proposed
- 日期：2026-07-29
- 决策人：项目维护者

## 背景

AnySSH 已能通过原生 File Picker 导入 OpenSSH Private Key，并把 Key 与可选
Passphrase 保存在 SQLCipher + Record AEAD 中。Desktop MVP 仍缺少应用内 Key
生成、Public Key 查看和 Private Key 导出。

Private Key 生成或导出若经过 WebView、普通 Tauri JSON、Shell、系统
`ssh-keygen` 或可记录的命令行参数，会让 Key、PIN、Export Passphrase 或文件
Path 越过现有安全边界。未加密导出还容易把长期私钥直接暴露在用户文件系统中。

## 决策

- Private Key 生成、解析、解密、重新加密和文件写入全部由 Rust/原生代码完成，
  不调用系统 `ssh-keygen`，不把 Private Key 文本发送到 WebView。
- v1 生成算法为：
  - Ed25519，默认。
  - RSA 4096，作为兼容选项。
  DSA、SHA-1 RSA 和自定义低位数 RSA 不提供。
- 生成的 Key 使用 CSPRNG，在 Rust Blocking Task 中完成，并作为现有
  `private_key` Credential 写入 Vault。v1 不新增 Schema；Vault 内部保护依赖
  SQLCipher 与 Record AEAD，不额外要求生成时设置 OpenSSH Passphrase。
- Public Key 不是秘密。WebView 可按 Credential ID 请求受限 Public Projection：
  Algorithm、SHA-256 Fingerprint 和单行 OpenSSH Public Key。Private Key、
  Stored Passphrase 和 Key Path 不包含在 Projection 中。
- Private Key Export 只接受 Credential ID，并要求：
  1. 进程内/系统原生 PIN Step-up。
  2. 进程内/系统原生新 Export Passphrase 与确认。
  3. Rust-owned Native Save Picker。
- v1 只导出使用新 Passphrase 加密的 OpenSSH Private Key。不得复用或显示保存的
  Key Passphrase，也不提供未加密导出。
- Export Destination Path 不进入 WebView、普通 IPC、日志或遥测。v1 只创建新
  文件，不覆盖已有文件，不跟随 Symlink/Reparse Point；Unix 权限为 `0600`，
  Windows 使用当前用户受限 ACL。
- 私钥明文不在 WebView 中 Reveal 或复制到 Web Clipboard。路线图中的
  “Reveal”在 v1 指 Public Key/Fingerprint Reveal；Private Key 通过受控加密导出
  交付。
- Browser QA 只模拟生成后的 Credential/Public Metadata，不生成 Key、不打开
  Picker、不写文件。
- Android/iOS v1 构建保留类型边界，但在专用安全 Prompt、Document Provider /
  Share Sheet 实现前明确返回 Unsupported。

## 备选方案

- WebView 生成 Key：扩大 Renderer 攻击面并让随机数、Key 和错误路径进入前端，
  拒绝。
- 调用系统 `ssh-keygen`：引入 Shell/Path/环境变量与子进程泄漏，拒绝。
- 把 Private Key Reveal 到 React Modal 或 Clipboard：破坏 Secret 不进入
  WebView 的既有边界，拒绝。
- 默认导出未加密 Key：容易产生长期明文 Secret，拒绝。
- 直接覆盖用户选择的现有文件：需要更复杂的身份确认、备份和原子替换语义，
  v1 拒绝。
- 为 Generated Key 新增独立表：现有 Private Key Credential 已能表达同一
  Secret，不需要 Schema Migration。

## 后果

### 正面

- Key 生命周期延续现有 Rust/Vault 安全边界。
- Public Key 可用于部署和核验，同时 Private Key 不进入 WebView。
- Export 默认且强制加密，降低文件系统明文泄漏风险。
- Imported 与 Generated Private Key 共用 Credential、Host、Group 和 Jump
  Route 引用模型。

### 代价与风险

- Desktop 需要新的 Native PIN、Passphrase Confirmation 和 Save Picker 流程。
- PIN Step-up 必须由 DB Actor 验证，不能把 VMK/KEK 或 PIN 暴露给 Tauri
  Command。
- RSA 4096 生成可能较慢，必须离开 UI/DB Actor 热路径并支持取消/错误边界。
- 文件创建需要跨平台处理 Symlink/Reparse Point、权限、取消和部分写入清理。

## 验证

- Rust Unit Test 验证 Ed25519/RSA 4096、Public Projection、Debug Redaction 和
  错误 Algorithm。
- Generated Credential 通过真实 OpenSSH Password-independent Public Key
  Authentication。
- Existing Imported Encrypted Key 与 Generated Key 都能导出为新 Passphrase
  加密的 OpenSSH Key；错误 Passphrase 不能解密。
- PIN 错误、Prompt 取消、Passphrase 不匹配、已有 Destination、写入失败和
  Vault Lock 均不留下文件或新 Credential。
- IPC JSON 拒绝 `privateKey`、`passphrase`、`pin`、`path`、`payload` 和
  `command`。
- Browser、X11、Wayland、Windows、Android/Linux Container 与同 Commit CI；
  Vault/Log/Evidence 扫描不包含生成或导出的 Private Key、PIN 和 Passphrase。

## 相关文档

- Design：[Private Key Generation and Encrypted Export v1](../design/private-key-generation-and-encrypted-export-v1.md)
- ExecPlan：[Private Key Generation and Encrypted Export](../execplans/active/0009-private-key-generation-and-encrypted-export.md)
- ADR：[ADR-0006](0006-secrets-stay-out-of-webview.md)
- ADR：[ADR-0007](0007-modern-ssh-algorithm-policy.md)
- ADR：[ADR-0011](0011-native-private-key-import-stays-in-rust.md)
- ADR：[ADR-0014](0014-encrypted-private-key-passphrase-stays-out-of-webview.md)
- Supersedes：
- Superseded by：
