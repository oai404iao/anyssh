# ADR-0006：秘密不得长期进入 WebView

- 状态：Accepted
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

Tauri UI 运行在 WebView 中。前端状态、DevTools、日志、错误序列化和第三方依赖都会扩大密码与私钥的暴露面。

## 决策

- 保存的密码、私钥、VMK、KEK 和数据库密钥保留在 Rust/原生层。
- 未保存 Quick Connection 可以提交一次性临时密码；它不得进入前端全局状态、
  日志或持久化，并在提交、取消、锁定和切页时清空。
- SSH 认证和签名由 Rust Core 执行。
- 前端通过 ID 引用 Credential。
- 用户查看密码时使用一次性、短 TTL 的秘密展示结果。
- Release 禁用 DevTools，使用严格 CSP 和最小 Tauri Capability。

## 备选方案

- 前端直接读取并管理秘密：实现简单，但攻击面不可接受。
- 所有秘密完全禁止显示：与产品需求冲突。

## 后果

### 正面

- 降低 XSS、日志和状态快照泄露风险。
- 安全边界清晰。

### 代价与风险

- 查看密码时仍会短暂进入 WebView 内存，无法保证完全清零。
- IPC 需要专门的 Secret Reveal 流程。
- UI 调试更复杂。

## 验证

- 常规 Host 查询 API 不返回秘密。
- 前端 Store、日志和错误对象不出现测试密码。
- Reveal 结果在超时、切页、锁屏和后台时失效。

### 当前证据

- 2026-07-27：Vault PIN 从 Tauri Request 立即移入 `Zeroizing<String>`，再通过
  私有 DB Actor Command 送入专用线程；Actor State 只长期持有
  `Option<LocalVault>`，不把 VMK、派生 Key 或解密后的记录返回 Tauri/WebView。
- 2026-07-27：SSH IPC 只接受临时密码或 Credential ID，并拒绝额外
  `privateKey`/`passphrase` 字段。Private Key 由 DB Actor 解密后经
  `anyssh-app` 直接 move 到 SSH Core；Credential Summary、Debug 和 IPC JSON
  均不包含 Secret。
- 2026-07-27：Host IPC 只返回 endpoint 与 Credential/Route ID，Jump Route
  只返回有序 Host ID；`deny_unknown_fields` 会拒绝 Host 内嵌 Password 或
  Route 内嵌 Credential。
- 2026-07-27：Private Key Import Request 只允许 Label/Username，并拒绝
  `path`、`privateKey` 和 `passphrase`。Native Picker、文件读取和 russh Key
  校验在 Rust 内完成；Browser QA DOM 不包含 File Input 或 Passphrase Input，
  原生 Xvfb 已验证导入后的 UI 只显示 metadata。
- 2026-07-27：Windows WebView2 Runtime 已验证 Credential/Host/Route CRUD 和
  重启恢复；截图、DOM Snapshot、Console/Error Log 与 Vault 明文扫描均未返回
  保存的 Password 或其他 Secret。

Quick Connection 的临时 Password 与 Vault PIN 在用户输入期间仍会短暂存在于
WebView 内存，这是当前明确的剩余风险；它们不得进入全局 Store、日志或持久化。
Secret Reveal 尚未实现，实施时必须更新 Threat Model。

## 相关文档

- [总体技术设计：分层规则](../design/technical-architecture-2026.md#分层规则)
- [Credential Repository v1](../design/credential-repository-v1.md)
- [Host 与 Jump Route Repository v1](../design/host-jump-route-repository-v1.md)
