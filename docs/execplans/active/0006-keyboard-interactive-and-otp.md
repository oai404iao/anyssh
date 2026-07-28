# ExecPlan 0006：Keyboard-interactive and OTP

- 状态：Active
- 创建日期：2026-07-28
- 最后更新：2026-07-28
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

让 AnySSH 能连接要求 RFC 4256 Keyboard-interactive、OTP 或
Public Key/Password/System Agent + 第二因子的 SSH Server，同时保持响应只属于
当前 Session，不被保存、自动匹配或错误应用到其他 Jump Hop。

## 范围

### 包含

- Proposed ADR-0016 与 Keyboard-interactive Authentication v1 Design。
- SQLCipher Schema v7 `keyboard_interactive` Credential。
- russh 多轮 Challenge/Response 和 Partial-success 第二因子。
- Request/Hop/Round 绑定、Count/Size/Timeout 上限和取消。
- Quick、Saved、Jump 和 Target。
- Tauri Typed IPC 与 Desktop/Mobile React Prompt。
- Linux OpenSSH PAM Fixture 和受控多轮 Test Server。
- Browser、X11、Wayland、Windows、Container 与同 Commit CI。

### 不包含

- 保存 OTP/TOTP/HOTP Seed 或自动生成验证码。
- Prompt 文本到 Saved Password 的自动匹配。
- Duo Push/WebAuthn/FIDO2 专用流程。
- SSH Password Change。
- OpenSSH Keyboard-interactive Device/Submethod 配置。
- Forwarding、多 Tab、WebDAV 或 Secret Reveal。

## 上下文

当前状态：

- `SessionAuthentication` 只有 Password、Private Key、System Agent。
- 三种认证只检查 `AuthResult::success()`，丢弃 `partial_success` 和
  `remaining_methods`。
- `SessionEvent` 只有 Host Key/Changed-Key 和 Terminal 生命周期事件。
- `SessionControl` 只有 Host Key Decision、Input、Resize 和 Disconnect。
- Schema v6 Credential 必须有 Secret Nonce/Ciphertext，无法表达“只有
  Username、运行时等待 Server Prompt”的 Interactive Credential。
- Quick 临时 Password 已允许短暂进入局部 WebView 表单，但不得进入全局状态、
  日志或持久化。

已核验的固定依赖能力：

- russh `0.62.4` 提供
  `authenticate_keyboard_interactive_start/respond`、多轮 `InfoRequest`、
  `Prompt { prompt, echo }`、`partial_success` 和 `remaining_methods`。
- Alpine 3.22 提供 `openssh-server-pam 10.0_p1-r10`；本地探针已证明
  `AuthenticationMethods keyboard-interactive:pam` 能通过
  `pam_exec.so expose_authtok` 接受固定 Token。

关键路径：

- `crates/anyssh-ssh/src/lib.rs`
- `crates/anyssh-storage/src/credential.rs`
- `crates/anyssh-storage/src/lib.rs`
- `crates/anyssh-storage/src/actor.rs`
- `crates/anyssh-app/src/lib.rs`
- `apps/client/src-tauri/src/lib.rs`
- `apps/client/src/lib/ssh-bridge.ts`
- `apps/client/src/App.tsx`
- `apps/client/src/components/ConfigurationWorkspace.tsx`
- `scripts/test-ssh-smoke.sh`
- `scripts/qa/native-xvfb-smoke.sh`
- `scripts/qa/native-wayland-ime-smoke.sh`
- `scripts/qa/native-windows-smoke.ps1`
- `tests/fixtures/openssh/`

## Progress

- [x] 2026-07-28：完成 ExecPlan 0005，接受 ADR-0015。
- [x] 2026-07-28：核验固定 russh Keyboard-interactive/Partial-success API。
- [x] 2026-07-28：用 Alpine OpenSSH PAM 临时探针证明固定 Token
  Keyboard-interactive Authentication 可行。
- [x] 2026-07-28：创建 Proposed ADR-0016、Design 和本 ExecPlan。
- [x] 2026-07-28：完成 Milestone 1：SSH Core Challenge/Response。
- [x] 2026-07-28：完成 Milestone 2：Schema v7 与 Application Boundary。
- [x] 2026-07-28：完成 Milestone 3：Tauri/React Product UI。
- [x] 2026-07-28：Alpine OpenSSH PAM 完成纯 Interactive、
  Password/Private Key/System Agent Partial-success + OTP、Saved Host 和
  Jump 1/Jump 2/Target Prompt 归属。
- [x] 2026-07-28：Browser/Playwright/agent-browser、Linux X11 和无
  `DISPLAY` Wayland/IBus Native Challenge 通过；关键截图已人工检查。
- [x] 2026-07-28：Windows controlled russh Server、真实 EXE/WebView2
  Challenge、Interactive Credential 重启和 Response Secret Scan 已接入 QA。
- [x] 2026-07-28：本地通过 Workspace Clippy/Test、Client Test、Vitest 16、
  Playwright 5、OpenSSH Smoke、agent-browser、X11、Wayland、Android ARM64、
  Linux/Android Container、Docs、Format 和 `git diff --check`。
- [ ] 完成 Milestone 4：OpenSSH 与 Native QA。
- [ ] 完成 Milestone 5：全量回归、Artifact 检查、ADR 状态评审和收尾。

## Milestones

### Milestone 1：SSH Core Challenge/Response

1. 增加 Interactive Authentication 与 Partial-success Outcome。
2. 增加 Challenge Event、Pending Request 和 Response Control。
3. 实现 Round/Prompt/Text/Response 上限、Timeout、Cancel 和零 Prompt。
4. 受控 russh Server 覆盖多 Prompt、多 Round、Echo 和异常流。
5. 普通第一因子失败不得回退。

出口：

- Direct/Jump 的 Challenge 归属和 Stale Request 有确定测试。
- Debug/Error 不包含 Response。

### Milestone 2：Schema v7 与 Application Boundary

1. Credential Kind 增加 `keyboard_interactive`。
2. v6 -> v7 Migration 重建 Nullable/Kind Constraint 和依赖引用。
3. Repository CRUD/Resolve/Actor/Connection Plan。
4. Quick/Saved Application API 和 Response 编排。
5. Migration 成功、重启、中断回滚、引用保持和明文扫描。

出口：

- Interactive Credential 只有 Label/Username，无 Secret Payload。
- WebView 仍不能提交 Saved Secret 或拼装 Route。

### Milestone 3：Tauri/React Product UI

1. Typed Challenge Event 和 Response Command。
2. Credential Editor 增加 Interactive Kind。
3. Quick Connection Auth Selector。
4. 多 Prompt Desktop/Mobile Modal、Focus、Echo 和清理。
5. Vitest、Playwright 和 agent-browser。

出口：

- Response 只存在局部表单和当前 IPC Request。
- Cancel/Submit/Lock/Disconnect/Unmount 均清空。

### Milestone 4：OpenSSH 与 Native QA

1. 提交 Alpine OpenSSH PAM Fixture。
2. 纯 Interactive 与 Public Key + OTP。
3. X11/Wayland 真实 Challenge。
4. Windows 受控 Test Server + 真实 EXE/WebView2。
5. 保持 Known Host、Encrypted Key、Agent、4 MiB 和 IME 回归。
6. Evidence/日志/Vault 扫描不得出现测试 Token。

出口：

- Linux 真实 OpenSSH 证明 RFC 4256/PAM 兼容。
- Windows/Linux Native UI 证明响应生命周期。

### Milestone 5：全量回归与治理

1. Workspace、Frontend、Browser、OpenSSH、Native、Container。
2. 同 Commit CI 九个 Job。
3. 人工检查截图、Error Log、Build Hash 和 Secret Scan。
4. 更新 Threat Model、Status、Roadmap、README 和 AGENTS。
5. 接受、拒绝或替代 ADR-0016。
6. 移动本计划到 `completed/`。

出口：

- Schema v7、Runtime、UI、Native Evidence 与治理文档一致。

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

- Schema v6 -> v7 和中断恢复。
- Interactive Credential 无持久响应。
- Partial Success 后继续，普通失败不降级。
- 多 Prompt、多 Round、Echo、零 Prompt。
- Request/Hop/Round/Count/Size/Timeout/Cancel。
- Quick/Saved/Jump/Target。
- Response 不进入 Global State、Repository、Error、Log、Screenshot。
- OpenSSH PAM 与 Native Runtime。

## Surprises & Discoveries

- 2026-07-28：russh `0.62.4` 已完整暴露多轮 Keyboard-interactive Client API，
  但当前 AnySSH 的 `AuthResult::success()` 抽象会丢失第二因子所需的
  `partial_success` 和 `remaining_methods`。
- 2026-07-28：OpenSSH PAM 成功认证后会再发送一次零 Prompt
  `SSH_MSG_USERAUTH_INFO_REQUEST`；Core 必须自动回复空列表且把它计入 Round
  上限。
- 2026-07-28：Alpine 默认 `openssh` Fixture 明确关闭 PAM；新增测试必须使用
  `openssh-server-pam`/`sshd.pam`，不能改变现有 Password/Key Fixture 语义。
- 2026-07-28：`pam_exec.so expose_authtok` 在 `pam_setcred` 阶段也可能执行；
  Fixture 必须使用 `type=auth`，再由 `pam_permit.so` 处理 Account/Session，
  否则认证成功后 Session 仍可能被关闭。
- 2026-07-28：russh `0.62.4` Test Server 在 Public Key Handler 返回带
  `partial_success = true` 的 Reject 后，会在发送 Failure Packet 前把该标志
  重置为 `false`。受控 russh Server 仍可覆盖多 Prompt/多 Round；真实
  Partial-success 必须由 OpenSSH
  `AuthenticationMethods publickey,keyboard-interactive:pam` Fixture 证明。
- 2026-07-28：OpenSSH 会清理任意 Server 环境变量，PAM Helper 无法直接读取
  Docker `ANYSSH_OTP_TOKEN`。Fixture 现在在启动 `sshd.pam` 前把 Token 写入
  root-only `/run/anyssh-otp-token` 并从 Server 环境移除；PAM Helper 只读该
  临时文件且不记录内容。
- 2026-07-28：仅有 `pam_exec.so type=auth` 时，成功认证后的
  `pam_setcred()` 会因没有成功模块而失败；在 Auth Stack 增加
  `pam_permit.so` 后 Session 可正常建立。
- 2026-07-28：OpenSSH 10 的 Per-source Penalty 会把高频 `ssh-keyscan`
  Readiness Probe 视为未认证连接并暂时丢弃。Wayland Fixture 改为有界
  `/dev/tcp` Readiness Probe，真正协议兼容仍由后续 russh Session 验证。
- 2026-07-28：Windows 多 Stage QA 原本会让上一个 Stage 设置的 Picker/
  Passphrase Driver 环境进入下一次 AnySSH 进程。`Start-NativeStage` 现在在
  启动每个 EXE 前显式清除 Driver Secret，再在进程启动后重新提供给外部 Driver；
  Keyboard-interactive Response 使用同一规则。
- 2026-07-28：首个同提交 CI Run `30357730425` 的 Windows WebView2 截图已显示
  Quick Connection 切换为 Keyboard-interactive 且 Password Field 不可见，
  但非精确 ARIA `getByLabel("Password")` 仍报告一个匹配项。Windows QA 改用
  唯一 DOM ID `#connection-password` 断言敏感 Input 已从 Form 移除；ARIA
  Locator 继续用于用户可见控件交互。
- 2026-07-28：通过的 Run `30358929562` 证明了 Windows Challenge，但 Artifact
  Review 发现 standalone OpenSSH 的 `LogLevel VERBOSE` 和
  `ssh-fixture.txt` 会把 System Agent Public Key Fingerprint 写入 QA Evidence。
  Fixture 改为 `LogLevel ERROR`、不再记录 Fingerprint，并把该次随机 Fingerprint
  加入 Evidence Secret Scan；真实 Agent Authentication 继续由远端 Marker 和
  EXE/WebView2 Screenshot 证明。

## Decision Log

- 2026-07-28：Interactive Credential 只保存 Label/Username，不保存 OTP Seed、
  Response 或 Prompt Rule。
- 2026-07-28：当前 Session Response 可以像 Quick 临时 Password 一样短暂通过
  局部 WebView/IPC，但不得持久化或进入全局状态。
- 2026-07-28：普通第一因子失败不自动回退；只有明确 Partial Success 才继续
  Keyboard-interactive。
- 2026-07-28：Server Prompt 不触发 Saved Password 自动填充。
- 2026-07-28：Linux OpenSSH PAM 证明真实兼容，多轮边界由受控 russh Server
  补齐。
- 2026-07-28：Schema v7 Interactive Credential 的 Secret/Passphrase 四列
  必须全部为 `NULL`；其他 Kind 继续要求 Secret Nonce/Ciphertext。
- 2026-07-28：React Challenge Response 移入按 Request ID 重建的子组件局部
  State；Submit/Cancel 先清空，Disconnect/Lock/Route Change/Unmount 通过卸载
  组件清理。
- 2026-07-28：Windows 使用
  `crates/anyssh-ssh/examples/keyboard_interactive_server.rs` 作为 controlled
  Test Server；它只用于 QA，不进入产品 Runtime，也不调用系统 `ssh`。

## 当前本地证据

- OpenSSH：`pnpm test:ssh:smoke`，覆盖纯 Interactive、错误/正确 Response、
  Password/Private Key/System Agent + OTP、Saved Host 和 Jump 1/2/Target。
- Browser：
  `/home/u/dev/local/any_ssh/artifacts/agent-browser/smoke-1785239733`。
- X11：
  `/home/u/dev/local/any_ssh/artifacts/native-xvfb/smoke-1785240099-954190`。
- Wayland：
  `/home/u/dev/local/any_ssh/artifacts/native-wayland/smoke-1785239155-888591`。
- Android ARM64：
  `/home/u/dev/local/any_ssh/artifacts/android-build/build-1785239962-939026`。
- Linux Container：
  `/home/u/dev/local/any_ssh/artifacts/linux-build/build-1785240001-1`。
- Android Container：
  `/home/u/dev/local/any_ssh/artifacts/android-build/build-1785240065-1`。
- Windows：脚本与 controlled Server 已完成，等待同 Commit Windows Runner
  生成 Artifact。

## Outcomes & Retrospective

尚未完成。
