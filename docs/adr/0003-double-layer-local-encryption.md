# ADR-0003：SQLCipher 与记录级 AEAD 双层本地加密

- 状态：Accepted
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
- 2026-07-27：`anyssh-storage` 专用 DB Actor 已接管 Vault 生命周期和
  SQLCipher Connection；容量 16 的有界 Queue、oneshot Response、串行命令、
  Shutdown 和 Thread Join 测试通过。
- 2026-07-27：Schema v2 `credentials` 表已实现 Password、Private Key 和
  Passphrase 独立 Record AEAD；Schema v1 自动迁移、中断回滚、重启恢复和明文
  扫描通过。
- 2026-07-27：Schema v3 已把旧 Host 内嵌 Password 原子迁移为 Password
  Credential；新 Host/Jump Route 只保存 ID 引用。v2 -> v3 中断回滚、重启恢复、
  引用占用和循环检测通过。
- 2026-07-27：Schema v4 已新增 Group 与三态 Override。v3 -> v4 语义保持、
  State/Value CHECK、中断回滚、重启和 Group 元数据明文扫描通过。
- Linux WebKitGTK 同时引入系统 SQLite；当前 X11/Wayland 长流程已在同一进程
  持续执行 Vault 与 SSH，未观察到 bundled SQLCipher 符号冲突。
- Windows Run `30270414706` 已通过真实 SQLCipher Vault 创建、错误 PIN、
  Lock/Unlock、Repository 写入、进程重启恢复和数据库 Header/明文扫描；EXE
  同时包含 SQLCipher 符号。
- Android ARM64 APK 已包含 bundled SQLCipher Marker；iOS Evidence 仍延期。

Linux 与 Windows Runtime、Android Build、Record AEAD 和 Migration 恢复证据
已足以接受双层本地加密决策。iOS 与未来平台 Slot 继续作为平台验证债务。

## 相关文档

- [总体技术设计：本地 Vault](../design/technical-architecture-2026.md#8-本地加密-vault)
- [Vault Bootstrap v1](../design/vault-bootstrap-v1.md)
- [Credential Repository v1](../design/credential-repository-v1.md)
- [Host 与 Jump Route Repository v1](../design/host-jump-route-repository-v1.md)
- [ADR-0005](0005-vmk-multiple-key-slots.md)
