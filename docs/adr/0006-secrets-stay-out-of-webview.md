# ADR-0006：秘密不得长期进入 WebView

- 状态：Proposed
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

Tauri UI 运行在 WebView 中。前端状态、DevTools、日志、错误序列化和第三方依赖都会扩大密码与私钥的暴露面。

## 决策

- 密码、私钥、VMK、KEK 和数据库密钥保留在 Rust/原生层。
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

## 相关文档

- [总体技术设计：分层规则](../design/technical-architecture-2026.md#分层规则)
