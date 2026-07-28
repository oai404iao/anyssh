# AnySSH Saved Host Connection Plan v1

> 状态：已实现
> 日期：2026-07-27

本文定义从 Target Host ID 到任意长度 russh Jump Route 的 Rust-only 解析和执行
路径。产品配置 UI 和 Group 继承已在独立里程碑实现；Known Host Repository
由 [Known Host Repository v1](known-host-repository-v1.md) 单独定义。

## IPC 边界

```text
WebView
  -> { hostId, columns, rows }
    -> Tauri typed command
      -> ApplicationCore
        -> DatabaseActorHandle::resolve_host_connection_plan(host_id)
```

Saved Host Request 不接受 endpoint、Username、Credential ID、Password、
Private Key、Passphrase 或 Route Step。

## Actor 解析

Actor 顺序执行：

1. 加载 Target Host。
2. 沿 `Host -> Jump Route -> ordered Step Hosts` 递归展开 Route。
3. 保持 DFS 后序：先加入 Step Host 自己的上游 Route，再加入 Step Host。
4. 拒绝循环、展开后的重复 Host 和超过 32 跳的 Route。
5. 要求 Target 和每个 Jump Host 都有 Credential ID。
6. 在 Actor-owned Vault 中解析所有 Credential。
7. 返回不实现 Serialize 的 `ResolvedHostConnectionPlan`。

例如：

```text
Target -> Route [Jump 2]
Jump 2 -> Route [Jump 1]

Resolved Plan:
  jump_hosts = [Jump 1, Jump 2]
  target = Target
```

## Application 与 SSH Core

`anyssh-app` 把每个 `ResolvedHostConnection` 转为：

```text
SshConnectionConfig {
  endpoint,
  username,
  authentication,
  host_key_policy: Prompt
}
```

SSH Core：

1. TCP 连接 Jump 1。
2. 认证 Jump 1。
3. 在上一跳 Handle 上为下一跳打开 `direct-tcpip`。
4. 通过 `Channel::into_stream()` 启动下一层 `connect_stream()`。
5. 重复直到 Target。
6. Target 打开 PTY；Session 结束后逆序关闭全部 Handle。

`SessionHop::JumpHost { index }` 使用从 1 开始的 Route 顺序。

## 失败边界

- Host 不存在。
- Host 没有 Credential。
- Credential 不存在或 Vault Locked。
- Route 不存在、循环、重复 Host 或展开后超过 32 跳。
- 任一跳 Host Key 拒绝、认证失败、超时或上游断开。

所有错误不得包含 Credential Secret。

## 验证

- Actor/Application Unit Test。
- Tauri `deny_unknown_fields` IPC Test。
- 两个 Jump Host 的隔离 Docker 网络协议测试。
- Password Jump 1 + Password Jump 2 + Private Key Target。
- Host Key Event 顺序必须是 Jump 1、Jump 2、Target。
