# ADR-0010：Saved Host Connection Plan 只在 Rust 内解析

- 状态：Proposed
- 日期：2026-07-27
- 决策人：项目维护者

## 背景

Schema v3 已持久化 Host、Credential ID 和有序 Jump Route，但当前连接 IPC 仍由
WebView 提交 endpoint、Credential ID 和可选单 Jump Host。若产品 UI 在前端展开
Route，会复制拓扑、增加竞态，并诱使后续代码把 Credential Secret 或解析结果带回
WebView。SSH Core 同时需要从单 Jump Host 扩展到任意长度的有界 Route。

## 决策

- Saved Host 连接 IPC 只接受 Host ID 和 Terminal Size。
- DB Actor 在单条命令中读取目标 Host、递归展开 Jump Route，并解析每一跳的
  Credential。
- Actor 返回的 Connection Plan 是 Rust-only 类型，不实现 Serialize，Debug 不
  展示 Credential 内容。
- Route 展开保持持久化顺序，最多允许 32 个 Jump Host；循环、重复 Host、
  缺失 Credential 和超长 Route 在启动网络连接前失败。
- `anyssh-app` 把 Rust-only Plan 转换为 SSH Core Config。
- SSH Core 使用 `Vec<SshConnectionConfig>`，逐跳建立 `direct-tcpip` 和嵌套
  russh Transport，并在结束时按 Target、最后一跳到第一跳的顺序关闭。
- 现有显式 endpoint Connect IPC 暂时保留给 Phase 0 QA 和未保存连接。

## 备选方案

- WebView 展开 Route：会扩大秘密和拓扑竞态边界，拒绝。
- Tauri 逐个请求 Host/Credential：无法得到一致快照且增加 IPC，拒绝。
- 每一跳启动系统 `ssh`：违反 russh 嵌入式 Engine 决策，拒绝。
- Runtime 继续只支持单 Jump Host：无法执行已持久化的有序 Route，拒绝。

## 后果

### 正面

- WebView 不需要读取 Credential Username 或 Secret 即可连接保存的 Host。
- Route 与 Credential 在 DB Actor 串行边界内得到一致解析。
- 任意长度 Route 复用同一个 Host Key、取消、超时和背压状态机。

### 代价与风险

- SSH Core 必须持有多个上游 Handle，并正确处理任一跳断开。
- 嵌套 Route 展开需要重复 Host 与最大长度约束。
- Known Host 持久化尚未实现，当前每一跳仍使用 Prompt Policy。

## 验证

- Unit Test 验证 Route 顺序、递归展开、重复/缺失 Credential/超长拒绝和 Debug
  脱敏。
- Docker 拓扑验证 `Client -> Jump 1 -> Jump 2 -> Target`，Client 和 Jump 1
  均不能直接访问 Target。
- Vault Lock/Unlock 后只通过 Target Host ID 完成三层 Host Key 确认和 SSH
  Private Key 认证。
- Tauri Saved Host Request 携带 endpoint、Credential 或 Secret 字段时反序列化
  失败。

### 当前证据

- 2026-07-27：Storage/Actor Test 覆盖递归顺序、缺失 Credential、重复 Host、
  超过 32 跳、Vault Locked 和 Debug 脱敏。
- 2026-07-27：Tauri `ssh_connect_saved_host` 只接受 Host ID 与 Terminal Size；
  endpoint、Credential ID 和 Password 字段均被拒绝。
- 2026-07-27：隔离 Docker 拓扑已验证
  `Client -> Jump 1 -> Jump 2 -> Target`，其中 Jump 1 无法直接访问 Target。
  Vault Lock/Unlock 后只使用 Target Host ID，三跳 Host Key 顺序正确，Target
  Private Key 认证成功，Jump 2 错误密码被归属为 `jump host 2`。

## 相关文档

- Design：[Saved Host Connection Plan v1](../design/saved-host-connection-plan-v1.md)
- ADR：[ADR-0002](0002-russh-as-default-ssh-engine.md)
- ADR：[ADR-0009](0009-host-jump-route-reference-model.md)
- ExecPlan：[Phase 0 技术风险验证](../execplans/active/0001-phase-0-technical-validation.md)
- Supersedes：
- Superseded by：
