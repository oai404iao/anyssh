# ExecPlan 0011：Linux/Android Material 3 设计评审网页

- 状态：Completed
- 创建日期：2026-08-02
- 最后更新：2026-08-02
- 负责人：项目维护者与执行 Agent

## 目的与用户价值

在继续实现 AnySSH 产品 UI 前，提供一个独立、可点击、无需真实 Vault/SSH
后端的设计评审网页，让项目负责人可以在同一页面中：

- 查看 Linux 与 Android 的核心界面总览。
- 按用户旅程点击体验首次启动、主机连接、终端、凭据与设置流程。
- 切换 Light/Dark 和 Linux/Android 展示。
- 对每个界面标记“待评审 / 通过 / 待修改”并填写本地备注。

本计划的目标不是直接替换生产 UI，而是先形成可评审的 Material Design 3
产品基线，避免继续在没有设计稿和流程定义的情况下堆叠功能。

## 范围

### 包含

- 独立 pnpm Workspace 应用 `apps/design-review`。
- 中文优先、现代专业的 Material Design 3 视觉方向。
- “界面总览 + 可点击流程”两种评审模式。
- Linux、Android 与双端对比视图。
- Light/Dark 展示。
- 核心流程：
  - 欢迎、创建与解锁本地保险库。
  - 主机列表、主机详情、添加主机。
  - Host Key 确认、OTP、建立终端会话。
  - Session 列表与 Android 终端辅助键盘。
  - Credential、Snippet、Appearance 与 Security Settings。
- 每个界面的状态与备注使用浏览器 `localStorage` 本地保存。
- TypeScript、Vitest、构建检查和 agent-browser 人工截图评审。
- 根命令、README、AGENTS 和项目状态的准确入口更新。

### 不包含

- 修改现有 Tauri/React 产品 UI。
- 连接真实 SSH、Vault、数据库或 Tauri IPC。
- 决定继续 Tauri 还是迁移 Flutter/Compose。
- 生成最终品牌 Logo、插画、商店素材或营销网站。
- 把评审备注上传到网络或写入真实 Vault。

## 上下文

项目负责人在 2026-08-02 明确：

- 第一交付优先级是 Linux 与 Android。
- 产品视觉采用 Material Design 3。
- 当前缺少设计稿和可评审的页面流程。
- 评审网页采用“总览 + 可点击”、现代专业、中文优先、页面状态与备注。

当前生产前端仍位于 `apps/client`，其中：

- `src/App.tsx` 约 2,400 行。
- `src/App.css` 约 2,400 行。
- Android 目前只有 ARM64 APK 构建证据，没有产品级真机 UI/生命周期验收。

本计划因此使用完全独立的 Mock 应用，避免评审原型污染安全边界或现有产品
Runtime。设计通过后，再创建单独的生产 UI 重构与 Android 真机验证 ExecPlan。

## Progress

- [x] 2026-08-02：项目负责人确认评审形式、视觉方向、界面语言和备注方式。
- [x] 2026-08-02：创建本 ExecPlan。
- [x] 2026-08-02：建立独立 Design Review Workspace 与根命令。
- [x] 2026-08-02：完成 18 个界面、5 条核心流程与双平台 Mock UI。
- [x] 2026-08-02：完成状态/备注本地持久化和响应式评审壳。
- [x] 2026-08-02：完成类型检查、测试、构建和 agent-browser 人工评审。
- [x] 2026-08-02：更新 README、AGENTS、Product Brief、Roadmap 和 Status。
- [x] 2026-08-02：项目负责人确认首轮设计评审通过，并要求进入生产 UI
  结构化重构。

## Milestones

### Milestone 1：独立评审应用

工作：

1. 新建 `apps/design-review` Vite + React + TypeScript 应用。
2. 增加 `dev:design`、`build:design`、`typecheck:design`、
   `test:design`、`lint:design` 和 `format:design` 根命令。
3. 使用 Mock Data，禁止调用产品 Bridge 或网络。

出口：

- `pnpm dev:design` 可在固定本地端口启动评审网页。
- `pnpm build:design` 生成独立静态产物。

### Milestone 2：界面总览与交互流程

工作：

1. 建立核心 Flow/Screen 清单和设计说明。
2. 实现 Linux/Android Device Canvas。
3. 实现总览、流程、平台与主题切换。
4. 让关键按钮可以沿流程进入下一界面。

出口：

- 项目负责人可以在一个页面中查看全部核心界面。
- 首次启动到终端的主路径可以连续点击体验。

### Milestone 3：评审记录

工作：

1. 每屏支持待评审、通过和待修改状态。
2. 每屏支持本地备注。
3. 显示总体评审进度并支持清空本地记录。

出口：

- 刷新页面后评审记录仍然存在。
- 任何备注都不离开浏览器。

### Milestone 4：验证与文档

工作：

1. TypeScript、Vitest、ESLint、Prettier 和 Production Build。
2. 使用 agent-browser 实际点击核心流程、切换平台/主题、填写备注。
3. 人工查看 Desktop、双端对比与窄视口截图。
4. 更新 README、AGENTS、Status 和 ExecPlan。

出口：

- 页面无 Browser Error。
- 关键界面不存在明显截断、遮挡或不可读文字。
- 项目负责人获得准确的本地启动命令。

## Validation

```bash
pnpm install
pnpm typecheck:design
pnpm test:design
pnpm lint:design
pnpm build:design
pnpm format:design
pnpm docs:check
git diff --check
```

浏览器验证：

```bash
pnpm dev:design
agent-browser --session anyssh-design open http://127.0.0.1:1430
```

开发服务必须监听 `0.0.0.0:1430`；本机自动化通过 Loopback URL 访问。

必须实际验证：

- 总览和流程模式切换。
- Linux、Android、双端对比切换。
- Light/Dark 切换。
- 欢迎 -> 创建保险库 -> 主机 -> Host Key -> OTP -> 终端。
- 页面状态和备注刷新后恢复。
- Desktop 与窄视口截图。
- Browser Console/Error 为空。

## Surprises & Discoveries

- 2026-08-02：在总览中直接复用完整可交互 Screen Canvas 会产生大量可聚焦
  控件；缩略图使用 HTML `inert`，只让“打开并评审”进入键盘和无障碍顺序。
- 2026-08-02：窄视口的右侧评审栏仅使用 `transform` 移出屏幕时仍会出现在
  Accessibility Tree；增加 `visibility` 和独立 Backdrop 后，关闭状态不再抢占
  焦点。
- 2026-08-02：`prettier --check .` 会扫描 Vite `dist`，与并行 Production
  Build 互相影响；增加应用级 `.prettierignore`，生成产物不参与源码格式检查。
- 2026-08-02：同一流程的 Linux 与 Android 主操作会同时存在于双端对比 DOM；
  自动化先切到单平台完成纵向流程，再单独验证 Android 与双端展示。

## Decision Log

- 2026-08-02：设计评审网页独立于 `apps/client`，避免 Mock Flow、评审备注和
  临时设计结构进入生产 Tauri Client。
- 2026-08-02：本计划只产出可评审基线，不提前决定生产 UI 技术栈；Tauri 与
  Android 真机适用性在设计通过后另建计划验证。
- 2026-08-02：评审记录只使用浏览器 `localStorage`，不使用服务器、Vault、
  Tauri IPC 或遥测。
- 2026-08-02：按项目负责人要求，人工访问的开发、预览和 Browser QA Web 服务
  统一监听 `0.0.0.0`；仅安全测试中明确要求 Loopback 的 Listener 保持
  `127.0.0.1`/`::1`。

## Outcomes & Retrospective

当前已完成：

- 独立 `@anyssh/design-review` Workspace 和固定端口 `1430`。
- 18 个核心界面、5 条用户旅程、Linux/Android/双端、Light/Dark。
- 欢迎 -> 创建保险库 -> 主机 -> Host Key -> OTP -> 终端可点击纵向流程。
- 每屏待评审/通过/待修改与本地备注，刷新后恢复。
- Desktop、Android、Dark 和 820px 窄视口人工截图；Browser Error 为空。
- Design Review 与现有 Product Frontend 的 TypeScript、Vitest 和 ESLint
  回归通过。

评审结论：

- 2026-08-02：项目负责人确认设计评审通过。
- 生产 UI 继续以当前 Accepted ADR-0001 的 Tauri 2 + React 为实现基础，先进行
  结构化代码组织和 Material Design 3 迁移。
- Linux 原生窗口不得继续显示与应用风格割裂的 GNOME 大白框；后续计划采用
  Linux-only Client-side Window Chrome 并完成 X11/Wayland 验证。

下一步由
[`ExecPlan 0012`](../active/0012-material3-production-ui-and-linux-window-chrome.md)
实施生产 UI 重构和 Linux Window Chrome，并保留后续 Android 真机验证出口。
