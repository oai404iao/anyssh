# ADR-0003：SQLCipher 与记录级 AEAD 双层本地加密

- 状态：Proposed
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

项目需要防止其他普通应用直接读取 Host、密码、私钥、脚本和同步配置。仅加密敏感字段会泄露大量业务元数据；仅整库加密则不利于秘密隔离、导出和同步。

## 决策

- 使用 SQLCipher 加密整个 SQLite 数据库。
- 使用 XChaCha20-Poly1305 对密码、私钥等敏感字段二次加密。
- SQLCipher Key 与记录加密 Key 均从随机 VMK 通过 HKDF 派生。
- 用户 PIN 不直接作为数据库密码。
- 数据库访问集中在单独 DB Actor。

## 备选方案

- 仅普通 SQLite + 字段加密：会泄露 Host、Group 和索引元数据。
- 仅 SQLCipher：秘密边界和同步对象封装能力较弱。
- 直接使用平台 Keychain 保存全部记录：不适合结构化查询和大量数据。

## 后果

### 正面

- 数据库文件整体无业务明文。
- 敏感字段拥有独立生命周期和 AAD。
- 有利于实现加密导出与同步。

### 代价与风险

- 跨平台 SQLCipher 构建和迁移更复杂。
- 双层加密增加密钥管理和测试成本。
- 必须正确处理 WAL、临时页和备份。

## 验证

- 四平台创建、迁移和重启解锁。
- 数据库文件中搜索不到测试 Host、用户名和密码。
- 模拟迁移中断后可以恢复。
- 旧 Key Slot 轮换不需要重加密全部数据库。

### 当前证据

- 2026-07-26：Linux bundled SQLCipher 4.10.0 community 创建、重启解锁通过。
- 2026-07-26：测试 Host、用户名、密码、SQLite 明文 Header 在数据库、WAL、
  Sidecar 和 Bootstrap 中均未检出。
- 2026-07-26：Credential 密码使用独立 XChaCha20-Poly1305 AAD 加密。
- 2026-07-26：Schema migration 中断事务回滚通过。
- Linux WebKitGTK 同时引入系统 SQLite；bundled SQLCipher 的符号共存需要继续
  验证，当前未因此把本 ADR 提升为 Accepted。
- Windows、Android 和 iOS 构建证据仍缺失，因此 ADR 保持 Proposed。

## 相关文档

- [总体技术设计：本地 Vault](../design/technical-architecture-2026.md#8-本地加密-vault)
- [Vault Bootstrap v1](../design/vault-bootstrap-v1.md)
- [ADR-0005](0005-vmk-multiple-key-slots.md)
