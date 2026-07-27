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
| [0001](0001-unified-tauri-react-shell.md) | Proposed | 使用 Tauri 2 + React 作为统一应用壳 |
| [0002](0002-russh-as-default-ssh-engine.md) | Proposed | 使用 russh 作为默认 SSH Engine |
| [0003](0003-double-layer-local-encryption.md) | Proposed | SQLCipher 与记录级 AEAD 双层本地加密 |
| [0004](0004-webdav-operation-log-sync.md) | Proposed | WebDAV 同步不可变操作日志，不同步数据库 |
| [0005](0005-vmk-multiple-key-slots.md) | Proposed | VMK 使用多个 Key Slot 解锁 |
| [0006](0006-secrets-stay-out-of-webview.md) | Proposed | 秘密不得长期进入 WebView |
| [0007](0007-modern-ssh-algorithm-policy.md) | Proposed | 默认现代 SSH 算法，Legacy 按 Host 开启 |
| [0008](0008-no-arbitrary-local-scripting-in-mvp.md) | Proposed | MVP 不允许任意本地脚本执行 |
| [0009](0009-host-jump-route-reference-model.md) | Proposed | Host 与 Jump Route 只保存 ID 引用 |
| [0010](0010-saved-host-plans-resolve-in-rust.md) | Proposed | Saved Host Connection Plan 只在 Rust 内解析 |

Phase 0 完成后，应根据实验证据更新这些 ADR 的状态。

## 文件规范

新 ADR 使用：

```text
NNNN-short-kebab-case-title.md
```

建议从 [`0000-template.md`](0000-template.md) 复制。
