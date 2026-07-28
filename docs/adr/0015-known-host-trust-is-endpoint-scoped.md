# ADR-0015：Known Host 信任按 Endpoint 建模并在继续握手前持久化

- 状态：Accepted
- 日期：2026-07-28
- 接受日期：2026-07-28
- 决策人：项目维护者

## 背景

当前 SSH Core 已支持首次 TOFU Prompt、已知 SHA-256 Fingerprint 免提示和变化
硬阻断，但 `ApplicationCore` 对每一跳仍固定使用 `HostKeyPolicy::Prompt`。
用户接受的 Host Key 不会写入 Vault，重启或再次连接时仍需重复确认。

Known Host 是防止网络 MITM 的安全状态。它必须同时适用于 Quick Connection、
Saved Host、Jump Host 和 Target，不能因为 Host UI 记录、Group 或 Route 的变化
而复制、遗失或静默替换。

## 决策

- Known Host 身份使用规范化的逻辑 SSH Endpoint：`canonical host + explicit
  port`。
- Trust 不绑定 Host ID、Group ID、Jump Route ID，也不绑定 DNS 解析后的 IP。
- SQLCipher Schema v6 为每个 Endpoint 保存一个 CSPRNG ID 的 Known Host
  Entity，并在子表保存最多 16 个完整 Public Key。
- 每个 Key 保存规范化 Public Key Bytes、Algorithm 和 SHA-256 Fingerprint；
  Actor 必须解析 Bytes 并重新计算后再接受或加载记录。
- Public Host Key 不是 Credential Secret，因此不增加 Record AEAD；Endpoint、
  Key 和 Fingerprint 仍只存在于 SQLCipher 数据库。
- DB Actor 在创建连接计划时返回 Rust-only Trust Policy：
  - Endpoint 没有记录：`Prompt`。
  - Endpoint 有记录：只接受该有界 Key Set。
- 首次 Prompt Event 继续只把 Request ID、Hop、Endpoint、Algorithm 和
  Fingerprint 发给 WebView。完整 Public Key Bytes 留在 SSH Core/Application
  Core。
- 用户选择“Trust and continue”时：
  1. `ApplicationCore` 通过 Request ID 取得仍在等待的完整 Observed Key。
  2. DB Actor 在 Transaction 中创建或验证 Known Host。
  3. 只有持久化成功后才向 SSH Core 发送接受决定。
- 并发首次连接采用 First-writer-wins：
  - 相同 Endpoint + 相同 Key 幂等成功。
  - 相同 Endpoint 已存在不同 Key 时拒绝第二次 TOFU，不把两个观察结果静默合并。
- 已知 Endpoint 收到未受信任 Key 时硬阻断并发送 typed Changed-Key Event；
  当前连接界面不得提供“仍然接受”按钮。
- 用户只能在独立 Known Hosts 管理界面请求 Forget 整个 Endpoint。
  `ApplicationCore` 必须先通过 WebView 外的原生确认 Provider 展示 Endpoint 和
  Fingerprint；确认后才删除。删除后下一次连接重新进入 TOFU。
- v1 不直接读取或修改系统 `~/.ssh/known_hosts`，不启动 `ssh-keygen`。
- OpenSSH 文件导入/导出、Hash/Pattern、`@revoked`、`@cert-authority` 和 Host
  Certificate 留给后续计划；v1 保存完整 Key Bytes 以避免未来格式受限。
- 未来同步必须把一个 Endpoint 的 Trust Set 作为原子领域状态；并发出现不同
  Trust Set 时进入冲突并阻断连接，不允许自动取并集。

## Endpoint 规范化 v1

- 去除首尾空白。
- 有效 IP Literal 使用 `std::net::IpAddr` 的规范字符串。
- 仅当方括号内部是有效 IPv6 时移除 `[]`。
- DNS 风格名称移除一个末尾的 `.`，并做 ASCII 小写。
- 不执行 DNS Lookup、CNAME 展开、反向解析或 Unicode IDNA 重写。
- Port 始终参与身份；`host:22` 与 `host:2222` 是不同 Known Host。

规范化规则属于持久化格式。后续改变必须使用显式 Schema/Sync Migration，不能
在读取时静默重新解释旧记录。

## 备选方案

- 把 Fingerprint 直接加到 Host 表：同一 Endpoint 的多个 Host/Jump 引用会复制
  Trust，Host 改名或重建也会丢失身份，拒绝。
- 只保存 SHA-256 Fingerprint：足以完成当前比较，但无法可靠导出 OpenSSH
  Public Key 或验证冗余字段，拒绝。
- 直接使用用户 `~/.ssh/known_hosts`：不适合 Windows/Android/iOS、Vault、
  DB Actor 和未来 E2EE Sync，拒绝。
- 接受后先继续握手，再异步写库：数据库失败或进程崩溃会产生“已信任但未保存”
  窗口，拒绝。
- Changed-Key Dialog 提供“Accept Anyway”：会把最重要的 MITM 硬阻断退化为
  普通确认，拒绝。
- WebView 直接调用无二次确认的 Delete Command：被攻陷的 WebView 可先删除旧
  Trust 再自动接受 MITM Key，拒绝。
- 自动把并发观察到的不同 Key 加入同一 Trust Set：攻击者可借竞态扩大信任，
  拒绝。

## 后果

### 正面

- Quick、Saved、Jump 和 Target 共享一致、可重启恢复的 Endpoint Trust。
- Host/Group/Route 重构不会自动改变 Host Key 信任。
- 数据模型支持未来多 Algorithm 和 OpenSSH Export。
- 数据库失败、并发 TOFU 和 Changed-Key 均 Fail Closed。

### 代价与风险

- Schema 升级到 v6，并新增两张表、Actor Command 和迁移回滚测试。
- SSH Core 需要保存有界 Pending Public Key Evidence，Application Core 需要
  编排“先持久化、后继续握手”。
- 相同 DNS 服务通过不同 Alias 访问时各自建立 Trust，这是避免错误合并的安全
  取舍。
- 如果服务端协商了尚未受信任的新 Host Key Algorithm，即使它仍提供旧 Key，
  v1 也会硬阻断；基于 Known Key 的 Algorithm 重排属于单独验证项。
- Forget 操作会让下一次连接重新进入 TOFU，需要平台原生确认 Adapter 和 Native
  QA。

## 验证

- Schema v5 -> v6 成功、重启和模拟中断回滚。
- Endpoint 规范化、Port 隔离、最多 16 Key 和 Public Key 字段一致性。
- 首次接受后无需再次 Prompt；锁定/解锁和进程重启后仍生效。
- DB 持久化失败时 SSH 不继续。
- 两个并发首次连接观察相同 Key 时幂等；不同 Key 时只有第一个可成为 Trust。
- Host Key 轮换产生 typed Changed-Key，且不出现接受入口、不改写数据库。
- WebView 单独提交 Known Host ID 不能删除 Trust；原生 Forget Prompt 取消时
  Repository 不变。
- 两层 Jump Route 的每一跳独立持久化。
- Browser、X11、Wayland、Windows、OpenSSH、Android/Linux Container 和同
  Commit CI 回归。

Head `a75da9cf6d4ba73f8b93257c683fb97ad2c0b90f` 的 GitHub Actions Run
`30344638562` 九个 Job 全部通过。Linux X11 和 Windows 真实 Runtime 验证了
首次 TOFU、二次免提示、原生 Forget、重新 TOFU、重启恢复和同 Endpoint Host
Key Rotation 硬阻断；Wayland 验证了无 `DISPLAY` 的 Durable Trust 回归。
Artifact 人工检查、空 Browser Error Log、SQLCipher 明文扫描和测试 Secret
二次扫描均通过，因此本 ADR 于 2026-07-28 接受。

## 相关文档

- Design：[Known Host Repository v1](../design/known-host-repository-v1.md)
- Reference：[OpenSSH `known_hosts` 2026 基线](../reference/openssh-known-hosts-baseline-2026.md)
- ExecPlan：[Known Host Repository and Durable TOFU](../execplans/completed/0005-known-host-repository-and-durable-tofu.md)
- ADR：[ADR-0002](0002-russh-as-default-ssh-engine.md)
- ADR：[ADR-0010](0010-saved-host-plans-resolve-in-rust.md)
- Supersedes：
- Superseded by：
