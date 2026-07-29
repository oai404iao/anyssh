# ExecPlan 0009：Private Key Generation and Encrypted Export

- 状态：Active
- 创建日期：2026-07-29
- 最后更新：2026-07-29
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

让用户不依赖外部 `ssh-keygen` 就能创建现代 SSH Key、查看可部署的 Public Key，
并在再次验证后把 Private Key 安全导出为新 Passphrase 加密的 OpenSSH 文件，
同时保持 Private Key、PIN、Passphrase 和 Path 不进入 WebView。

## 范围

### 包含

- Proposed ADR-0019 与 Private Key Generation/Export v1 Design。
- Ed25519 默认和 RSA 4096 兼容生成。
- Existing Imported/Generated Key 的 Public Key/Fingerprint Projection。
- Native PIN Step-up。
- Native Export Passphrase Confirmation。
- Rust-owned Save Picker 与 encrypted OpenSSH create-new 写入。
- Metadata-only Tauri/React/Browser QA。
- OpenSSH、X11、Wayland、Windows、Container 和同 Commit CI。

### 不包含

- WebView Private Key Reveal 或 Clipboard Copy。
- 未加密 Export、覆盖已有文件或批量 Export。
- Password Reveal。
- PKCS#8/PEM/PPK、SSH Certificate、FIDO2/PKCS#11 或应用内 Agent。
- Android Document Provider、iOS Share Sheet 或 Biometric Step-up。
- Theme/Font/Snippet；在本计划后单独实施。

## 上下文

当前状态：

- Schema v7 `private_key` Credential 已保存 OpenSSH Key 与可选 Passphrase。
- Native Import、Linux GTK/Windows Credential UI Passphrase 和真实
  OpenSSH Authentication 已完成。
- `ApplicationCore` 可通过 DB Actor 创建/解析 Private Key Credential。
- WebView 当前只能创建 Import Metadata，不能请求 Public Projection 或 Export。
- Vault 只有 Lock/Unlock；尚无已解锁状态下的原生 PIN Step-up Command。
- `ssh-key 0.7.0-rc.11` 已由 Workspace 固定，并支持 Ed25519、RSA、OpenSSH
  Public/Private Serialization 与 AES-256-CTR + bcrypt-pbkdf Encryption。

关键路径：

- `crates/anyssh-app/src/lib.rs`
- `crates/anyssh-storage/src/actor.rs`
- `crates/anyssh-storage/src/credential.rs`
- `apps/client/src-tauri/src/lib.rs`
- `apps/client/src-tauri/src/native_passphrase.rs`
- `apps/client/src/lib/credential-bridge.ts`
- `apps/client/src/components/ConfigurationWorkspace.tsx`
- `apps/client/e2e/connect-preview.spec.ts`
- `apps/client/e2e/windows-native-smoke.mjs`
- `scripts/qa/native-xvfb-smoke.sh`
- `scripts/qa/native-wayland-ime-smoke.sh`
- `scripts/qa/native-windows-smoke.ps1`

## Progress

- [x] 2026-07-29：完成 ExecPlan 0008；Head
  `6fcb1a68d5d791d164f3ed43209aa3a9613b5acf` 的 GitHub Actions Run
  `30416305300` 九个 Job 全部通过并接受 ADR-0018。
- [x] 2026-07-29：创建 Proposed ADR-0019、Design 和本 ExecPlan。
- [x] 2026-07-29：完成 Milestone 1：Rust Key Generation 与 Public
  Projection。Ed25519/RSA 4096、Imported Encrypted Key Projection、Lock/Kind
  Failure、Debug Redaction 和 Comment Bound 均有 Unit Test。
- [x] 2026-07-29：完成 Milestone 2：Native Step-up 与 Encrypted Export。
  Linux GTK、Windows Credential UI、create-new、Unix `0600`、Windows
  protected owner-only DACL、Reparse Point/ADS 拒绝和 Partial Cleanup 已实现。
- [x] 2026-07-29：完成 Milestone 3：Tauri/React Product UI。Typed IPC、
  Browser Metadata Preview、Generate/Public/Export UI、Vitest、Playwright 和
  agent-browser 已通过。
- [ ] 完成 Milestone 4：OpenSSH 与 Native QA。
- [ ] 完成 Milestone 5：全量回归与治理。
- [x] 2026-07-29：新增
  `generated_private_key_smoke.rs`。Generated Ed25519 通过 Direct、
  Generated RSA 4096 通过 Saved Host、Encrypted Export/Reimport 通过
  Password Jump -> Private Key Target 完成真实 OpenSSH Authentication。
- [x] 2026-07-29：`pnpm test:ssh:smoke` 通过；Canonical Fixture 现在覆盖
  imported/generated/exported Private Key。
- [x] 2026-07-29：`pnpm qa:native:xvfb` 通过，Evidence 为
  `artifacts/native-xvfb/smoke-1785303021-2441062`。已人工检查 Ed25519/RSA
  Public Projection、Native Save Picker、错误/正确 PIN、Passphrase
  Mismatch/Retry、Export Result 和新 Passphrase 原生 Reimport。
- [x] 2026-07-29：`pnpm qa:native:wayland` 回归通过，Evidence 为
  `artifacts/native-wayland/smoke-1785303237-2447216`。
- [x] 2026-07-29：`pnpm qa:browser` 通过，Evidence 为
  `artifacts/agent-browser/smoke-1785302978`；Desktop/Mobile Public Key
  Dialog、Metadata-only Generation 和 no-file Export 已人工检查，Error Log
  为空。
- [ ] 在真实 Windows Runner 编译并执行 Credential UI、Save Dialog、
  owner-only ACL、Junction/ADS Guard、Export/Reimport 和 OpenSSH Marker。
- [x] 2026-07-29：Linux Container Build 通过，
  `artifacts/linux-build/build-1785303364-1/anyssh-client` SHA-256 为
  `d2f1f4cc59ced897f89ba79989fa643a24fac485ab8fae1bd04b8dc7233da612`。
- [x] 2026-07-29：Android ARM64 Container Build 通过，
  `artifacts/android-build/build-1785303420-1/AnySSH-arm64-debug.apk`
  SHA-256 为
  `847abd61804aaf827a100d15d9de72333b59725e61206a0cb82359d009f6f3cd`。
- [x] 2026-07-29：实现 Commit
  `2d5312df073937bbf0b0f48b25fa565182a7e5a9` 的 GitHub Actions Run
  `30425670349` 完成；Frontend、Browser E2E、OpenSSH、agent-browser、
  Rust、Linux Container 和 Android Build 通过，Windows/Linux Native QA
  暴露自动化兼容问题并已修复，等待新 Commit 同 Commit CI。
- [x] 2026-07-29：修复 Commit
  `27af156e233c772f8d000359fcef3b60a17b0ded` 的 Run `30426639654`
  证明 Windows Generation、PIN/Passphrase、ACL、Junction/ADS、
  Export/Reimport、OpenSSH Marker、重启与 Changed-Key 全部执行成功；Job
  仅因短 PIN 在 PNG 压缩字节中的假阳性 Secret Scan 失败。Linux X11 已进入
  Step-up，但共享 Runner 的 Argon2 Retry 超过固定等待。两项 QA 判定已修复。
- [ ] 运行同 Commit GitHub Actions，检查全部 Artifact 和 Windows/Linux/
  Android Build Hash。

## Milestones

### Milestone 1：Rust Generation 与 Public Projection

1. 定义 Algorithm/Public Summary/Error。
2. Ed25519/RSA 4096 `spawn_blocking` CSPRNG Generation。
3. 生成 Key 写入现有 Private Key Credential。
4. Imported/Generated Key parse/decrypt -> Public Key/Fingerprint。
5. Unit Test、Debug Redaction 和 Vault Lock/Kind Error。

出口：

- 不调用系统 `ssh-keygen`。
- Public Projection 不含 Private Key 或 Stored Passphrase。

### Milestone 2：Native Step-up 与 Encrypted Export

1. DB Actor Verify PIN，绑定当前已解锁 Vault。
2. Linux/Windows Native PIN Prompt。
3. Linux/Windows New Passphrase + Confirmation。
4. Rust-owned Save Picker、create-new、权限、fsync 和 Partial Cleanup。
5. Existing/Generated Key Re-encryption 与错误/取消测试。

出口：

- Export Request 只含 Credential ID。
- 输出始终为新 Passphrase 加密的 OpenSSH Key。

### Milestone 3：Tauri/React Product UI

1. Typed Generation/Public/Export IPC，`deny_unknown_fields`。
2. Browser Metadata-only Simulation。
3. Generate Dialog、Public Key Dialog 和 `Export encrypted…` Action。
4. Desktop/Mobile/Compact 与 Accessibility。
5. Vitest、Playwright 和 agent-browser。

出口：

- React State 不出现 PIN、Passphrase、Path 或 Private Key。
- Public Key 可以选择/复制。

### Milestone 4：OpenSSH 与 Native QA

1. Generated Ed25519/RSA Direct/Saved/Jump Authentication。
2. Exported Key 新 Passphrase 解密与真实 SSH。
3. X11 Native Prompt/Picker/Source Delete/Marker。
4. Windows Credential UI/Save Dialog/EXE/OpenSSH。
5. Wayland/IBus、Vault/Log/Evidence Secret Scan。

出口：

- Generated/Exported Key 在真实协议和平台 Runtime 中可用。
- Secret 不进入 Vault 明文、日志或 Evidence。

### Milestone 5：全量回归与治理

1. Workspace、Frontend、OpenSSH、Browser、Native、Container。
2. 同 Commit CI 九个 Job。
3. 人工检查 Screenshot、Error Log、Build Hash 和 Secret Scan。
4. 更新 Threat Model、Status、Roadmap、README 和 AGENTS。
5. 接受、拒绝或替代 ADR-0019。
6. 移动本计划到 `completed/`。

出口：

- Key Generation/Public Reveal/Encrypted Export 的 Runtime、UI 和治理一致。

## Validation

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm test:ssh:smoke
pnpm test:e2e
pnpm qa:browser
pnpm qa:native:xvfb
pnpm qa:native:wayland
pnpm qa:native:windows
pnpm check:container:linux
pnpm check:container:android
pnpm docs:check
pnpm format:check
git diff --check
```

验收重点：

- Ed25519/RSA 4096 CSPRNG 与 Blocking Boundary。
- Public Projection 只含 Public Material。
- Native PIN Step-up/Passphrase/Path。
- Encrypted-only Export 与 create-new。
- Imported/Generated、Direct/Saved/Jump。
- Cancel/Failure/Lock/Existing File Cleanup。
- Browser 与 Native Secret Scan。

## Surprises & Discoveries

- 2026-07-29：当前固定 `ssh-key 0.7.0-rc.11` 已提供
  `PrivateKey::random`、Public Key/Fingerprint、OpenSSH Serialization 和
  AES-256-CTR + bcrypt-pbkdf Encryption；不需要调用外部 `ssh-keygen`。
- 2026-07-29：现有 Private Key Credential 已足够保存 Generated Key，不需要
  Schema v8。主要新边界是已解锁 Vault 的 PIN Step-up 和 Native Export File。
- 2026-07-29：RSA 4096 在共享 Runner 上可能超过数秒；X11 QA 必须等待
  Generation Modal 真正关闭，固定的短等待会把后续点击错误路由到 Modal。
- 2026-07-29：Generated Key 的协议验收可以复用现有 OpenSSH Topology：
  Test 仅把 Public Key 追加到 Fixture `authorized_keys`，Private Key 始终留在
  ApplicationCore/Vault。
- 2026-07-29：Windows `std::fs::OpenOptions` 不能表达 protected current-owner
  DACL。实现使用一个受限的 `cfg(windows)` Win32 Unsafe Boundary：
  `CreateFileW(CREATE_NEW | FILE_FLAG_OPEN_REPARSE_POINT)` 加
  `O:<current-user-sid>D:P(A;;FA;;;<current-user-sid>)`；独立 Windows-target
  Compile Probe 已通过，真实 Runtime 仍需 Windows CI。
- 2026-07-29：Wayland Forward QA 暴露 Number Input Focus 后首字符偶发丢失；
  在 `ctrl-a` 与输入之间增加短等待后回归通过。
- 2026-07-29：`spawn_blocking` 的 Join Future 被取消时 Blocking Task 不会自动
  停止；Operation Permit 必须移入 Blocking Closure，并在 Generation 完成后
  随 Key 一起返回，才能避免取消窗口提前释放并发槽。
- 2026-07-29：GitHub Ubuntu 的 GTK Save Picker 保持 Filename Entry Focus，
  `Ctrl+L` 没有切换到 Location Entry；QA 改为在已知 `src-tauri` Current
  Directory 中输入随机 Filename，再按确定的绝对路径验证和清理。
- 2026-07-29：Windows PowerShell 5.1 在
  `$ErrorActionPreference = "Stop"` 下会把 `cargo test 2>&1 | Tee-Object`
  的正常编译 stderr 变成终止性 `NativeCommandError`；QA 改为
  `Start-Process` 分离重定向 stdout/stderr，并只按 Process Exit Code 判定。
- 2026-07-29：六位 PIN 直接扫描 PNG Binary 会命中压缩数据中的偶然
  `000000` Byte Sequence。长 Secret/Path 继续扫描全部 Evidence；短 PIN 改为
  只扫描 `.txt/.log/.json/.md`，Native Prompt Screenshot 由人工确认 Masked。
- 2026-07-29：Windows Runner 的 Vault PIN Argon2 Step-up 已证明可能超过两秒；
  X11 错误/正确 PIN 和 Passphrase Retry 改为有界 Poll，而不是固定 Sleep 后
  单次探测。

## Decision Log

- 2026-07-29：v1 默认生成 Ed25519，并提供 RSA 4096 兼容选项。
- 2026-07-29：Public Key/Fingerprint 可以进入 WebView；Private Key、
  Stored/Export Passphrase、PIN 和 Path 保持 Rust/native-only。
- 2026-07-29：v1 只提供 encrypted OpenSSH Export，不提供 plaintext Export 或
  覆盖已有文件。
- 2026-07-29：路线图中的 Private Key “Reveal”在 v1 收窄为 Public Key Reveal；
  不在 React Modal 或 Web Clipboard 暴露 Private Key。
- 2026-07-29：Canonical OpenSSH 覆盖采用三条互补路径：Generated Ed25519
  Direct、Generated RSA 4096 Saved Host、Exported/Reimported Ed25519 Jump。
- 2026-07-29：Generation/Public Projection/Export 共用一个
  `ApplicationCore` Operation Slot；并发请求 Fail Closed，避免被攻陷 WebView
  向 Blocking Pool 无界提交 RSA 4096 工作。Permit 随 Blocking Task 生命周期
  持有，Task 完成或失败后才释放。
- 2026-07-29：Windows Export File 使用当前 Owner 的 protected DACL，拒绝
  Reparse Point Ancestor 与 Alternate Data Stream。为保持 Win32 Handle/
  Security Descriptor 生命周期可审计，`anyssh-app` 从全 crate
  `forbid(unsafe_code)` 收窄为 `deny(unsafe_code)`，只允许该 Windows-only
  Module 的带 Safety Comment Unsafe。

## Outcomes & Retrospective

Rust、Browser、OpenSSH、X11 和 Wayland 已完成并有本地 Evidence。Windows
真实 Runner、同 Commit CI、CI Artifact Hash/Secret 复核以及 ADR-0019 最终
状态尚未完成，因此计划继续保持 Active。
