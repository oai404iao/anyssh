export type Platform = "linux" | "android";
export type PlatformMode = Platform | "compare";
export type PrototypeTheme = "light" | "dark";
export type ReviewStatus = "pending" | "approved" | "changes";

export type ScreenId =
  | "welcome"
  | "vault-create"
  | "vault-unlock"
  | "hosts"
  | "host-editor"
  | "host-detail"
  | "host-key"
  | "otp"
  | "terminal"
  | "sessions"
  | "forwarding"
  | "credentials"
  | "credential-editor"
  | "snippets"
  | "snippet-run"
  | "appearance"
  | "security"
  | "known-hosts";

export type FlowId =
  "onboarding" | "connection" | "sessions" | "assets" | "settings";

export interface ScreenDefinition {
  id: ScreenId;
  flowId: FlowId;
  title: string;
  shortTitle: string;
  description: string;
  purpose: string;
  checkpoints: string[];
}

export interface FlowDefinition {
  id: FlowId;
  number: string;
  title: string;
  summary: string;
  screenIds: ScreenId[];
}

export interface ReviewEntry {
  status: ReviewStatus;
  note: string;
}

export type ReviewMap = Partial<Record<ScreenId, ReviewEntry>>;

export const SCREENS: ScreenDefinition[] = [
  {
    id: "welcome",
    flowId: "onboarding",
    title: "欢迎与产品价值",
    shortTitle: "欢迎",
    description: "首次打开时解释 AnySSH 的用途、隐私边界和跨设备价值。",
    purpose: "先让用户理解产品，再要求创建本地安全空间。",
    checkpoints: [
      "首屏不展示密码学实现名词",
      "一个主按钮即可开始",
      "Linux 与 Android 保持同一品牌气质",
    ],
  },
  {
    id: "vault-create",
    flowId: "onboarding",
    title: "创建本地保险库",
    shortTitle: "创建保险库",
    description: "通过简洁步骤设置本地 PIN，并说明锁定与恢复含义。",
    purpose: "把安全能力解释为用户价值，而不是工程实现。",
    checkpoints: [
      "PIN 规则清楚但不过度打扰",
      "Android 可引导生物识别",
      "明确数据仅保存在本机加密空间",
    ],
  },
  {
    id: "vault-unlock",
    flowId: "onboarding",
    title: "日常解锁",
    shortTitle: "解锁",
    description: "应用再次启动或自动锁定后，以 PIN 或平台能力快速恢复。",
    purpose: "高频操作必须安静、快速、可预测。",
    checkpoints: [
      "主操作始终处于拇指或键盘易达位置",
      "错误 PIN 不泄露内部错误",
      "Android 生物识别是加速入口而非唯一恢复方式",
    ],
  },
  {
    id: "hosts",
    flowId: "connection",
    title: "主机首页",
    shortTitle: "主机",
    description: "以搜索、分组和最近使用为中心的默认工作台。",
    purpose: "用户打开应用后可以立即找到并连接服务器。",
    checkpoints: [
      "常用主机优先于配置入口",
      "在线状态不伪装成实时探测",
      "空状态可以直接创建第一台主机",
    ],
  },
  {
    id: "host-editor",
    flowId: "connection",
    title: "添加或编辑主机",
    shortTitle: "编辑主机",
    description: "将基本信息、认证和高级路由分层，避免一页堆满字段。",
    purpose: "新用户只填写必要信息，高级用户仍可访问完整能力。",
    checkpoints: [
      "必填项数量最少",
      "Credential 和 Jump Route 使用引用",
      "高级设置默认折叠",
    ],
  },
  {
    id: "host-detail",
    flowId: "connection",
    title: "主机详情与连接",
    shortTitle: "主机详情",
    description: "展示目标、认证摘要、路由和近期会话，并突出连接动作。",
    purpose: "在连接前给用户足够上下文，但不展示秘密。",
    checkpoints: [
      "连接按钮是视觉主操作",
      "只显示 Credential 元数据",
      "错误和危险状态具有明确层级",
    ],
  },
  {
    id: "host-key",
    flowId: "connection",
    title: "首次 Host Key 确认",
    shortTitle: "主机身份",
    description: "用可理解的语言解释首次信任，并保留指纹核验信息。",
    purpose: "让安全确认既严格又不吓退普通用户。",
    checkpoints: [
      "Host、Port、算法和 SHA-256 指纹完整",
      "接受与取消语义明确",
      "Changed Key 使用独立的阻断界面",
    ],
  },
  {
    id: "otp",
    flowId: "connection",
    title: "附加认证与 OTP",
    shortTitle: "二次验证",
    description: "把服务端 Challenge 呈现为当前连接的一次性步骤。",
    purpose: "保持请求归属清楚，避免用户把 OTP 当作保存密码。",
    checkpoints: [
      "显示目标主机和当前认证阶段",
      "取消会终止当前连接",
      "响应不会被建议保存",
    ],
  },
  {
    id: "terminal",
    flowId: "connection",
    title: "SSH 终端工作区",
    shortTitle: "终端",
    description: "Linux 强调多 Tab 和信息密度，Android 强调全屏与辅助键盘。",
    purpose: "把终端作为产品核心，而不是配置页中的一个小区域。",
    checkpoints: [
      "终端拥有最大可用空间",
      "连接状态和危险操作始终可见",
      "Android 提供 Esc/Ctrl/Alt/方向键辅助栏",
    ],
  },
  {
    id: "sessions",
    flowId: "sessions",
    title: "会话列表",
    shortTitle: "会话",
    description: "集中查看活动、断开和需要用户操作的 Session。",
    purpose: "让移动端切换会话不依赖桌面式 Tab Strip。",
    checkpoints: [
      "活动与断开状态易识别",
      "待确认或待 OTP 会话有明显提示",
      "关闭和断开是不同操作",
    ],
  },
  {
    id: "forwarding",
    flowId: "sessions",
    title: "端口转发",
    shortTitle: "转发",
    description: "在当前会话中管理 Local、Dynamic 与 Remote 转发。",
    purpose: "展示绑定范围和生命周期，避免用户误以为是永久后台服务。",
    checkpoints: [
      "默认仅 Loopback",
      "显示当前 Session 归属",
      "停止转发是单独且可逆的动作",
    ],
  },
  {
    id: "credentials",
    flowId: "assets",
    title: "凭据管理",
    shortTitle: "凭据",
    description: "统一管理密码、私钥、系统 Agent 和交互式认证元数据。",
    purpose: "让 Host 只引用凭据，不重复保存认证信息。",
    checkpoints: [
      "列表不显示秘密",
      "类型和用户名一眼可见",
      "导入、生成和新建入口清楚分开",
    ],
  },
  {
    id: "credential-editor",
    flowId: "assets",
    title: "新建凭据",
    shortTitle: "编辑凭据",
    description: "按认证类型展示不同表单，并明确原生安全操作边界。",
    purpose: "减少用户面对不相关字段，并保留安全说明。",
    checkpoints: [
      "私钥文件由原生选择器读取",
      "密码只在当前表单短暂存在",
      "系统 Agent 通过指纹选择身份",
    ],
  },
  {
    id: "snippets",
    flowId: "assets",
    title: "命令片段",
    shortTitle: "片段",
    description: "以搜索、标签和最近使用组织可复用命令。",
    purpose: "让高频命令可发现，但不把 Snippet 包装成任意脚本。",
    checkpoints: ["正文按需读取", "变量数量和多行状态可见", "运行目标始终明确"],
  },
  {
    id: "snippet-run",
    flowId: "assets",
    title: "运行片段确认",
    shortTitle: "运行确认",
    description: "填写变量、预览最终文本并选择插入或直接运行。",
    purpose: "在内容进入远端 PTY 前给用户最后一次确认。",
    checkpoints: [
      "多行内容必须预览",
      "变量按字面量替换",
      "目标 Session 和 Enter Policy 清楚",
    ],
  },
  {
    id: "appearance",
    flowId: "settings",
    title: "外观与终端",
    shortTitle: "外观",
    description: "集中设置系统主题、色彩、终端字体和显示密度。",
    purpose: "让 Material 3 应用界面与终端配色彼此独立。",
    checkpoints: [
      "Light/Dark/System 清楚",
      "终端预览即时可见",
      "移动端不暴露不支持的文件路径能力",
    ],
  },
  {
    id: "security",
    flowId: "settings",
    title: "安全与自动锁定",
    shortTitle: "安全",
    description: "管理自动锁、平台解锁、剪贴板和敏感操作策略。",
    purpose: "把安全设置写成用户可以理解的行为结果。",
    checkpoints: [
      "默认值安全且有解释",
      "平台不可用能力明确降级",
      "危险操作与普通开关分区",
    ],
  },
  {
    id: "known-hosts",
    flowId: "settings",
    title: "已信任的主机身份",
    shortTitle: "主机身份",
    description: "查看 Endpoint、算法和指纹，并通过原生确认忘记信任。",
    purpose: "将安全资产放在可发现但不易误触的位置。",
    checkpoints: [
      "不展示完整 Public Key",
      "忘记信任需要额外确认",
      "Changed Key 不在这里提供一键替换",
    ],
  },
];

export const FLOWS: FlowDefinition[] = [
  {
    id: "onboarding",
    number: "01",
    title: "首次启动与解锁",
    summary: "先解释价值，再建立本地安全空间。",
    screenIds: ["welcome", "vault-create", "vault-unlock"],
  },
  {
    id: "connection",
    number: "02",
    title: "主机与连接",
    summary: "从主机首页进入严格的身份确认和终端。",
    screenIds: [
      "hosts",
      "host-editor",
      "host-detail",
      "host-key",
      "otp",
      "terminal",
    ],
  },
  {
    id: "sessions",
    number: "03",
    title: "会话与转发",
    summary: "管理多个实时 Session 及其端口转发。",
    screenIds: ["sessions", "forwarding"],
  },
  {
    id: "assets",
    number: "04",
    title: "凭据与效率工具",
    summary: "用受控对象复用认证信息和命令片段。",
    screenIds: ["credentials", "credential-editor", "snippets", "snippet-run"],
  },
  {
    id: "settings",
    number: "05",
    title: "外观与安全",
    summary: "统一管理应用体验、本地锁定和信任资产。",
    screenIds: ["appearance", "security", "known-hosts"],
  },
];

export const PRIMARY_JOURNEY: ScreenId[] = [
  "welcome",
  "vault-create",
  "hosts",
  "host-detail",
  "host-key",
  "otp",
  "terminal",
];

export const SCREEN_BY_ID = Object.fromEntries(
  SCREENS.map((screen) => [screen.id, screen]),
) as Record<ScreenId, ScreenDefinition>;

export const FLOW_BY_ID = Object.fromEntries(
  FLOWS.map((flow) => [flow.id, flow]),
) as Record<FlowId, FlowDefinition>;

export const NEXT_SCREEN: Partial<Record<ScreenId, ScreenId>> = {
  welcome: "vault-create",
  "vault-create": "hosts",
  "vault-unlock": "hosts",
  hosts: "host-detail",
  "host-editor": "hosts",
  "host-detail": "host-key",
  "host-key": "otp",
  otp: "terminal",
  terminal: "sessions",
  sessions: "terminal",
  forwarding: "terminal",
  credentials: "credential-editor",
  "credential-editor": "credentials",
  snippets: "snippet-run",
  "snippet-run": "terminal",
  appearance: "security",
  security: "known-hosts",
  "known-hosts": "hosts",
};

export const REVIEW_STATUS_LABEL: Record<ReviewStatus, string> = {
  pending: "待评审",
  approved: "通过",
  changes: "待修改",
};
