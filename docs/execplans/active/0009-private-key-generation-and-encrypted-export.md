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
- [ ] 完成 Milestone 1：Rust Key Generation 与 Public Projection。
- [ ] 完成 Milestone 2：Native Step-up 与 Encrypted Export。
- [ ] 完成 Milestone 3：Tauri/React Product UI。
- [ ] 完成 Milestone 4：OpenSSH 与 Native QA。
- [ ] 完成 Milestone 5：全量回归与治理。

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

## Decision Log

- 2026-07-29：v1 默认生成 Ed25519，并提供 RSA 4096 兼容选项。
- 2026-07-29：Public Key/Fingerprint 可以进入 WebView；Private Key、
  Stored/Export Passphrase、PIN 和 Path 保持 Rust/native-only。
- 2026-07-29：v1 只提供 encrypted OpenSSH Export，不提供 plaintext Export 或
  覆盖已有文件。
- 2026-07-29：路线图中的 Private Key “Reveal”在 v1 收窄为 Public Key Reveal；
  不在 React Modal 或 Web Clipboard 暴露 Private Key。

## Outcomes & Retrospective

尚未完成。
