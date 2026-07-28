# 2026 技术版本基线

> 最后核验：2026-07-28
> 用途：Phase 0 初始化依赖时的参考起点，不是永久版本锁定。

| 领域 | 基线 |
| --- | --- |
| Tauri | 2.11.x |
| Tauri Dialog Plugin | 2.7.x |
| Android JDK | OpenJDK 17 |
| Android SDK / Target SDK | 36 |
| Android Build Tools | 35.0.0 |
| Android NDK | 29.0.13846066（Tauri CLI 2.11.4 基线） |
| Build Container Rust | `rust:1.93.1-bookworm` 固定 Manifest Digest |
| Build Container Node | `node:24.13.0-bookworm-slim` 固定 Manifest Digest |
| React | 19.x |
| Vite | 8.x |
| xterm.js | 6.0.x |
| Rust SSH | russh 0.62.x |
| OpenSSH PAM Fixture | Alpine 3.22 `openssh-server-pam 10.0_p1-r10` / `sshd.pam` |
| SQLCipher | 4.10.0 community（当前 bundled 验证值） |
| rusqlite | 0.39.x |
| Tokio | 1.x |
| PIN KDF | Argon2id / RFC 9106 |
| Record AEAD | XChaCha20-Poly1305 |
| Key Derivation | HKDF-SHA-256 |
| WebDAV | RFC 4918；可选 RFC 6578 Sync Collection |

## 版本策略

- 初始化时再次检查每个依赖的最新稳定 patch。
- 密码学、安全存储、SSH 和 Tauri 依赖必须进入 lockfile。
- 不为了追求“最新”采用 RC 或预发布密码学依赖。
- 自动更新工具可以创建 PR，但不能自动合并安全关键依赖。
- 版本升级若改变数据格式、安全属性或平台支持范围，必须创建 ADR 或迁移 ExecPlan。
- Rust 1.93 基线暂时固定 `rusqlite 0.39.x` / `libsqlite3-sys 0.37.x`；
  0.38.x 的构建脚本使用了该工具链尚未稳定的 `cfg_select`。

## 主要核验来源

- Tauri 官方文档与 Releases。
- xterm.js 官方 Release Notes。
- russh crates.io 元数据与源码。
- OpenSSH Release Notes。
- SQLCipher 官方 Release Notes。
- Apple、Android 和 Microsoft 平台安全文档。
- IETF WebDAV 与 Argon2 RFC。
