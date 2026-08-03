# AnySSH 产品构想

> 状态：初始产品需求
> 原始目标：实现一个开源、跨平台、具备 Termius Pro 同类核心能力的 SSH 客户端。产品可以参考通用工作流，但不得复制 Termius 的商标、素材或受版权保护的界面细节。

## 产品目标

AnySSH 是一个多平台 SSH 客户端，核心能力包括：

- SSH 主机与分组管理。
- Jump Host、上游代理和端口转发。
- 密码、私钥及 SSH Agent 认证。
- 脚本、密钥、主题和字体管理。
- 本地加密存储。
- 多后端端到端加密同步，首个后端为 WebDAV。

## 目标平台

- Linux：Wayland 与 X11。
- Windows。
- Android。
- iOS。

架构应保留未来支持 macOS 的可能性，但 macOS 当前不属于首发范围。

## 当前交付优先级

2026-08-02，项目负责人明确首要产品平台为：

1. Linux。
2. Android。

Linux 与 Android 的产品界面采用 Material Design 3，并先通过独立可点击网页
评审界面、信息架构和核心流程。Windows 与 iOS 继续保留架构兼容性，但不应在
Linux/Android 产品体验完成前主导 UI 开发顺序。

## 核心功能

### 主机与连接

- 类似现代 SSH 管理工具的主机列表和终端工作流。
- SSH 分组；分组可以向节点批量继承配置。
- SSH Jump Host/ProxyJump。
- SOCKS5 和 HTTP CONNECT 上游代理。
- Local、Remote、Dynamic SOCKS 端口转发。
- 密码认证，并允许用户在再次验证后查看已保存密码。
- SSH 私钥认证。
- 系统 SSH Agent 与应用内 Agent。

### 管理能力

- 脚本片段和 Runbook 管理。
- SSH 密钥导入、生成、查看和导出。
- 应用主题与终端主题。
- 自定义字体。
- Nerd Font、Powerline、Emoji、CJK 和特殊 Unicode 符号。

### 加密与同步

- 所有 Host、Credential、Private Key、Script 和同步配置均加密存储。
- 支持 PIN 解锁。
- 支持平台密钥环或安全硬件保护。
- 支持 Android 生物识别。
- 支持 iOS Face ID/Touch ID。
- 支持 Windows Hello 和 Linux Secret Service 的合理回退方案。
- 同步数据端到端加密，WebDAV 服务端不能读取业务明文。
- 同步架构允许未来增加 S3、自托管服务等 Provider。

## 安全体验

- 密码默认隐藏，但允许用户在再次认证后短暂查看。
- 私钥导出需要再次认证，并默认导出为加密格式。
- 应用锁定后不允许前端继续读取秘密。
- 密码、私钥和 Token 不进入日志或遥测。

## 非目标

- 不复制 Termius 的品牌、图标、截图和逐像素 UI。
- MVP 不支持任意本地插件或不受限脚本执行。
- MVP 不承诺 iOS 后台长期维持 SSH Session 或 Tunnel。
- 首个版本不追求覆盖所有老旧 SSH 算法；兼容模式按 Host 显式开启。
