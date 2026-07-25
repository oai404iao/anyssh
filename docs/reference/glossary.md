# 术语表

| 术语 | 含义 |
| --- | --- |
| ADR | Architecture Decision Record，长期架构决策记录 |
| ExecPlan | Agent 可执行并持续更新的实施计划 |
| VMK | Vault Master Key，随机生成的 Vault 根密钥 |
| KEK | Key Encryption Key，用于包装其他密钥的密钥 |
| Key Slot | 使用平台密钥、PIN 或恢复密钥包装 VMK 的记录 |
| AEAD | 同时提供机密性和完整性校验的加密模式 |
| HLC | Hybrid Logical Clock，用于跨设备确定性排序 |
| LWW | Last-Writer-Wins，基于逻辑时间的冲突合并策略 |
| Tombstone | 表示对象已删除并参与同步的逻辑记录 |
| Outbox | 本地事务中产生、等待同步的 Operation 队列 |
| TOFU | Trust On First Use，首次连接确认 Host Key |
| Jump Host | 通过 SSH `direct-tcpip` Channel 访问下一跳的主机 |
| Dynamic Forward | 通过 SSH 暴露本地 SOCKS 服务的转发方式 |
| Step-up Authentication | 查看或导出秘密前再次要求用户认证 |
| WebView | Tauri 中运行 React/xterm.js UI 的平台网页视图 |
