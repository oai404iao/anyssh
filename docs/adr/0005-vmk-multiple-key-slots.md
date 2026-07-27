# ADR-0005：VMK 使用多个 Key Slot 解锁

- 状态：Proposed
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

同一个 Vault 需要支持平台密钥环、生物识别、PIN、同步密码和恢复密钥。生物识别状态可能因设备重置或重新录入而失效，不能成为唯一恢复方式。

## 决策

- 创建随机 256-bit VMK。
- VMK 不直接落盘。
- 每种解锁方式创建独立 Key Slot：
  - Platform Slot
  - PIN Slot
  - Sync Slot
  - Recovery Slot
- 生物识别只授权平台密钥解包 VMK。
- PIN 和同步密码使用 Argon2id 派生 KEK。

## 备选方案

- PIN 直接派生全部业务密钥：更换 PIN 需要重加密全部数据。
- 只依赖平台 Keychain：设备迁移和同步恢复困难。
- 把生物特征当作密钥材料：平台 API 不提供也不应这样使用。

## 后果

### 正面

- 可独立添加、删除和轮换解锁方式。
- 更换 PIN 或同步密码只需要重包 VMK。
- 生物识别 Slot 失效后仍可恢复。

### 代价与风险

- Key Slot 格式必须版本化。
- 必须测试平台状态变化和 Slot 恢复。
- Recovery Code 丢失且无可用设备时数据不可恢复。

## 验证

- PIN、平台认证和 Recovery Slot 均能解锁同一 Vault。
- 删除某个 Slot 不影响其他 Slot。
- 生物信息变化后可以通过 PIN 恢复并创建新 Platform Slot。

### 当前证据

- 2026-07-26：随机 256-bit VMK、版本化 PIN Slot 和 Argon2id KEK 已实现。
- 2026-07-26：正确 PIN、错误 PIN、损坏 Ciphertext 和不安全 KDF 参数测试通过。
- 2026-07-27：锁定命令会在 DB Actor Thread 中销毁其独占的 `LocalVault`，
  随后可用同一 PIN 重新解包 VMK；错误 PIN 不改变 Locked 状态。
- Platform、Sync 和 Recovery Slot 尚未实现，因此 ADR 保持 Proposed。

## 相关文档

- [ADR-0003](0003-double-layer-local-encryption.md)
- [总体技术设计：密钥层次](../design/technical-architecture-2026.md#82-密钥层次)
- [Vault Bootstrap v1](../design/vault-bootstrap-v1.md)
