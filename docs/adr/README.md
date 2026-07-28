# Architecture Decision Records

ADR 记录需要长期保留的架构决策。实现细节放在 `docs/design/`，任务进度放在 `docs/execplans/`。

## 状态定义

- **Proposed**：候选方案，等待验证或负责人确认。
- **Accepted**：当前有效决策。
- **Rejected**：已评估但未采用。
- **Deprecated**：仍可能存在，但不应继续扩展。
- **Superseded**：已被后续 ADR 替代。

## 决策索引

| ADR | 状态 | 决策 |
| --- | --- | --- |
| [0001](0001-unified-tauri-react-shell.md) | Accepted | 使用 Tauri 2 + React 作为统一应用壳 |
| [0002](0002-russh-as-default-ssh-engine.md) | Accepted | 使用 russh 作为默认 SSH Engine |
| [0003](0003-double-layer-local-encryption.md) | Accepted | SQLCipher 与记录级 AEAD 双层本地加密 |
| [0004](0004-webdav-operation-log-sync.md) | Proposed | WebDAV 同步不可变操作日志，不同步数据库 |
| [0005](0005-vmk-multiple-key-slots.md) | Proposed | VMK 使用多个 Key Slot 解锁 |
| [0006](0006-secrets-stay-out-of-webview.md) | Accepted | 秘密不得长期进入 WebView |
| [0007](0007-modern-ssh-algorithm-policy.md) | Proposed | 默认现代 SSH 算法，Legacy 按 Host 开启 |
| [0008](0008-no-arbitrary-local-scripting-in-mvp.md) | Accepted | MVP 不允许任意本地脚本执行 |
| [0009](0009-host-jump-route-reference-model.md) | Accepted | Host 与 Jump Route 只保存 ID 引用 |
| [0010](0010-saved-host-plans-resolve-in-rust.md) | Accepted | Saved Host Connection Plan 只在 Rust 内解析 |
| [0011](0011-native-private-key-import-stays-in-rust.md) | Accepted | 原生私钥导入完全留在 Rust 边界 |
| [0012](0012-group-inheritance-uses-explicit-three-state-overrides.md) | Accepted | Group 继承使用显式三态 Override |
| [0013](0013-system-ssh-agent-uses-fingerprint-selected-identities.md) | Accepted | 系统 SSH Agent 使用 Fingerprint 选择的外部签名身份 |
| [0014](0014-encrypted-private-key-passphrase-stays-out-of-webview.md) | Accepted | 加密私钥 Passphrase 使用原生安全提示且不进入 WebView |
| [0015](0015-known-host-trust-is-endpoint-scoped.md) | Accepted | Known Host 信任按 Endpoint 建模并在继续握手前持久化 |
| [0016](0016-keyboard-interactive-responses-are-session-bound.md) | Accepted | Keyboard-interactive 响应是 Session-bound 临时秘密 |
| [0017](0017-session-tabs-own-independent-runtime-lifecycles.md) | Accepted | Session Tab 拥有独立的 Runtime Lifecycle |
| [0018](0018-port-forwarding-is-rust-owned-and-session-scoped.md) | Proposed | SSH Port Forwarding 由 Rust 拥有并绑定 Session |

Phase 0 已接受 ADR-0001、0002、0003、0006、0008、0009、0010 和 0011；
Phase 1 Group、System Agent 和加密 Private Key Prompt 验证后已接受
ADR-0012、0013、0014；Known Host Repository 与 Durable TOFU 验证后已接受
ADR-0015；Keyboard-interactive/OTP 验证后已接受 ADR-0016；Multi Tab Session
Lifecycle 验证后已接受 ADR-0017。ADR-0004、0005 和 0007 因对应能力尚未完整
实现而继续保持 Proposed；ADR-0018 等待 SSH Port Forwarding 验证。

## 文件规范

新 ADR 使用：

```text
NNNN-short-kebab-case-title.md
```

建议从 [`0000-template.md`](0000-template.md) 复制。
