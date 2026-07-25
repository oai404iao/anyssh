# ADR-0001：使用 Tauri 2 + React 作为统一应用壳

- 状态：Proposed
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

项目需要覆盖 Linux、Windows、Android 和 iOS，同时复用 SSH、加密、同步和数据库核心。终端需要成熟的 Unicode、字体、选择、搜索和渲染能力。

## 决策

使用：

- Tauri 2 作为桌面与移动应用壳。
- React + TypeScript 作为 UI。
- xterm.js 作为首发终端。
- Rust Core 承担业务和安全敏感逻辑。

## 备选方案

- Electron：移动端无法共用，运行时较重。
- Flutter：移动端优秀，但终端与 Rust FFI 风险更高。
- Compose Multiplatform：终端生态和 Rust 桥接成本更高。
- Qt/QML：平台成熟，但双语言栈和许可证治理成本更高。

## 后果

### 正面

- UI 可最大程度复用。
- Rust Core 可直接运行于 Tauri。
- 可使用 xterm.js 生态。

### 代价与风险

- Linux WebKitGTK/GPU 兼容性必须验证。
- 移动端 IME、软键盘和 WebGL 需要专项测试。
- 生物识别和平台 Key Store 需要自定义原生插件。

## 验证

Phase 0 必须证明：

- 四个平台可构建并启动。
- Linux X11/Wayland 有可用渲染路径。
- xterm.js 大输出和移动输入满足原型要求。

## 相关文档

- [总体技术设计](../design/technical-architecture-2026.md)
- [Phase 0 ExecPlan](../execplans/active/0001-phase-0-technical-validation.md)
