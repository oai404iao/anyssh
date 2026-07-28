# OpenSSH `known_hosts` 2026 基线

> 核验日期：2026-07-28
> 用途：Known Host Repository、TOFU 持久化和后续 OpenSSH 导入/导出的外部
> 格式基线。

## 协议责任

SSH Transport 会让服务端使用 Host Key 证明身份，但“该 Key 是否属于目标
Endpoint”仍由客户端本地策略决定。AnySSH 因此不能把成功完成密钥交换等同于
Host 身份已可信。

## OpenSSH 文件模型

OpenSSH `known_hosts` 的普通记录由以下字段组成：

```text
[marker] host-patterns key-type base64-public-key [comment]
```

关键语义：

- 一行绑定一个 Public Key，可有一个或多个逗号分隔的 Host Pattern。
- 非默认端口使用 `[host]:port`。
- Pattern 可以包含 `*`、`?` 和 `!` 否定。
- Hash 记录使用 `|1|salt|hash` 形式。它只能通过候选 Endpoint 计算匹配，不能
  还原出可展示的 Host。
- `@revoked` 表示该 Key 必须被拒绝。
- `@cert-authority` 表示 Host Certificate CA，不等于普通 TOFU Key。
- 同一 Endpoint 可以有多行、多个 Host Key Algorithm。
- Comment 不参与 Host 身份校验。

OpenSSH 提供：

- `ssh-keygen -F`：查找 Host。
- `ssh-keygen -H`：Hash 已有 Host 名。
- `ssh-keygen -R`：移除 Host。
- `HashKnownHosts`：控制新记录是否 Hash Host 名。

这些 CLI/用户文件语义只作为兼容基线。AnySSH 核心功能不得通过启动
`ssh-keygen`、`ssh` 或直接修改用户的 `~/.ssh/known_hosts` 实现。

## 当前 Rust 依赖能力

仓库固定的 `ssh-key 0.7.0-rc.11`：

- 可解析普通、Pattern、Hash、`@revoked` 和 `@cert-authority` Entry。
- `PublicKey::to_bytes()` / `from_bytes()` 可提供规范化二进制表示。
- 可从 Public Key 重新计算 Algorithm 和 SHA-256 Fingerprint。

`russh 0.62.4` 自带的 Known Hosts Helper 面向用户主目录文件，不符合 AnySSH
的 SQLCipher、DB Actor、移动端和未来同步边界，因此不作为产品 Repository。

## AnySSH v1 规划约束

- 内部 Runtime Trust 先使用精确、规范化的逻辑 `host + port`。
- 保存完整 Public Key Bytes，并从它校验 Algorithm/Fingerprint。
- 不通过 DNS 解析结果、Host ID 或 Jump Route ID 定义 Host 身份。
- 第一阶段不实现 Pattern、Hash、Marker、Host Certificate 和系统文件导入。
- 数据模型必须保留后续 OpenSSH 导入/导出所需的 Public Key，而不能只存显示用
  Fingerprint。

## 核验来源

- [RFC 4253：SSH Transport Layer](https://datatracker.ietf.org/doc/html/rfc4253)
- [OpenBSD sshd(8)：SSH_KNOWN_HOSTS_FILE_FORMAT](https://man.openbsd.org/sshd.8#SSH_KNOWN_HOSTS_FILE_FORMAT)
- [OpenBSD ssh_config(5)：HashKnownHosts](https://man.openbsd.org/ssh_config.5#HashKnownHosts)
- [OpenBSD ssh-keygen(1)](https://man.openbsd.org/ssh-keygen.1)
- 本地固定源码：
  - `ssh-key-0.7.0-rc.11/src/known_hosts.rs`
  - `ssh-key-0.7.0-rc.11/src/public.rs`
  - `russh-0.62.4/src/keys/known_hosts.rs`
