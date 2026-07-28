# Known Host Repository v1

> 状态：已实现，等待同 Commit CI 与 ADR-0015 状态评审
> 日期：2026-07-28

本文定义 Phase 1 Desktop MVP 的 Endpoint-scoped Known Host Repository、持久化
TOFU、Changed-Key 阻断和管理 UI。长期决策见 Proposed ADR-0015。

## 目标

- 首次确认的 Host Key 安全写入 SQLCipher。
- Quick Connection、Saved Host、Jump Host 和 Target 使用同一 Trust Store。
- 再次连接和进程重启后，匹配 Key 不再提示。
- Key 变化继续硬阻断，不允许从连接错误界面直接覆盖。
- WebView 只接收 Host Key 展示元数据，不接收完整 Public Key Bytes。
- 数据模型保留后续 OpenSSH `known_hosts` 导入/导出的能力。

## 非目标

- OpenSSH 文件导入/导出。
- Wildcard、Negation、Hashed Host Pattern。
- `@revoked`、`@cert-authority` 和 Host Certificate。
- `HostKeyAlias`、DNS/CNAME/IP 自动合并。
- 自动 Host Key Rotation 或 `hostkeys@openssh.com` 更新。
- WebDAV Operation/Outbox 实现。
- 在 Changed-Key Dialog 中提供接受或替换。

## 安全不变量

1. 没有 Known Host Record 时才允许 TOFU Prompt。
2. Endpoint 已有 Trust 时，任何未匹配 Key 都必须硬阻断。
3. 接受决定必须在数据库持久化成功之后送达 SSH Worker。
4. WebView 的 `accepted: true` 不能携带 Endpoint、Fingerprint 或 Key Bytes。
5. Request ID 必须仍绑定 Session、Hop、Endpoint 和 Observed Key。
6. TOFU 不能向已存在的 Endpoint Trust Set 增加不同 Key。
7. Key Blob、Algorithm 和 Fingerprint 必须可相互验证。
8. Vault Locked 时不能读取、创建或删除 Known Host。
9. WebView 不能在没有原生用户确认的情况下 Forget 已保存 Trust。

## 领域模型

```rust
struct KnownHostSummary {
    id: String,
    host: String,
    port: u16,
    keys: Vec<KnownHostKeySummary>,
}

struct KnownHostKeySummary {
    algorithm: String,
    fingerprint_sha256: String,
}

enum ResolvedHostKeyPolicy {
    Prompt,
    RequireSha256Set(Vec<String>),
}
```

Summary 可序列化到 WebView，但不包含 Public Key Bytes。Runtime Policy 和
Resolved Connection Plan 是 Rust-only 类型。

单个 Endpoint 最多保存 16 个 Key。v1 的 TOFU 只创建一个 Key；有界集合为后续
OpenSSH Import、多 Algorithm 和显式 Key Rotation 保留空间。

## Endpoint 规范化

新增统一的 Endpoint Identity Normalizer，供 DB Actor 的 Lookup、Insert 和
Delete 使用：

```text
" EXAMPLE.COM. " + 22 -> "example.com" + 22
"[2001:0db8::1]" + 22 -> "2001:db8::1" + 22
"2001:0db8::1" + 2222 -> "2001:db8::1" + 2222
```

规则：

- 规范化逻辑 Host，不解析网络地址。
- IPv4/IPv6 使用 `IpAddr::to_string()`。
- DNS 风格文本只做 ASCII 小写和单个末尾点移除。
- 不做 Unicode IDNA、CNAME、反向 DNS 或 Jump Route Alias 合并。
- Port 总是显式参与唯一键。

## Schema v6

```sql
CREATE TABLE known_hosts(
    id TEXT PRIMARY KEY NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
    UNIQUE(host, port)
) WITHOUT ROWID;

CREATE TABLE known_host_keys(
    known_host_id TEXT NOT NULL,
    fingerprint_sha256 TEXT NOT NULL,
    algorithm TEXT NOT NULL,
    public_key BLOB NOT NULL,
    PRIMARY KEY(known_host_id, fingerprint_sha256),
    FOREIGN KEY(known_host_id)
        REFERENCES known_hosts(id) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX known_host_keys_algorithm_idx
    ON known_host_keys(known_host_id, algorithm);
```

要求：

- Known Host ID 使用 Rust CSPRNG，前缀 `known-`。
- `public_key` 使用 `ssh_key::PublicKey::to_bytes()` 的规范二进制。
- 写入和读取时用 `PublicKey::from_bytes()` 重算 Algorithm/Fingerprint。
- 相同 Endpoint 只有一个 Entity。
- Key 顺序不表达优先级；List 按 Algorithm/Fingerprint 确定排序。
- SQLCipher 提供静态加密；Public Key 不再增加 Record AEAD。

Schema v5 -> v6 只新增表，不重建 Credential/Group/Host/Route。Migration 仍使用
`IMMEDIATE` Transaction，并提供模拟中断回滚和旧数据重启恢复测试。

## DB Actor API

新增有界命令：

```text
ResolveKnownHostPolicy(endpoint)
TrustObservedHostKey(endpoint, algorithm, fingerprint, public_key)
ListKnownHosts
DeleteKnownHost(id)
```

`ResolveHostConnectionPlan` 同时为 Target 和每个 Jump Host 解析 Policy，保证
Endpoint、Credential、Route 和 Known Host 来自同一个 Actor Snapshot。

Quick Connection 在网络启动前单独调用 `ResolveKnownHostPolicy`。

### First-writer-wins

`TrustObservedHostKey` 在单个 Transaction 中：

1. 规范化 Endpoint。
2. 解析 Public Key Bytes 并重算 Algorithm/Fingerprint。
3. Endpoint 不存在时创建 Entity 和第一个 Key。
4. Endpoint 已包含相同 Key 时幂等成功。
5. Endpoint 已存在但不包含该 Key 时返回 `KnownHostConflict`。

DB Actor 已串行处理 Command，因此两个同时等待的 TOFU Session 会得到确定顺序；
第二个不同 Key 不会被追加到 Trust Set。

## SSH Core Boundary

现有 `SessionEvent::HostKey(HostKeyInfo)` 保持 metadata-only。新增 Rust-only：

```rust
struct ObservedHostKey {
    request_id: u64,
    hop: SessionHop,
    endpoint: SshEndpoint,
    algorithm: String,
    fingerprint_sha256: String,
    public_key: Vec<u8>,
}
```

`ObservedHostKey`：

- 不实现 Serialize。
- Debug 不输出 Public Key Bytes。
- 大小有硬上限。
- 只在 Request 仍 Pending 时可由 `SessionControl` 读取。

`HostKeyPolicy` 从单 Fingerprint 扩展为有界集合。SSH Handler：

- 集合中任一 SHA-256 Fingerprint 匹配即继续。
- 集合非空但不匹配时保存 Changed-Key Evidence 并拒绝握手。
- `Prompt` 时把完整 Key 放入 Pending Slot，只发 metadata Event。

## ApplicationCore 决策流

Tauri 不直接编排持久化。Command 取得 `SessionControl` 后调用：

```text
ApplicationCore::decide_host_key(control, request_id, accepted)
```

拒绝：

```text
WebView false
  -> SessionControl rejects pending request
  -> no database mutation
```

接受：

```text
WebView true
  -> ApplicationCore reads pending ObservedHostKey
  -> DatabaseActor TrustObservedHostKey
  -> success: SessionControl accepts
  -> failure/conflict: SessionControl rejects and connection closes
```

这样 WebView 仍只提交 Session ID、Request ID 和 Boolean。它不能替换 Endpoint、
Fingerprint 或 Key。

## Changed-Key Flow

Known Endpoint 收到不同 Key 时发送 typed Event：

```text
HostKeyChanged {
  hop,
  host,
  port,
  algorithm,
  receivedFingerprintSha256,
  trustedFingerprintsSha256
}
```

UI：

- 明确显示 Jump Index 或 Target。
- 同时显示已信任和收到的 Fingerprint。
- 说明连接已阻断。
- 不显示 Accept/Replace。
- 提供关闭和导航到 Known Hosts 管理页。

管理页只允许显式 `Forget` 整个 Endpoint。删除不会修改 Host/Group/Route；
下一次连接重新 TOFU。

Forget 属于安全降级操作，不能只有 WebView Modal：

```text
WebView requests { knownHostId }
  -> ApplicationCore resolves metadata
    -> KnownHostForgetPrompt outside WebView
      -> confirmed: DB Actor deletes
      -> cancelled/unavailable: no mutation
```

Prompt Context 只包含经过长度和控制字符约束的 Endpoint 与 Fingerprint，不包含
Public Key Bytes。Linux/Windows 由 Tauri 提供原生确认 Adapter；Browser QA 只
模拟 Result。

## IPC 与 UI

新增 typed Commands：

- `known_host_list`
- `known_host_forget`

新增/修改：

- `ssh_confirm_host_key` 调用 `ApplicationCore::decide_host_key`。
- Host Key Prompt 的主操作文案改为 `Trust and continue`。
- Configuration Workspace 增加 Known Hosts 页面。
- Forget Command 只接受 Known Host ID，并通过 ApplicationCore/Native Provider
  授权。
- Browser QA 只模拟 Summary、Prompt、Changed-Key 和 Forget，不读取文件或联网。

Public Key Bytes 不进入：

- Tauri Request/Response。
- React State。
- Browser QA Fixture。
- Error/Debug/Console。

## 同步边界

当前 v1 只写本地 SQLCipher，不创建 Outbox。未来同步必须使用原子
`PutKnownHostTrustSet`/等价 Operation：

- Endpoint 和完整 Key Set 同版本更新。
- 两台设备对同一 Endpoint 产生不同 Trust Set 时创建冲突。
- 冲突状态下 Endpoint 必须阻断连接。
- 不允许用集合并集自动信任两边 Key。
- 不同步 SQLCipher 文件。

## 失败与上限

- 每个 Endpoint 最多 16 个 Key。
- Public Key Bytes 使用有界长度。
- 空/无效 Endpoint、Key、Algorithm、Fingerprint 返回通用错误。
- Repository 数据损坏返回 Record Integrity，不启动网络。
- Vault Lock 会先断开 Session；Locked Repository Command 一律拒绝。
- DB 写入失败、Actor 不可用、Request 过期和并发冲突均不继续握手。
- Native Forget Prompt 取消或不可用时不删除 Trust。

## 验证

### Storage/Application

- v5 -> v6 Migration、重启、模拟中断和旧数据恢复。
- DNS/IP/IPv6/Port 规范化。
- CSPRNG ID、重复 Endpoint、最多 16 Key。
- Key Bytes/Algorithm/Fingerprint 一致性和损坏拒绝。
- First-writer 相同 Key 幂等、不同 Key 冲突。
- Locked Actor 拒绝。
- Quick/Saved/两 Jump Connection Plan Policy。
- Debug/IPC 不包含 Public Key Bytes。

### SSH/OpenSSH

- 第一次 Prompt 接受并持久化。
- 第二次连接和进程重启后无 Prompt。
- Host Key 轮换产生 Changed-Key，且数据库保持旧 Trust。
- Forget 后重新 Prompt。
- Jump 1、Jump 2、Target 三个 Endpoint 分别学习并恢复。
- DB Persist 失败时连接不认证。

### UI/Native

- Playwright 和 agent-browser 覆盖 Known Hosts Desktop/Mobile、Forget Warning、
  Prompt 和 Changed-Key。
- X11/Windows 覆盖首次 Trust、第二次免提示、原生 Forget Confirmation、重启
  恢复、轮换阻断和 Browser Error Log。
- Wayland/IBus、4 MiB 背压、System Agent 和 Encrypted Key 回归。
- Android/Linux Container 和 Windows Build 同 Commit 通过。
- 人工检查截图、报告和 SQLCipher 明文扫描。

## 相关文档

- [ADR-0015](../adr/0015-known-host-trust-is-endpoint-scoped.md)
- [OpenSSH `known_hosts` 2026 基线](../reference/openssh-known-hosts-baseline-2026.md)
- [Threat Model v1](threat-model-v1.md)
- [Saved Host Connection Plan v1](saved-host-connection-plan-v1.md)
- [Host 与 Jump Route Repository v1](host-jump-route-repository-v1.md)
- [ExecPlan 0005](../execplans/active/0005-known-host-repository-and-durable-tofu.md)
