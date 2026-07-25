# ADR-0004：WebDAV 同步不可变操作日志，不同步数据库

- 状态：Proposed
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

直接上传 SQLCipher 数据库会导致多设备整文件冲突、中断损坏和无法进行字段级合并。WebDAV 服务实现差异较大，也不能假设存在可靠的全局锁。

## 决策

- 本地领域修改生成加密 Operation/Outbox。
- 每台设备只写自己的不可变 Operation 序列。
- 使用 Snapshot 压缩历史。
- 使用 HLC + Device ID 确定性合并。
- 可变 Head 使用 ETag/`If-Match` CAS。
- 服务端支持 Sync Collection 时增量同步，否则使用 `PROPFIND`。

## 备选方案

- 同步 SQLCipher 文件：冲突粒度和可靠性不可接受。
- 单一共享 JSON 文件：仍有全文件竞争和数据量问题。
- 立即引入通用 CRDT 框架：超出当前数据模型需求，审计成本高。

## 后果

### 正面

- 离线优先。
- 多设备写入冲突更少。
- 可扩展到 S3 和自托管 Provider。

### 代价与风险

- 需要设计格式版本、Tombstone、快照和垃圾回收。
- 恶意服务端仍可删除或回滚数据。
- 新设备的完整回滚防护需要额外信任基础设施。

## 验证

- 两设备离线并发修改后确定性收敛。
- 密码、私钥和脚本并发修改产生冲突副本。
- 弱 ETag、无 Sync Collection 和中断上传场景可恢复。

## 相关文档

- [总体技术设计：WebDAV](../design/technical-architecture-2026.md#9-webdav-端到端加密同步)
