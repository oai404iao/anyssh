# Material 3 Production UI Shell v1

> 实现核验日期：2026-08-03

## 1. 目标

把已通过评审的 Linux/Android Material Design 3 方向迁移到生产客户端，并将当前
超大 React/CSS 文件拆分为可持续维护的 App Shell、Feature Workspace、共享 UI
和 Design Token。

本设计不改变 SSH、Vault、SQLCipher、Session、Port Forwarding、Appearance、
Snippet 或 Typed Tauri IPC 的安全边界。

## 2. 信息架构

### Linux

```text
Custom Window Titlebar
  -> Navigation Rail
    -> Hosts
    -> Sessions / Terminal
    -> Credentials
    -> Snippets
    -> Settings
  -> Workspace Header
  -> Feature Workspace
  -> Contextual Dialog / Bottom Sheet
```

Group、Jump Route、Known Host 和 Appearance 继续存在，但从技术对象平铺导航
逐步迁移到 Hosts、Credentials 和 Settings 的上下文入口，避免主导航暴露所有
Repository 名词。

### Android

```text
System Status Area
  -> Top App Bar
  -> Current Feature
  -> Bottom Navigation
  -> Full-screen Terminal + SSH Auxiliary Keyboard
```

Android 不渲染 Desktop Window Titlebar。终端页优先保留最大可用高度，并保留
Esc、Ctrl、Alt、Tab、方向键等辅助键盘。

生产实现采用：

- 非 Terminal Workspace：Hosts、Sessions、Credentials、Snippets、More 五项
  Bottom Navigation；Groups、Jump Routes、Known Hosts、Appearance 和 Vault
  Lock 收入 More Sheet。
- Terminal Workspace：隐藏普通 Workspace Header 和应用 Bottom Navigation，
  改用 Session Context Action Bar、全高 xterm Surface 和辅助键盘。
- Android User Agent 无论横竖屏宽度都使用 Product Shell；普通 Browser 以
  `max-width: 780px` 提供等价 Compact QA。

## 3. 前端目录和依赖方向

目标目录：

```text
apps/client/src/
|-- app/
|   |-- App.tsx
|   |-- AppFrame.tsx
|   |-- useRepositoryWorkspace.ts
|   `-- shell/
|       |-- WindowTitlebar.tsx
|       |-- AppSidebar.tsx
|       |-- WorkspaceHeader.tsx
|       `-- MobileNavigation.tsx
|-- features/
|   |-- vault/
|   |-- hosts/
|   |-- sessions/
|   |   |-- SessionWorkspace.tsx
|   |   |-- useSessionRuntime.ts
|   |   |-- TerminalMobileControls.tsx
|   |   `-- terminal-input.ts
|   |-- credentials/
|   |-- snippets/
|   `-- appearance/
|-- shared/
|   |-- icons/
|   |-- ui/
|   `-- types/
|-- lib/                         # Typed Bridge Clients
`-- styles/
    |-- tokens.css
    |-- base.css
    |-- window.css
    |-- shell.css
    |-- terminal.css
    |-- dialogs.css
    |-- management.css
    |-- appearance.css
    `-- responsive.css
```

规则：

- Feature 可以依赖 `shared/` 和 `lib/`。
- `shared/` 不依赖 Feature。
- Shell 不持有 Credential Secret、Keyboard-interactive Response 或 Snippet
  Variable Value。
- Tauri Window API 只允许出现在 `app/shell/WindowTitlebar.tsx`。
- Terminal Instance 仍由 Session Feature 按 Tab 持有，不移动到全局 Store。
- `App.tsx` 只组合 Vault、Repository、Session 和 Workspace；Repository
  Refresh/Appearance 进入 App Hook，SSH Runtime/Event/Forwarding 进入 Session
  Hook。

## 4. Design Token

Token 使用 `--md-sys-*` 命名，并保留迁移期 Alias：

```text
Color:
  primary / on-primary
  primary-container / on-primary-container
  surface / surface-container-low / surface-container
  surface-container-high / outline / outline-variant
  error / warning / success

Shape:
  extra-small 4
  small 8
  medium 12
  large 16
  extra-large 28
  full 999

Type:
  display-small
  headline-small
  title-large / medium / small
  body-large / medium / small
  label-large / medium / small

Motion:
  short 120 ms
  medium 180 ms
  emphasized easing
```

Light/Dark 只切换 Token 值。现有 Appearance Setting 继续决定
`data-app-theme`，不得把任意 CSS 或 JavaScript 保存进 Theme。

## 5. Linux Window Chrome

Linux 使用平台配置关闭原生 Decorations，应用内容从窗口顶端开始绘制。

标题栏要求：

- 高度 44 px，颜色使用 Surface Token。
- 左侧显示 Product Mark 与当前 Workspace。
- 中间区域使用 `data-tauri-drag-region`。
- 右侧提供最小化、最大化/恢复和关闭。
- 每个控制按钮至少 40 x 40 CSS Pixel，并有 Accessible Name。
- 按钮不依赖颜色表达功能；关闭 Hover 使用 Error Container。
- Browser QA 中按钮保持可见但不触发原生操作。
- Android/Compact 视口隐藏 Desktop Window Chrome。

只为 `main` Window 授予：

```text
core:window:allow-start-dragging
core:window:allow-minimize
core:window:allow-toggle-maximize
core:window:allow-close
```

Linux Platform Config 必须与 Canonical Window Size/Min Size 同步；Windows QA
Config 继续只用于 Debug WebView2 CDP，不继承 Linux Decorations 设置。

## 6. 渐进迁移

迁移按以下顺序：

1. Design Token、Base Style、Window Chrome 和 App Shell。
2. Vault Welcome/Create/Unlock。
3. Hosts List/Detail/Edit 和 Host Key/OTP Dialog。
4. Terminal Workspace、Session Navigation 和 Android Auxiliary Keyboard。
5. Credentials、Snippet、Appearance、Known Host 和高级管理页。
6. 删除旧 Compatibility CSS 和不再使用的巨型组件。

每个阶段都保留现有 Accessible Name，除非同时更新 Playwright、agent-browser 和
Native Driver。不能为视觉重构破坏 Secret 清理、Mounted Terminal 或 Output Ack。

## 7. 响应式与可访问性

- Desktop Navigation Rail 默认 252 px；小桌面可收缩为 Icon Rail。
- 触控目标最小 44 px；Android 主要操作优先 48 px。
- Dialog 在 Compact 视口改为可滚动 Bottom Sheet。
- 使用语义 `nav`、`header`、`main`、`dialog`、`tablist` 和 `tabpanel`。
- Focus Ring 使用 Primary Token；支持 `prefers-reduced-motion`。
- 文字不低于 12 px；正文默认 14–16 px，不使用评审前的 9–10 px 主信息。
- Compact Shell 使用 `100dvh` 和 `safe-area-inset-bottom`，软键盘出现后 Bottom
  Action 不依赖固定 `100vh`。
- Inactive Terminal 和离开 Terminal Workspace 后的 Terminal 继续 Mounted；
  Connection Panel 不跨 Workspace 保持 Mounted，避免隐藏的临时 Password Form
  与配置 Editor 同时进入 Label/Accessibility 查询。

## 8. Android Terminal 输入规则

- Esc、Tab 和方向键发送固定 VT Sequence。
- Ctrl/Alt 是只作用于下一次输入的 Latch；Ctrl 把 ASCII 字母与 `@`、`[` 等映射
  为 Control Character，Alt 使用 Escape Prefix。
- 修改键按 `Tab ID + Connection Generation` 绑定；Tab 切换、Close、新建、
  Workspace 导航、Disconnect 或重连后不得恢复旧 Latch。
- 普通中文 IME Composition 不经过 Control Mapping；没有 Latch 时继续走 xterm
  原生 Composition 路径。
- “Keyboard” Action 只聚焦 xterm Helper Textarea，让 Android 系统软键盘接管，
  不引入 React 隐藏 Input、原生文本代理或新的 IPC。
- 所有辅助输入仍调用当前 Session 的 `sendSshInput`；不得进入 Log、Global
  State、Vault 或其他 Tab。

## 9. 验证

- TypeScript、ESLint、Vitest、Production Build 和 Playwright。
- agent-browser Desktop 1440x900、Compact 820px、Android 390x844 与 Light/Dark。
- 人工检查关键截图，不只检查退出码。
- Linux X11/Wayland 真实 Tauri Window Chrome。
- Window Drag、Minimize、Toggle Maximize、Close 和 Keyboard Focus。
- Existing SSH、Multi Tab、Port Forward、Appearance、Snippet 与 Vault Lock
  回归。
- Android ARM64 Build；真机 Runtime 验证在本计划后半段执行。
