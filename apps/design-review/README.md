# AnySSH Design Review

独立的 Linux/Android Material Design 3 产品评审网页。

该应用只使用模拟数据，用于在生产 UI 重构前评审界面、信息架构和用户流程。
它不连接真实 SSH、Vault、数据库、网络或 Tauri IPC。

## 启动

从仓库根目录运行：

```bash
pnpm install
pnpm dev:design
```

服务监听 `0.0.0.0:1430`。本机打开 `http://127.0.0.1:1430`，其他设备使用
开发机局域网 IP 加端口 `1430`。

## 能力

- 全部核心界面总览。
- 可点击的首次启动到 SSH 终端主流程。
- Linux、Android 和双端对比。
- Light/Dark。
- 每个界面的待评审、通过、待修改状态。
- 浏览器本地备注；数据只存放在 `localStorage`。

## 验证

```bash
pnpm typecheck:design
pnpm test:design
pnpm lint:design
pnpm build:design
pnpm format:check:design
```
