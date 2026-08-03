# ADR-0021：Material 3 结构化客户端壳与 Linux 自定义窗口框

- 状态：Proposed
- 日期：2026-08-02
- 最新实现核验：2026-08-03
- 决策人：项目维护者

## 背景

AnySSH 已完成大量 SSH、Vault、Repository 和 Session Runtime，但当前生产 React
UI 主要集中在超大的 `App.tsx`、`App.css` 和
`ConfigurationWorkspace.tsx`。界面在 Linux 上还叠加桌面环境原生标题栏，与
应用内部视觉不一致；浅色 GNOME 装饰会在深色应用外形成明显的大白框。

独立 Linux/Android Material Design 3 评审网页已于 2026-08-02 通过。现在需要把
评审结论迁移到生产客户端，同时保持 Accepted ADR-0001 的 Tauri 2 + React +
xterm.js + Rust Core 架构和现有安全边界。

## 决策

1. 生产 React UI 按以下依赖方向组织：

   ```text
   app shell
     -> feature workspaces
       -> shared typed UI primitives
         -> typed bridge clients
   ```

   `app/` 只负责组合、导航、窗口框和跨 Feature 生命周期；`features/` 按 Vault、
   Hosts、Sessions、Credentials、Snippets、Appearance 等产品能力拆分；
   `shared/` 只放无业务状态的组件、Icon 和通用类型。

2. Material Design 3 使用有界 CSS Design Token 实现。Theme 继续是
   `system | dark | light` 和固定字段数据，不引入运行时 CSS、远程资源或
   可执行 Theme。

3. Linux 主窗口使用 Client-side Window Chrome：

   - Linux 平台配置关闭原生 Decorations。
   - React 标题栏提供拖动区、最小化、最大化/恢复和关闭按钮。
   - Window API 只授予完成上述操作所需的最小 Tauri Permission。
   - Windows 保持现有原生窗口装饰；Android 不渲染桌面标题栏。
   - Browser QA 渲染相同标题栏外观，但不调用原生 Window API。

4. 迁移期间不重写 SSH、Vault、Repository、Typed IPC 或 xterm Runtime。现有
   Accessible Name 和 Browser/Native 自动化所依赖的稳定语义优先保持。

5. 巨型文件采用渐进式拆分，不进行一次性大爆炸重写。每个阶段必须能独立通过
   TypeScript、Vitest、Playwright 和关键 Native QA。

6. Android Product Shell 按平台身份选择，而不是只依赖 CSS 宽度：

   - Android User Agent 在横屏和平板宽度下仍使用 Bottom Navigation 和全高
     Terminal。
   - 普通 Browser 使用 Compact Media Query 提供等价 QA。
   - Inactive xterm 继续 Mounted；辅助键盘只发送有界 VT/Control Sequence，
     Ctrl/Alt 只绑定下一次 Tab/Generation-scoped 输入。

## 备选方案

- 保留 GNOME/系统原生标题栏：实现简单，但无法消除 Linux 外框与应用设计割裂，
  不满足项目负责人明确要求。
- 全平台都关闭原生 Decorations：视觉最统一，但会无必要地改变已验证的 Windows
  窗口行为，并增加平台回归面。
- 引入重型 Material UI Framework：可快速获得组件，但会增加依赖、样式覆盖和
  长期迁移成本；当前已有受控 Token 与自建组件基础。
- 删除现有 React UI 后整体重写：短期结构整齐，但容易破坏 Session、Secret
  生命周期和已验证的 Native 工作流。

## 后果

### 正面

- Linux 窗口边框与应用内部视觉一致。
- UI 依赖方向、Feature Ownership 和 CSS Token 更清晰。
- Linux 与 Android 可共享产品语言，同时保留平台导航和窗口能力差异。
- Android 横屏不会误回退到 Desktop Sidebar，Terminal 可继续使用系统 IME。
- 可以逐屏迁移而不放弃现有 SSH/Vault 可靠性证据。

### 代价与风险

- Linux 自定义标题栏必须自行维护拖动、窗口控制、最大化和无障碍行为。
- 无原生 Decorations 时，Resize、Window Shadow 和不同 Compositor 的行为需要
  X11/Wayland 实机验证。
- 渐进迁移期间会短暂存在新旧组件和样式并存，需要明确 Compatibility Layer
  和删除里程碑。
- Android 真机的软键盘、IME、生命周期和全屏终端仍需后续专项验证。

## 验证

- Browser QA 检查 Material 3 Shell、Light/Dark、Desktop/Mobile 和原有核心流程。
- Linux X11 与 Wayland 启动真实 Tauri 窗口，确认不存在原生大白标题框，拖动、
  最小化、最大化/恢复和关闭可用。
- 确认窗口控制 Permission 仅限 `main` Window。
- `pnpm test:e2e`、`pnpm qa:browser`、`pnpm qa:native:xvfb` 和
  `pnpm qa:native:wayland` 保持通过。
- Windows Build/Native QA 证明 Linux-only 配置没有改变 Windows 原生 Window。
- Android Build 和后续真机纵向流程验证 Material 3 Responsive Shell。
- Android User Agent 的 Landscape Playwright 检查必须保持 Product Shell；
  agent-browser 必须检查 Bottom Navigation、More Sheet、辅助键盘和 xterm Focus。

若 Client-side Window Chrome 在主流 Wayland/X11 Compositor 上无法提供稳定的
拖动、Resize 或窗口控制，则本 ADR 保持 Proposed，并改为评估与应用配色协调的
GTK HeaderBar 原生集成。

## 相关文档

- Design：
  [Material 3 Production UI Shell v1](../design/material3-production-ui-shell-v1.md)
- ExecPlan：
  [0012](../execplans/active/0012-material3-production-ui-and-linux-window-chrome.md)
- Supersedes：无
- Superseded by：无
