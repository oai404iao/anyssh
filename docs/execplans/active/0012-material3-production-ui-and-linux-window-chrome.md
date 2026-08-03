# ExecPlan 0012：Material 3 生产 UI 重构与 Linux 自定义窗口框

- 状态：Active
- 创建日期：2026-08-02
- 最后更新：2026-08-03
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

把已通过评审的 Linux/Android Material Design 3 方向迁移到 AnySSH 生产客户端，
同时拆分当前巨型 React/CSS 文件，并让 Linux 原生窗口不再出现与应用视觉割裂的
GNOME 大白框。

用户可观察结果：

- Linux 应用从窗口外框到内部页面使用一致的 Material 3 Surface、Shape 和色彩。
- 常用页面拥有清晰的信息层级和更大的可读字号、触控目标。
- Android 继续共享同一产品语言，但使用移动导航和全屏终端。
- UI 重构不影响现有 SSH、Vault、Known Host、OTP、Multi Tab、Forward、
  Appearance 和 Snippet Runtime。

## 范围

### 包含

- `apps/client/src` 的 App Shell、Feature、Shared UI 和 Style 分层。
- Material 3 Color/Type/Shape/Spacing/Motion Token。
- Linux-only 无原生 Decorations 和 React Client-side Window Titlebar。
- Window Drag、Minimize、Toggle Maximize 和 Close 的最小 Tauri Permission。
- Vault Gate、Navigation、Hosts、Terminal、Dialogs、Credential、Snippet、
  Appearance 等生产页面分阶段迁移。
- Desktop、Compact、Android Viewport、Light/Dark 和 Browser/Native QA。
- Android ARM64 Build 与后续真机 Runtime/IME/生命周期纵向验证。

### 不包含

- 修改 Rust SSH、Vault、SQLCipher Schema、Credential Secret 或 Session 协议。
- WebDAV、SFTP、Runbook、Plugin、OpenSSH `known_hosts` Import/Export。
- 删除独立 `apps/design-review`；它在生产迁移完成前作为视觉参考保留。
- 一次性替换所有页面或引入重型 UI Framework。
- 改变 Windows Release Window Decorations。

## 上下文

- 设计评审已由项目负责人于 2026-08-02 确认通过。
- Accepted ADR-0001 继续规定 Tauri 2 + React + xterm.js + Rust Core。
- Proposed ADR-0021 规定结构化 Material 3 Client Shell 和 Linux Client-side
  Window Chrome。
- 计划开始时的关键巨型文件：
  - `apps/client/src/App.tsx`：约 2,400 行。
  - `apps/client/src/App.css`：约 2,400 行。
  - `apps/client/src/components/ConfigurationWorkspace.tsx`：约 2,100 行。
- Existing E2E/Native Driver 依赖大量 Accessible Name；迁移必须优先保持语义。
- Linux Window Chrome 只能改变窗口呈现，不得扩大 WebView 对文件、Shell、
  Credential 或其他原生能力的访问。

## Progress

- [x] 2026-08-02：项目负责人确认设计评审通过。
- [x] 2026-08-02：完成 ExecPlan 0011 并创建本计划。
- [x] 2026-08-02：创建 Proposed ADR-0021 与生产 UI Shell Design。
- [x] 2026-08-02：建立 Material 3 Design Token、八个分层 CSS、App Frame 和
  Shared Icon。
- [x] 2026-08-02：实现 Linux-only Client-side Window Chrome、Platform Config
  与 `main` Window 最小 Permission。
- [x] 2026-08-02：拆分 Session Model、Navigation、Header、Terminal、
  Connection Panel 和认证/Host Key Dialog。
- [x] 2026-08-02：Vault Gate 移入独立 Feature；完成第一轮 Shell 视觉迁移。
- [x] 2026-08-02：Frontend 27 Test、13 Playwright E2E、agent-browser、Linux
  X11、Wayland/IBus 和 Android ARM64 Build 通过。
- [x] 2026-08-02：`ConfigurationWorkspace.tsx` 从 2,106 行拆为
  Configuration Orchestrator、Manager Primitives 和 Host/Group/Credential/
  Route/Known Host Feature；拆分后最大 Feature 为 714 行。
- [x] 2026-08-02：生产 Vault 增加 Welcome -> Create/Unlock，首屏移除
  Argon2id、SQLCipher 和 Record Cipher 等实现名词；新增 4 个 Vault/Host
  Component Test。
- [x] 2026-08-02：Host 页面迁移为搜索、Group Filter、Material 3 Card、
  Connection Plan Detail 和三段式 Editor；保留原有自动化 Accessible Name。
- [x] 2026-08-02：Host Key First Trust、Changed Key Hard Block 和
  Keyboard-interactive Dialog 完成 Material 3 迁移。
- [x] 2026-08-02：Native Host Detail 的 Connect 直接进入 Saved Host ID
  连接路径；Browser QA 只打开 Session，不在 WebView 展开连接计划。Windows
  Native QA 已改为从 Detail 发起连接，等待 Windows Runner 验证。
- [x] 2026-08-02：Frontend 31 Test、13 Playwright E2E、更新后的
  agent-browser、Linux X11、Wayland/IBus、Native Check 和 Android ARM64
  Build 通过。
- [x] 2026-08-03：`App.tsx` 从 1,382 行进一步拆到 696 行；Repository
  生命周期进入 `app/useRepositoryWorkspace.ts`，SSH Event/Connect/Auth/
  Forward Runtime 进入 `features/sessions/useSessionRuntime.ts`，Terminal
  Product Shell 进入 `features/sessions/SessionWorkspace.tsx`。
- [x] 2026-08-03：完成 Android/Compact Product Shell：五项 Bottom
  Navigation、More 管理 Sheet、全高 Terminal、Session/Forwarding/Snippet/
  Keyboard Action、Esc/Ctrl/Alt/Tab/方向键辅助栏和 xterm Soft Keyboard Focus。
- [x] 2026-08-03：Android UA 在横屏宽度超过 780 px 时仍强制使用 Product
  Shell；新增 Landscape Playwright Evidence。
- [x] 2026-08-03：Frontend 39 Test、14 Playwright E2E、agent-browser、
  Linux X11、Wayland/IBus、Native Check 和 Android ARM64 Build 通过。最新本地
  Evidence：
  - Browser：`artifacts/agent-browser/smoke-1785717833/`
  - X11：`artifacts/native-xvfb/smoke-1785717089-1155103/`
  - Wayland：`artifacts/native-wayland/smoke-1785717339-1162049/`
  - Android Build：`artifacts/android-build/build-1785717922-1170485/`
- [ ] 迁移 Host -> Host Key -> OTP -> Terminal 生产纵向流程。
- [x] 拆分 Configuration Workspace。
- [x] 拆分 App Orchestration。
- [ ] 迁移剩余 Snippet/Appearance Compatibility Feature。
- [x] 完成 Browser、Linux X11/Wayland 和 Android Build 回归。
- [ ] 完成 Android 真机 Runtime 和 Windows Native 回归。

## Milestones

### Milestone 1：结构化基础与 Linux Window Chrome

工作：

1. 建立 `app/`、`features/`、`shared/` 和 `styles/` 边界。
2. 把 Color/Type/Shape/Spacing/Motion 提取为 Material 3 Token。
3. 拆分 App Navigation、Workspace Header、Session Model 和 Dialog。
4. 新增 Linux Platform Config，关闭原生 Decorations。
5. 实现 Custom Titlebar 与最小 Window Permission。

出口：

- `App.tsx` 不再同时拥有全部 Shell JSX、Icon、Dialog 和 Domain Type。
- Browser Desktop 截图可看到与内部 Surface 一致的 Window Titlebar。
- Linux Build 使用无原生 Decorations 的主窗口配置。
- Window 控制只授予 `main` Window。

### Milestone 2：Vault、Host 与认证纵向迁移

工作：

1. 迁移 Welcome/Create/Unlock，不在首屏暴露密码学实现名词。
2. 迁移 Host List/Detail/Edit。
3. 迁移 Host Key First Trust、Changed Key Hard Block 和 OTP Dialog。
4. 保持原有 Accessible Name 或同步更新全部测试。

出口：

- Native Vault -> Host -> Host Key -> OTP -> Terminal 可连续完成。
- Secret 清理与 Rust-only 边界保持不变。

### Milestone 3：Terminal 与 Android Product Shell

工作：

1. 迁移 Desktop Terminal Workspace、Session Strip 和 Context Actions。
2. Android 使用 Bottom Navigation、Full-screen Terminal 和辅助键盘。
3. 验证 IME Composition、软键盘、横竖屏、后台/锁屏恢复和长输出。

出口：

- Linux 与 Android 使用统一 Token 但不同平台布局。
- Inactive Terminal 继续 Mounted 并完成 Output Ack。

### Milestone 4：剩余 Feature 与巨型文件清理

工作：

1. 拆分 Groups、Credentials、Routes、Known Hosts、Snippets、Appearance。
2. 删除 Compatibility CSS 和不再使用的旧组件。
3. 更新 Browser、X11、Wayland、Windows 与 Android Evidence。

出口：

- 不再存在承担多个 Feature 的 2,000 行级 UI 文件。
- Proposed ADR-0021 根据 Linux/Android Runtime Evidence 评审为 Accepted、
  Rejected 或 Superseded。

## Validation

基础检查：

```bash
pnpm format
pnpm format:check
pnpm typecheck
pnpm lint:frontend
pnpm test:frontend
pnpm build
pnpm test:e2e
pnpm docs:check
git diff --check
```

浏览器视觉和交互：

```bash
pnpm qa:browser
```

必须人工查看 Desktop、Compact、Android Viewport 和 Light/Dark Screenshot，
确认 Titlebar、导航、Dialog、Terminal 和 Form 不截断、不遮挡。

原生：

```bash
pnpm check:native
pnpm qa:native:xvfb
pnpm qa:native:wayland
pnpm check:android
pnpm qa:native:windows
```

Windows 命令只在 Windows 执行。Linux QA 必须增加：

- 截图中不存在 GNOME/GTK 原生白色标题框。
- Custom Titlebar 可拖动窗口。
- Minimize、Toggle Maximize/Restore 和 Close 可用。
- X11 与无 `DISPLAY` Wayland 均通过。

Android 真机必须验证：

- 软键盘和中文 IME。
- Ctrl/Esc/Alt/Tab/方向键辅助栏。
- 横竖屏和全屏 Terminal。
- 后台、锁屏、Vault Lock 和 Session Cleanup。
- 长输出继续 Ack 且无截断。

## Surprises & Discoveries

- 2026-08-02：Custom Titlebar 增加 44 px 后会改变大量 Native Coordinate QA。
  Linux Custom Chrome 下把 Workspace Header 压缩为 44 px，并同步更新 Header
  Action 坐标，使 Terminal/Configuration 内容起点继续保持原来的 88 px。
- 2026-08-02：agent-browser 可以在 Accessibility Tree 中发现连接面板底部的
  Forward 按钮，但不会自动滚动被裁切按钮；QA 在点击前显式
  `scrollintoview`，并等待每个 Forward 真正出现后再继续。
- 2026-08-02：把 Desktop Sidebar 临时收窄到 232 px 会让大量 X11 Coordinate
  Driver 横向错位；第一阶段保持 252 px，后续只有在改为语义或图像定位后再调整。
- 2026-08-02：Production Build 已把动态 Tauri Window API 分离为独立 Chunk，
  但主 UI Chunk 仍超过 500 KiB；Feature Lazy-loading 留给后续里程碑。
- 2026-08-02：Fresh Vault 增加 Welcome 后，X11/Wayland/Windows Native QA
  必须先进入 PIN Setup；X11/Wayland 增加 `00-welcome.bmp`，Windows CDP
  Driver 增加 Welcome Accessible Assertion。
- 2026-08-02：重新设计 Host Key Card 后 Trust Button 从原生视口约
  `y=532` 移到 `y=595`；X11/Wayland Coordinate Driver 已同步，两个完整
  Native Smoke 均通过。
- 2026-08-02：Mobile Host Editor 的三段表单超过单屏高度；把 Save Action
  固定为滚动容器底部的 Sticky Action，避免用户必须先发现隐藏在末尾的提交按钮。
- 2026-08-03：把 Connection Panel 与 Mounted Terminal 一样长期留在隐藏
  Workspace，会让自动化 Label Locator 同时看到两个 Username/Password Form。
  Session Workspace 现在只长期保留 Terminal；离开 Terminal Workspace 时卸载
  Connection Panel，并继续按现有规则清空 Quick Password。
- 2026-08-03：初版 Mobile Action Bar 的 `z-index` 高于认证 Dialog，导致 OTP
  Bottom Sheet 下方仍露出可点击 Terminal Action。Dialog Backdrop 已提升到
  Product Navigation 和 More Sheet 之上。
- 2026-08-03：单用 `max-width` 无法覆盖 Android 横屏和平板；Product Shell
  判定必须是 `Android User Agent OR compact media query`。
- 2026-08-03：当前 Linux 主机已安装 `adb`，但 `adb devices -l` 没有连接设备，
  `emulator -list-avds` 也没有可用 AVD。Android 真机软键盘、中文 IME、锁屏/
  后台恢复和触控 Terminal Evidence 不能用 Browser UA 或 APK Build 伪造，保持
  为明确阻塞项。

## Decision Log

- 2026-08-02：设计评审通过后继续 Tauri/React 的渐进式生产迁移，不删除已验证
  Rust Core 和 Typed IPC；对应 ADR-0001。
- 2026-08-02：Linux 选择 Client-side Window Chrome，不尝试用 CSS 修改不可控的
  GNOME 原生 Decorations；对应 Proposed ADR-0021。
- 2026-08-02：Windows 保持原生 Decorations，避免扩大本轮平台回归；Android
  不渲染 Desktop Titlebar。
- 2026-08-02：Linux Desktop Chrome 使用受控 User Agent 判定渲染；Android
  User Agent 显式排除。后续若加入平台信息插件，只能替换这一处
  `shared/platform/runtime.ts`。
- 2026-08-02：为保持已验证 Native QA，Custom Chrome 下使用
  `44 px Titlebar + 44 px Workspace Header`，总内容起点仍为 88 px。
- 2026-08-02：Host Card 的可见主操作改为 `Connect`，但使用
  `aria-label="Open"` 暂时保持 Windows/Browser Native Driver 的稳定语义；
  完成自动化语义迁移后再统一 Accessible Name。
- 2026-08-02：Milestone 2 不伪造尚不存在的生物识别或 Desktop Platform
  Slot。Welcome/Create 只展示实际存在的 Local PIN 能力。
- 2026-08-02：Native Host Detail 的 `Connect` 只提交 Saved Host ID 并复用
  现有 Rust-owned `connectSavedHost` 路径。Browser QA 显示 `Open session`，
  不在 WebView 展开 Credential/Route Connection Plan。
- 2026-08-03：Android Terminal 使用全高 Surface，应用 Bottom Navigation 在
  Terminal Workspace 中替换为 Session Context Action Bar；Back 返回 Hosts 后
  再访问 Credentials、Snippets 和 More 管理入口。
- 2026-08-03：Ctrl/Alt 使用只作用于下一次输入的 Tab/Generation-scoped Latch；
  Tab 切换、Close、新建、导航、Disconnect 或 Connection Generation 变化时失效。
  辅助键只调用现有 `sendSshInput`，不新增全局 Store、日志或 IPC。
- 2026-08-03：显示 Android Soft Keyboard 只聚焦现有 xterm Helper Textarea，
  不引入原生文本代理，也不改变 IME Composition 数据路径。

## Outcomes & Retrospective

计划执行中。完成时记录：

- 实际拆分后的目录和最大文件规模。
- Linux X11/Wayland Window Chrome Evidence。
- Android Runtime 证据和剩余风险。
- ADR-0021 最终状态。
