import type { ReactNode } from "react";
import {
  NEXT_SCREEN,
  SCREEN_BY_ID,
  type Platform,
  type PrototypeTheme,
  type ScreenId,
} from "../model";
import { Icon, type IconName } from "./Icon";
import {
  MockButton,
  MockChip,
  MockField,
  MockIconButton,
  MockListItem,
  MockSwitch,
  SectionHeading,
  StatusDot,
} from "./MockPrimitives";

interface ScreenCanvasProps {
  interactive?: boolean;
  onNavigate?: (screenId: ScreenId) => void;
  platform: Platform;
  screenId: ScreenId;
  theme: PrototypeTheme;
}

const ONBOARDING_SCREENS = new Set<ScreenId>([
  "welcome",
  "vault-create",
  "vault-unlock",
]);

export function ScreenCanvas({
  interactive = true,
  onNavigate,
  platform,
  screenId,
  theme,
}: ScreenCanvasProps) {
  const navigate = (target: ScreenId) => {
    if (interactive) onNavigate?.(target);
  };
  const screen = SCREEN_BY_ID[screenId];
  const onboarding = ONBOARDING_SCREENS.has(screenId);
  const terminal = screenId === "terminal";

  return (
    <div
      className={`product-device product-device-${platform} product-theme-${theme}`}
      inert={interactive ? undefined : true}
    >
      {platform === "linux" ? (
        <LinuxChrome
          activeScreen={screenId}
          onboarding={onboarding}
          terminal={terminal}
          title={screen.shortTitle}
          onNavigate={navigate}
        >
          <ProductScreen
            navigate={navigate}
            platform={platform}
            screenId={screenId}
          />
        </LinuxChrome>
      ) : (
        <AndroidChrome
          activeScreen={screenId}
          onboarding={onboarding}
          terminal={terminal}
          title={screen.shortTitle}
          onNavigate={navigate}
        >
          <ProductScreen
            navigate={navigate}
            platform={platform}
            screenId={screenId}
          />
        </AndroidChrome>
      )}
    </div>
  );
}

function LinuxChrome({
  activeScreen,
  children,
  onboarding,
  onNavigate,
  terminal,
  title,
}: {
  activeScreen: ScreenId;
  children: ReactNode;
  onboarding: boolean;
  onNavigate(screenId: ScreenId): void;
  terminal: boolean;
  title: string;
}) {
  return (
    <div className="linux-window">
      <div className="linux-titlebar">
        <div className="linux-titlebar-brand">
          <span className="product-brand-mark">
            <Icon name="terminal" />
          </span>
          <strong>AnySSH</strong>
        </div>
        <span className="linux-title">{title}</span>
        <div className="linux-window-actions" aria-hidden="true">
          <span />
          <span />
          <span />
        </div>
      </div>
      <div
        className={`linux-app-body ${
          onboarding ? "linux-onboarding-body" : ""
        }`}
      >
        {!onboarding && (
          <ProductNavigation
            activeScreen={activeScreen}
            onNavigate={onNavigate}
            platform="linux"
          />
        )}
        <main
          className={`product-main product-main-linux ${
            terminal ? "product-main-terminal" : ""
          }`}
        >
          {!onboarding && !terminal && (
            <div className="linux-product-topbar">
              <div>
                <span className="product-topbar-overline">
                  ANYSSH / {title}
                </span>
                <strong>{title}</strong>
              </div>
              <div className="product-topbar-actions">
                <StatusDot label="保险库已解锁" tone="success" />
                <MockIconButton label="更多操作" name="more" />
              </div>
            </div>
          )}
          {children}
        </main>
      </div>
    </div>
  );
}

function AndroidChrome({
  activeScreen,
  children,
  onboarding,
  onNavigate,
  terminal,
  title,
}: {
  activeScreen: ScreenId;
  children: ReactNode;
  onboarding: boolean;
  onNavigate(screenId: ScreenId): void;
  terminal: boolean;
  title: string;
}) {
  return (
    <div className="android-phone">
      <div className="android-statusbar">
        <strong>09:41</strong>
        <div>
          <span className="android-signal" />
          <span className="android-wifi">⌁</span>
          <span className="android-battery" />
        </div>
      </div>
      <div className="android-app-surface">
        {!onboarding && !terminal && (
          <div className="android-product-topbar">
            <MockIconButton label="打开导航" name="menu" />
            <strong>{title}</strong>
            <MockIconButton label="更多操作" name="more" />
          </div>
        )}
        <main
          className={`product-main product-main-android ${
            onboarding ? "product-main-onboarding" : ""
          } ${terminal ? "product-main-terminal" : ""}`}
        >
          {children}
        </main>
        {!onboarding && !terminal && (
          <ProductNavigation
            activeScreen={activeScreen}
            onNavigate={onNavigate}
            platform="android"
          />
        )}
      </div>
      <div className="android-gesture-bar">
        <span />
      </div>
    </div>
  );
}

function ProductNavigation({
  activeScreen,
  onNavigate,
  platform,
}: {
  activeScreen: ScreenId;
  onNavigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  const active = navigationSection(activeScreen);
  const items: Array<{
    id: "hosts" | "sessions" | "credentials" | "snippets" | "settings";
    icon: IconName;
    label: string;
    target: ScreenId;
  }> = [
    { id: "hosts", icon: "host", label: "主机", target: "hosts" },
    { id: "sessions", icon: "sessions", label: "会话", target: "sessions" },
    {
      id: "credentials",
      icon: "credentials",
      label: "凭据",
      target: "credentials",
    },
    { id: "snippets", icon: "snippet", label: "片段", target: "snippets" },
    {
      id: "settings",
      icon: "appearance",
      label: "设置",
      target: "appearance",
    },
  ];

  if (platform === "linux") {
    return (
      <aside className="product-nav-rail">
        <div className="product-nav-brand">
          <span className="product-brand-mark">
            <Icon name="terminal" />
          </span>
          <div>
            <strong>AnySSH</strong>
            <span>安全连接工作台</span>
          </div>
        </div>
        <nav>
          {items.map((item) => (
            <button
              className={active === item.id ? "active" : ""}
              key={item.id}
              onClick={() => onNavigate(item.target)}
              type="button"
            >
              <Icon name={item.icon} />
              <span>{item.label}</span>
            </button>
          ))}
        </nav>
        <div className="product-nav-footer">
          <span className="product-user-avatar">林</span>
          <div>
            <strong>本地保险库</strong>
            <span>已自动锁定保护</span>
          </div>
        </div>
      </aside>
    );
  }

  return (
    <nav className="android-bottom-nav">
      {items.slice(0, 4).map((item) => (
        <button
          className={active === item.id ? "active" : ""}
          key={item.id}
          onClick={() => onNavigate(item.target)}
          type="button"
        >
          <span>
            <Icon name={item.icon} />
          </span>
          {item.label}
        </button>
      ))}
    </nav>
  );
}

function ProductScreen({
  navigate,
  platform,
  screenId,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
  screenId: ScreenId;
}) {
  switch (screenId) {
    case "welcome":
      return <WelcomeScreen navigate={navigate} platform={platform} />;
    case "vault-create":
      return <VaultCreateScreen navigate={navigate} platform={platform} />;
    case "vault-unlock":
      return <VaultUnlockScreen navigate={navigate} platform={platform} />;
    case "hosts":
      return <HostsScreen navigate={navigate} platform={platform} />;
    case "host-editor":
      return <HostEditorScreen navigate={navigate} platform={platform} />;
    case "host-detail":
      return <HostDetailScreen navigate={navigate} platform={platform} />;
    case "host-key":
      return <HostKeyScreen navigate={navigate} platform={platform} />;
    case "otp":
      return <OtpScreen navigate={navigate} platform={platform} />;
    case "terminal":
      return <TerminalScreen navigate={navigate} platform={platform} />;
    case "sessions":
      return <SessionsScreen navigate={navigate} platform={platform} />;
    case "forwarding":
      return <ForwardingScreen navigate={navigate} platform={platform} />;
    case "credentials":
      return <CredentialsScreen navigate={navigate} platform={platform} />;
    case "credential-editor":
      return <CredentialEditorScreen navigate={navigate} platform={platform} />;
    case "snippets":
      return <SnippetsScreen navigate={navigate} platform={platform} />;
    case "snippet-run":
      return <SnippetRunScreen navigate={navigate} platform={platform} />;
    case "appearance":
      return <AppearanceScreen navigate={navigate} platform={platform} />;
    case "security":
      return <SecurityScreen navigate={navigate} platform={platform} />;
    case "known-hosts":
      return <KnownHostsScreen navigate={navigate} platform={platform} />;
  }
}

function WelcomeScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className={`welcome-screen welcome-screen-${platform}`}>
      <div className="welcome-copy">
        <div className="welcome-brand">
          <span className="welcome-logo">
            <Icon name="terminal" />
          </span>
          <strong>AnySSH</strong>
        </div>
        <MockChip selected>Linux · Android</MockChip>
        <h1>你的服务器，安全地随身携带。</h1>
        <p>
          管理主机、密钥和会话。数据默认保存在本地加密空间，只有你可以解锁。
        </p>
        <div className="welcome-benefits">
          <span>
            <Icon name="security" />
            本地加密
          </span>
          <span>
            <Icon name="sessions" />
            多会话终端
          </span>
          <span>
            <Icon name="compare" />
            双端一致体验
          </span>
        </div>
        <div className="welcome-actions">
          <MockButton icon="arrow" onClick={() => navigate("vault-create")}>
            开始使用
          </MockButton>
          <MockButton tone="text">了解数据安全</MockButton>
        </div>
        <span className="welcome-footnote">无需注册账号 · 不上传主机信息</span>
      </div>
      <div className="welcome-visual" aria-hidden="true">
        <div className="welcome-orbit welcome-orbit-one" />
        <div className="welcome-orbit welcome-orbit-two" />
        <div className="welcome-terminal-card">
          <div>
            <span />
            <span />
            <span />
          </div>
          <code>
            <span>anyssh@prod</span>:~$ uptime
            <br />
            <strong>secure session ready</strong>
            <br />
            <span>anyssh@prod</span>:~$ ▋
          </code>
        </div>
        <div className="welcome-shield">
          <Icon name="security" />
        </div>
      </div>
    </div>
  );
}

function VaultCreateScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className={`centered-product-screen vault-screen vault-${platform}`}>
      <div className="vault-progress">
        <span className="active" />
        <span />
      </div>
      <span className="large-product-icon">
        <Icon name="lock" />
      </span>
      <div className="centered-heading">
        <span>第 1 步，共 2 步</span>
        <h1>创建本地保险库</h1>
        <p>设置一个仅用于此设备的 PIN。锁定后，主机和凭据不会保持可读状态。</p>
      </div>
      <div className="vault-form-card">
        <MockField label="创建 PIN" supporting="至少 6 位数字" value="••••••" />
        <MockField label="再次输入 PIN" value="••••••" />
        <MockSwitch
          checked={platform === "android"}
          description={
            platform === "android"
              ? "下次可使用设备生物识别快速解锁"
              : "可用时使用桌面安全存储辅助解锁"
          }
          label={platform === "android" ? "启用生物识别" : "使用系统安全存储"}
        />
        <MockButton icon="arrow" onClick={() => navigate("hosts")}>
          创建并继续
        </MockButton>
      </div>
      <div className="product-info-note">
        <Icon name="info" />
        <span>忘记 PIN 时，需要使用恢复方式或在其他已授权设备上恢复数据。</span>
      </div>
    </div>
  );
}

function VaultUnlockScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="centered-product-screen unlock-screen">
      <div className="unlock-brand">
        <span className="welcome-logo">
          <Icon name="terminal" />
        </span>
        <strong>AnySSH</strong>
      </div>
      <span className="unlock-avatar">林</span>
      <div className="centered-heading">
        <h1>欢迎回来</h1>
        <p>解锁本地保险库以继续访问你的主机和会话。</p>
      </div>
      <div className="unlock-pin-dots" aria-label="已输入六位 PIN">
        {Array.from({ length: 6 }, (_, index) => (
          <span key={index} />
        ))}
      </div>
      <MockButton icon="lock" onClick={() => navigate("hosts")}>
        解锁
      </MockButton>
      {platform === "android" && (
        <MockButton icon="fingerprint" tone="tonal">
          使用生物识别
        </MockButton>
      )}
      <button className="product-text-link" type="button">
        使用恢复方式
      </button>
    </div>
  );
}

function HostsScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page hosts-page">
      <SectionHeading
        action={
          platform === "linux" ? (
            <MockButton
              compact
              icon="plus"
              onClick={() => navigate("host-editor")}
            >
              添加主机
            </MockButton>
          ) : undefined
        }
        eyebrow="工作台"
        title="主机"
      />
      <div className="hosts-toolbar">
        <div className="product-search">
          <Icon name="search" />
          <span>搜索主机、地址或标签</span>
          <kbd>⌘ K</kbd>
        </div>
        <div className="host-filter-chips">
          <MockChip selected>全部</MockChip>
          <MockChip>生产环境</MockChip>
          <MockChip>个人设备</MockChip>
          <MockChip>实验室</MockChip>
        </div>
      </div>
      <div className="host-grid">
        <HostCard
          address="prod.example.com"
          color="violet"
          label="生产"
          name="生产服务器"
          onClick={() => navigate("host-detail")}
          status="最近 12 分钟前连接"
        />
        <HostCard
          address="192.168.1.20"
          color="teal"
          label="个人"
          name="家庭 NAS"
          onClick={() => navigate("host-detail")}
          status="经家庭网络访问"
        />
        <HostCard
          address="10.10.0.8"
          color="amber"
          label="实验"
          name="Kubernetes Lab"
          onClick={() => navigate("host-detail")}
          status="通过 Jump Route"
        />
      </div>
      <div className="recent-section">
        <SectionHeading title="最近使用" />
        <MockListItem
          description="prod.example.com · Ed25519"
          icon="terminal"
          onClick={() => navigate("host-detail")}
          title="生产服务器"
          trailing={<span className="list-time">12 分钟</span>}
        />
        <MockListItem
          description="192.168.1.20 · 私钥"
          icon="terminal"
          onClick={() => navigate("host-detail")}
          title="家庭 NAS"
          trailing={<span className="list-time">昨天</span>}
        />
      </div>
      {platform === "android" && (
        <button
          aria-label="添加主机"
          className="product-fab"
          onClick={() => navigate("host-editor")}
          type="button"
        >
          <Icon name="plus" />
        </button>
      )}
    </div>
  );
}

function HostCard({
  address,
  color,
  label,
  name,
  onClick,
  status,
}: {
  address: string;
  color: "violet" | "teal" | "amber";
  label: string;
  name: string;
  onClick(): void;
  status: string;
}) {
  return (
    <button className="host-product-card" onClick={onClick} type="button">
      <span className={`host-card-avatar host-card-avatar-${color}`}>
        <Icon name="host" />
      </span>
      <span className="host-card-menu">
        <Icon name="more" />
      </span>
      <span className="host-card-label">{label}</span>
      <strong>{name}</strong>
      <code>{address}</code>
      <span className="host-card-status">
        <span />
        {status}
      </span>
    </button>
  );
}

function HostEditorScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page editor-product-page">
      <SectionHeading
        action={
          platform === "linux" ? (
            <MockButton compact onClick={() => navigate("hosts")}>
              保存主机
            </MockButton>
          ) : undefined
        }
        eyebrow="主机 / 新建"
        title="添加主机"
      />
      <div className="editor-product-layout">
        <section className="product-form-section">
          <div className="product-form-section-title">
            <span>1</span>
            <div>
              <strong>基本信息</strong>
              <p>连接目标和列表中显示的名称。</p>
            </div>
          </div>
          <div className="product-field-grid">
            <MockField label="显示名称" value="生产服务器" />
            <MockField label="主机地址" value="prod.example.com" />
            <MockField label="端口" value="22" />
            <MockField label="所属分组" value="生产环境" />
          </div>
        </section>
        <section className="product-form-section">
          <div className="product-form-section-title">
            <span>2</span>
            <div>
              <strong>认证方式</strong>
              <p>主机只引用已经保存的凭据。</p>
            </div>
          </div>
          <MockField
            label="Credential"
            supporting="用户名：deploy · Ed25519 私钥"
            trailing={<Icon name="chevron" />}
            value="生产部署密钥"
          />
          <button
            className="inline-create-action"
            onClick={() => navigate("credential-editor")}
            type="button"
          >
            <Icon name="plus" />
            新建 Credential
          </button>
        </section>
        <section className="product-form-section product-form-section-muted">
          <div className="product-form-section-title">
            <span>3</span>
            <div>
              <strong>高级连接</strong>
              <p>Jump Route、算法兼容和 Keepalive。</p>
            </div>
          </div>
          <MockListItem
            description="通过 Bastion East 到达目标"
            icon="forwarding"
            title="Jump Route"
            trailing={<MockChip selected>已配置</MockChip>}
          />
        </section>
      </div>
      {platform === "android" && (
        <div className="android-sticky-action">
          <MockButton onClick={() => navigate("hosts")}>保存主机</MockButton>
        </div>
      )}
    </div>
  );
}

function HostDetailScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page host-detail-page">
      <section className="host-detail-hero">
        <span className="host-detail-avatar">
          <Icon name="host" />
        </span>
        <div className="host-detail-copy">
          <div>
            <MockChip tone="success">生产环境</MockChip>
            <MockChip>通过 Jump Route</MockChip>
          </div>
          <h2>生产服务器</h2>
          <code>deploy@prod.example.com:22</code>
          <p>用于线上 API 与 Worker 运维。最近连接于 12 分钟前。</p>
        </div>
        <div className="host-detail-actions">
          <MockButton icon="terminal" onClick={() => navigate("host-key")}>
            连接
          </MockButton>
          <MockButton icon="edit" tone="outlined">
            编辑
          </MockButton>
        </div>
      </section>
      <div className="host-detail-grid">
        <section className="detail-surface">
          <SectionHeading eyebrow="连接计划" title="认证与路由" />
          <MockListItem
            description="deploy · SHA256:2JS...sdQ"
            icon="key"
            title="生产部署密钥"
            trailing={<MockChip tone="success">Ed25519</MockChip>}
          />
          <MockListItem
            description="Bastion East → 生产服务器"
            icon="forwarding"
            title="Production Route"
          />
        </section>
        <section className="detail-surface">
          <SectionHeading eyebrow="最近活动" title="会话记录" />
          <div className="activity-timeline">
            <span />
            <div>
              <strong>交互式会话</strong>
              <p>持续 18 分钟 · 正常断开</p>
            </div>
            <time>12 分钟前</time>
          </div>
          <div className="activity-timeline">
            <span />
            <div>
              <strong>Local Forward</strong>
              <p>127.0.0.1:5432 → database:5432</p>
            </div>
            <time>昨天</time>
          </div>
        </section>
      </div>
      {platform === "android" && (
        <div className="android-sticky-action">
          <MockButton icon="terminal" onClick={() => navigate("host-key")}>
            连接到主机
          </MockButton>
        </div>
      )}
    </div>
  );
}

function HostKeyScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="connection-dialog-stage">
      <div className="dialog-host-backdrop">
        <span className="host-detail-avatar">
          <Icon name="host" />
        </span>
        <strong>正在连接生产服务器</strong>
        <span>建立加密通道…</span>
      </div>
      <section className={`product-dialog host-key-product-dialog ${platform}`}>
        <span className="dialog-leading-icon">
          <Icon name="fingerprint" />
        </span>
        <span className="dialog-overline">首次连接</span>
        <h2>确认主机身份</h2>
        <p>
          这是首次连接到此地址。请核对指纹；接受后 AnySSH
          会记住它，后续变化将被阻断。
        </p>
        <dl className="host-key-details">
          <div>
            <dt>目标</dt>
            <dd>prod.example.com:22</dd>
          </div>
          <div>
            <dt>算法</dt>
            <dd>ssh-ed25519</dd>
          </div>
        </dl>
        <div className="fingerprint-card">
          <span>SHA-256 指纹</span>
          <code>SHA256:W8eQ mR5K 9pNv 2JSf A7cd hG3x Lm4P sdQ</code>
          <MockIconButton label="复制指纹" name="copy" />
        </div>
        <div className="product-dialog-note">
          <Icon name="info" />
          如果管理员提供了指纹，请在继续前进行比较。
        </div>
        <div className="product-dialog-actions">
          <MockButton tone="text">取消连接</MockButton>
          <MockButton
            icon="arrow"
            onClick={() => navigate("otp")}
            tone="filled"
          >
            信任并继续
          </MockButton>
        </div>
      </section>
    </div>
  );
}

function OtpScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="connection-dialog-stage">
      <div className="dialog-host-backdrop">
        <span className="host-detail-avatar">
          <Icon name="host" />
        </span>
        <strong>生产服务器</strong>
        <StatusDot label="第一因子已通过" tone="success" />
      </div>
      <section className={`product-dialog otp-product-dialog ${platform}`}>
        <span className="dialog-leading-icon">
          <Icon name="security" />
        </span>
        <span className="dialog-overline">附加认证</span>
        <h2>输入一次性验证码</h2>
        <p>生产服务器要求额外验证。此验证码只属于当前连接，不会被保存。</p>
        <div className="otp-context">
          <span>目标</span>
          <strong>deploy@prod.example.com</strong>
        </div>
        <div className="otp-inputs" aria-label="六位验证码">
          {["5", "7", "2", "9", "4", "1"].map((digit, index) => (
            <span className={index === 5 ? "active" : ""} key={index}>
              {digit}
            </span>
          ))}
        </div>
        <span className="otp-time">验证码将在 21 秒后刷新</span>
        <div className="product-dialog-actions">
          <MockButton tone="text">取消</MockButton>
          <MockButton
            icon="arrow"
            onClick={() => navigate("terminal")}
            tone="filled"
          >
            验证并连接
          </MockButton>
        </div>
      </section>
    </div>
  );
}

function TerminalScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className={`terminal-product-screen terminal-${platform}`}>
      {platform === "linux" ? (
        <>
          <div className="terminal-tabs">
            <button className="active" type="button">
              <span />
              生产服务器
              <small>×</small>
            </button>
            <button type="button">
              <span />
              家庭 NAS
              <small>×</small>
            </button>
            <MockIconButton label="新建会话" name="plus" />
            <div className="terminal-tabs-spacer" />
            <StatusDot label="已连接" tone="success" />
            <MockButton
              compact
              onClick={() => navigate("sessions")}
              tone="outlined"
            >
              会话
            </MockButton>
          </div>
          <div className="terminal-workspace">
            <TerminalSurface />
            <aside className="terminal-side-panel">
              <div>
                <span className="product-topbar-overline">当前连接</span>
                <h3>生产服务器</h3>
                <code>deploy@prod.example.com</code>
              </div>
              <dl>
                <div>
                  <dt>算法</dt>
                  <dd>ML-KEM / ChaCha20</dd>
                </div>
                <div>
                  <dt>路由</dt>
                  <dd>Bastion East</dd>
                </div>
                <div>
                  <dt>会话</dt>
                  <dd>18 分钟</dd>
                </div>
              </dl>
              <MockButton
                icon="forwarding"
                onClick={() => navigate("forwarding")}
                tone="tonal"
              >
                端口转发
              </MockButton>
              <MockButton tone="outlined">断开连接</MockButton>
            </aside>
          </div>
        </>
      ) : (
        <>
          <div className="android-terminal-topbar">
            <MockIconButton
              label="返回会话"
              name="back"
              onClick={() => navigate("sessions")}
            />
            <div>
              <strong>生产服务器</strong>
              <span>
                <i />
                已连接 · 18 分钟
              </span>
            </div>
            <MockIconButton label="更多操作" name="more" />
          </div>
          <TerminalSurface />
          <div className="terminal-accessory-bar">
            {["ESC", "CTRL", "ALT", "TAB", "↑", "↓", "←", "→"].map((key) => (
              <button key={key} type="button">
                {key}
              </button>
            ))}
          </div>
          <div className="android-terminal-actions">
            <MockIconButton label="会话列表" name="sessions" />
            <MockIconButton label="命令片段" name="snippet" />
            <MockIconButton label="端口转发" name="forwarding" />
            <MockIconButton label="显示键盘" name="terminal" selected />
          </div>
        </>
      )}
    </div>
  );
}

function TerminalSurface() {
  return (
    <div className="terminal-surface-mock">
      <div className="terminal-command">
        <span className="terminal-user">deploy@production</span>
        <span className="terminal-path">:~$</span> systemctl status anyssh-api
      </div>
      <div className="terminal-output">
        <span className="terminal-success">● anyssh-api.service</span> - API
        Service
        <br />
        &nbsp;&nbsp;Loaded: loaded (/etc/systemd/system/anyssh-api.service)
        <br />
        &nbsp;&nbsp;Active:{" "}
        <span className="terminal-success">active (running)</span> since Sun
        09:23:18
        <br />
        &nbsp;&nbsp;Tasks: 24 &nbsp; Memory: 182.4M
      </div>
      <div className="terminal-command">
        <span className="terminal-user">deploy@production</span>
        <span className="terminal-path">:~$</span> docker ps --format
        &apos;table {"{{.Names}}"}\t{"{{.Status}}"}&apos;
      </div>
      <div className="terminal-table">
        <span>NAMES</span>
        <span>STATUS</span>
        <strong>api-01</strong>
        <em>Up 5 days (healthy)</em>
        <strong>worker-01</strong>
        <em>Up 5 days</em>
        <strong>redis</strong>
        <em>Up 12 days</em>
      </div>
      <div className="terminal-command terminal-current-line">
        <span className="terminal-user">deploy@production</span>
        <span className="terminal-path">:~$</span>
        <span className="terminal-cursor" />
      </div>
    </div>
  );
}

function SessionsScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page sessions-page">
      <SectionHeading
        action={
          platform === "linux" ? (
            <MockButton compact icon="plus">
              新建会话
            </MockButton>
          ) : undefined
        }
        eyebrow="实时状态"
        title="会话"
      />
      <div className="sessions-summary">
        <div>
          <span>2</span>
          <strong>活动会话</strong>
        </div>
        <div>
          <span>1</span>
          <strong>等待操作</strong>
        </div>
        <div>
          <span>3</span>
          <strong>今日连接</strong>
        </div>
      </div>
      <div className="session-product-list">
        <button
          className="session-product-card"
          onClick={() => navigate("terminal")}
          type="button"
        >
          <span className="session-product-icon connected">
            <Icon name="terminal" />
          </span>
          <div>
            <span className="session-card-overline">
              <i />
              已连接 · 18 分钟
            </span>
            <strong>生产服务器</strong>
            <code>deploy@prod.example.com</code>
            <span className="session-card-preview">
              systemctl status anyssh-api
            </span>
          </div>
          <Icon name="chevron" />
        </button>
        <button className="session-product-card" type="button">
          <span className="session-product-icon warning">
            <Icon name="security" />
          </span>
          <div>
            <span className="session-card-overline warning">
              等待一次性验证码
            </span>
            <strong>Kubernetes Lab</strong>
            <code>admin@10.10.0.8</code>
            <span className="session-card-preview">
              需要在 01:42 内完成验证
            </span>
          </div>
          <MockChip tone="warning">操作</MockChip>
        </button>
        <button className="session-product-card" type="button">
          <span className="session-product-icon">
            <Icon name="terminal" />
          </span>
          <div>
            <span className="session-card-overline">已断开 · 保留滚屏</span>
            <strong>家庭 NAS</strong>
            <code>lin@192.168.1.20</code>
            <span className="session-card-preview">最后活动于 34 分钟前</span>
          </div>
          <Icon name="chevron" />
        </button>
      </div>
      {platform === "android" && (
        <button aria-label="新建会话" className="product-fab" type="button">
          <Icon name="plus" />
        </button>
      )}
    </div>
  );
}

function ForwardingScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page forwarding-page">
      <SectionHeading
        action={
          platform === "linux" ? (
            <MockButton compact icon="plus">
              新建转发
            </MockButton>
          ) : undefined
        }
        eyebrow="生产服务器 / 当前会话"
        title="端口转发"
      />
      <div className="forward-policy-note">
        <Icon name="security" />
        <div>
          <strong>仅在当前会话中有效</strong>
          <span>断开、关闭 Tab 或锁定保险库时自动停止。默认只监听本机。</span>
        </div>
      </div>
      <div className="forwarding-product-list">
        <ForwardCard
          destination="database.internal:5432"
          kind="Local"
          listen="127.0.0.1:15432"
          tone="teal"
        />
        <ForwardCard
          destination="SOCKS5 CONNECT"
          kind="Dynamic"
          listen="127.0.0.1:1080"
          tone="violet"
        />
        <ForwardCard
          destination="127.0.0.1:3000"
          kind="Remote"
          listen="server:38021"
          tone="amber"
        />
      </div>
      <section className="forward-create-card">
        <SectionHeading eyebrow="快速创建" title="新的 Local Forward" />
        <div className="product-field-grid">
          <MockField label="本地监听" value="127.0.0.1" />
          <MockField label="端口" value="0（自动分配）" />
          <MockField label="目标地址" value="127.0.0.1" />
          <MockField label="目标端口" value="8080" />
        </div>
        <MockButton icon="plus">启动转发</MockButton>
      </section>
      <button
        className="product-text-link back-to-terminal"
        onClick={() => navigate("terminal")}
        type="button"
      >
        返回当前终端
      </button>
    </div>
  );
}

function ForwardCard({
  destination,
  kind,
  listen,
  tone,
}: {
  destination: string;
  kind: string;
  listen: string;
  tone: "teal" | "violet" | "amber";
}) {
  return (
    <div className="forward-product-card">
      <span className={`forward-kind forward-kind-${tone}`}>{kind}</span>
      <div>
        <strong>{listen}</strong>
        <span>
          <Icon name="arrow" />
          {destination}
        </span>
      </div>
      <StatusDot label="运行中" tone="success" />
      <MockButton compact tone="outlined">
        停止
      </MockButton>
    </div>
  );
}

function CredentialsScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page credentials-page">
      <SectionHeading
        action={
          platform === "linux" ? (
            <div className="heading-action-group">
              <MockButton compact tone="outlined">
                导入私钥
              </MockButton>
              <MockButton
                compact
                icon="plus"
                onClick={() => navigate("credential-editor")}
              >
                新建凭据
              </MockButton>
            </div>
          ) : undefined
        }
        eyebrow="安全资产"
        title="凭据"
      />
      <div className="credentials-summary">
        <div>
          <Icon name="key" />
          <span>
            <strong>5 个凭据</strong>
            <small>全部保存在本地保险库</small>
          </span>
        </div>
        <MockChip tone="success">无明文导出</MockChip>
      </div>
      <div className="credential-product-list">
        <CredentialCard
          detail="deploy · SHA256:2JS…sdQ"
          kind="Ed25519 私钥"
          name="生产部署密钥"
          tone="violet"
        />
        <CredentialCard
          detail="lin · 最近使用于家庭 NAS"
          kind="密码"
          name="家庭服务器"
          tone="teal"
        />
        <CredentialCard
          detail="workstation-key · Linux Agent"
          kind="系统 Agent"
          name="工作站身份"
          tone="amber"
        />
        <CredentialCard
          detail="admin · 不保存响应"
          kind="交互式认证"
          name="OTP 管理员"
          tone="blue"
        />
      </div>
      {platform === "android" && (
        <button
          aria-label="新建凭据"
          className="product-fab"
          onClick={() => navigate("credential-editor")}
          type="button"
        >
          <Icon name="plus" />
        </button>
      )}
    </div>
  );
}

function CredentialCard({
  detail,
  kind,
  name,
  tone,
}: {
  detail: string;
  kind: string;
  name: string;
  tone: "violet" | "teal" | "amber" | "blue";
}) {
  return (
    <button className="credential-product-card" type="button">
      <span className={`credential-kind-icon credential-${tone}`}>
        <Icon name="key" />
      </span>
      <span>
        <MockChip>{kind}</MockChip>
        <strong>{name}</strong>
        <small>{detail}</small>
      </span>
      <Icon name="chevron" />
    </button>
  );
}

function CredentialEditorScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page credential-editor-page">
      <SectionHeading
        eyebrow="凭据 / 新建"
        title="选择认证方式"
        action={
          platform === "linux" ? (
            <MockButton compact onClick={() => navigate("credentials")}>
              保存凭据
            </MockButton>
          ) : undefined
        }
      />
      <div className="credential-kind-selector">
        <button className="selected" type="button">
          <Icon name="key" />
          <strong>私钥</strong>
          <span>导入或生成 OpenSSH Key</span>
        </button>
        <button type="button">
          <Icon name="lock" />
          <strong>密码</strong>
          <span>保存用户名与密码</span>
        </button>
        <button type="button">
          <Icon name="fingerprint" />
          <strong>系统 Agent</strong>
          <span>使用外部签名身份</span>
        </button>
      </div>
      <section className="credential-editor-card">
        <div className="credential-editor-heading">
          <span className="large-product-icon">
            <Icon name="key" />
          </span>
          <div>
            <span>OPENSSH PRIVATE KEY</span>
            <h3>导入私钥</h3>
            <p>文件选择、读取和口令提示都由系统原生界面处理。</p>
          </div>
        </div>
        <div className="product-field-grid">
          <MockField label="名称" value="生产部署密钥" />
          <MockField label="默认用户名" value="deploy" />
        </div>
        <div className="native-picker-surface">
          <Icon name="key" />
          <div>
            <strong>选择 OpenSSH 私钥文件</strong>
            <span>支持 Ed25519 与 RSA，加密密钥会安全提示口令。</span>
          </div>
          <MockButton compact tone="tonal">
            打开选择器
          </MockButton>
        </div>
        <div className="product-info-note">
          <Icon name="security" />
          <span>路径、私钥内容和 Passphrase 不会进入普通应用界面。</span>
        </div>
      </section>
      {platform === "android" && (
        <div className="android-sticky-action">
          <MockButton onClick={() => navigate("credentials")}>
            保存凭据
          </MockButton>
        </div>
      )}
    </div>
  );
}

function SnippetsScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page snippets-page">
      <SectionHeading
        action={
          platform === "linux" ? (
            <MockButton compact icon="plus">
              新建片段
            </MockButton>
          ) : undefined
        }
        eyebrow="效率工具"
        title="命令片段"
      />
      <div className="product-search">
        <Icon name="search" />
        <span>搜索命令、标签或变量</span>
      </div>
      <div className="snippet-filter-row">
        <MockChip selected>全部</MockChip>
        <MockChip>部署</MockChip>
        <MockChip>诊断</MockChip>
        <MockChip>数据库</MockChip>
      </div>
      <div className="snippet-product-grid">
        <SnippetCard
          body="docker compose pull && docker compose up -d"
          meta="2 个变量 · 多行"
          name="部署新版本"
          onClick={() => navigate("snippet-run")}
          tag="部署"
        />
        <SnippetCard
          body="journalctl -u {{service}} --since '30 min ago'"
          meta="1 个变量 · 单行"
          name="查看服务日志"
          onClick={() => navigate("snippet-run")}
          tag="诊断"
        />
        <SnippetCard
          body="df -h && free -h && uptime"
          meta="无变量 · 多行"
          name="系统健康检查"
          onClick={() => navigate("snippet-run")}
          tag="诊断"
        />
        <SnippetCard
          body="pg_dump -Fc {{database}} > {{target}}"
          meta="2 个变量 · 单行"
          name="PostgreSQL 备份"
          onClick={() => navigate("snippet-run")}
          tag="数据库"
        />
      </div>
      {platform === "android" && (
        <button aria-label="新建片段" className="product-fab" type="button">
          <Icon name="plus" />
        </button>
      )}
    </div>
  );
}

function SnippetCard({
  body,
  meta,
  name,
  onClick,
  tag,
}: {
  body: string;
  meta: string;
  name: string;
  onClick(): void;
  tag: string;
}) {
  return (
    <button className="snippet-product-card" onClick={onClick} type="button">
      <div>
        <MockChip>{tag}</MockChip>
        <span className="snippet-card-more" title="更多操作">
          <Icon name="more" />
        </span>
      </div>
      <strong>{name}</strong>
      <code>{body}</code>
      <span>{meta}</span>
    </button>
  );
}

function SnippetRunScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page snippet-run-page">
      <SectionHeading
        eyebrow="命令片段 / 部署"
        title="部署新版本"
        action={<MockChip tone="warning">多行确认</MockChip>}
      />
      <div className="snippet-run-layout">
        <section className="snippet-variable-card">
          <SectionHeading eyebrow="步骤 1" title="填写变量" />
          <MockField label="service" value="anyssh-api" />
          <MockField label="version" value="2026.08.02" />
          <MockField
            label="目标会话"
            trailing={<Icon name="chevron" />}
            value="生产服务器 · 已连接"
          />
          <MockSwitch
            checked
            description="运行后自动发送 Enter"
            label="立即执行"
          />
        </section>
        <section className="snippet-preview-card">
          <SectionHeading eyebrow="步骤 2" title="确认最终内容" />
          <div className="snippet-code-preview">
            <span>01</span>
            <code>cd /srv/anyssh-api</code>
            <span>02</span>
            <code>docker compose pull anyssh-api:2026.08.02</code>
            <span>03</span>
            <code>docker compose up -d anyssh-api</code>
          </div>
          <div className="product-warning-note">
            <Icon name="warning" />
            <span>这三行内容将被发送到生产服务器的当前 PTY。</span>
          </div>
          <div className="snippet-run-actions">
            <MockButton tone="outlined">插入但不执行</MockButton>
            <MockButton icon="terminal" onClick={() => navigate("terminal")}>
              确认并运行
            </MockButton>
          </div>
        </section>
      </div>
      {platform === "android" && (
        <div className="android-sticky-action">
          <MockButton icon="terminal" onClick={() => navigate("terminal")}>
            确认并运行
          </MockButton>
        </div>
      )}
    </div>
  );
}

function AppearanceScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page appearance-page">
      <SectionHeading
        action={
          platform === "linux" ? (
            <MockButton compact tone="outlined">
              导入终端主题
            </MockButton>
          ) : undefined
        }
        eyebrow="设置"
        title="外观与终端"
      />
      <div className="appearance-product-layout">
        <div className="appearance-settings-column">
          <section className="appearance-setting-card">
            <SectionHeading eyebrow="应用界面" title="主题" />
            <div className="theme-choice-row">
              <button type="button">
                <span className="theme-swatch system-theme" />
                <strong>跟随系统</strong>
              </button>
              <button className="selected" type="button">
                <span className="theme-swatch light-theme" />
                <strong>浅色</strong>
              </button>
              <button type="button">
                <span className="theme-swatch dark-theme" />
                <strong>深色</strong>
              </button>
            </div>
            <span className="appearance-field-label">主色</span>
            <div className="color-choice-row">
              {["teal", "blue", "violet", "amber", "rose"].map(
                (color, index) => (
                  <button
                    aria-label={`选择${color}主色`}
                    className={`${color} ${index === 0 ? "selected" : ""}`}
                    key={color}
                    type="button"
                  />
                ),
              )}
            </div>
          </section>
          <section className="appearance-setting-card">
            <SectionHeading eyebrow="终端" title="字体与显示" />
            <MockField
              label="字体"
              trailing={<Icon name="chevron" />}
              value="JetBrains Mono Nerd Font"
            />
            <div className="product-field-grid">
              <MockField label="字号" value="14 px" />
              <MockField label="行高" value="舒适 · 1.45" />
            </div>
            <MockSwitch checked label="编程连字" />
          </section>
        </div>
        <section className="appearance-preview-card">
          <span className="product-topbar-overline">实时预览</span>
          <div className="mini-app-preview">
            <aside>
              <span className="product-brand-mark">
                <Icon name="terminal" />
              </span>
              <i />
              <i />
              <i />
            </aside>
            <div>
              <span />
              <strong>生产服务器</strong>
              <code>deploy@prod:~$ echo &quot;你好 AnySSH&quot;</code>
              <code className="preview-output">你好 AnySSH ✓</code>
            </div>
          </div>
          <div className="appearance-preview-copy">
            <strong>专业青绿</strong>
            <span>清晰、可靠，在 Light/Dark 中保持一致语义。</span>
          </div>
          <MockButton onClick={() => navigate("security")} tone="tonal">
            查看安全设置
          </MockButton>
        </section>
      </div>
    </div>
  );
}

function SecurityScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page security-page">
      <SectionHeading eyebrow="设置" title="安全与自动锁定" />
      <div className="security-health-card">
        <span className="security-health-icon">
          <Icon name="security" />
        </span>
        <div>
          <span>当前设备</span>
          <strong>保护状态良好</strong>
          <p>本地保险库已加密，自动锁定和敏感操作再验证均已开启。</p>
        </div>
        <MockChip tone="success">已保护</MockChip>
      </div>
      <div className="security-settings-grid">
        <section className="security-setting-section">
          <SectionHeading eyebrow="解锁" title="本地访问" />
          <MockSwitch
            checked
            description="应用离开前台 5 分钟后锁定"
            label="自动锁定"
          />
          <MockListItem description="5 分钟" icon="lock" title="锁定延迟" />
          <MockSwitch
            checked={platform === "android"}
            description={
              platform === "android"
                ? "使用设备生物识别授权解锁"
                : "可用时使用系统安全存储"
            }
            label={platform === "android" ? "生物识别" : "平台安全解锁"}
          />
        </section>
        <section className="security-setting-section">
          <SectionHeading eyebrow="敏感操作" title="额外保护" />
          <MockSwitch
            checked
            description="导出私钥前再次验证 PIN"
            label="私钥导出确认"
          />
          <MockSwitch
            checked
            description="复制秘密后 30 秒尝试清理"
            label="剪贴板自动清理"
          />
          <MockListItem
            description="3 个已信任 Endpoint"
            icon="fingerprint"
            onClick={() => navigate("known-hosts")}
            title="已信任的主机身份"
          />
        </section>
      </div>
      <section className="danger-zone">
        <SectionHeading eyebrow="危险区域" title="恢复与删除" />
        <MockListItem
          description="创建离线恢复材料"
          icon="key"
          title="恢复方式"
        />
        <MockButton tone="danger">删除此设备上的保险库</MockButton>
      </section>
    </div>
  );
}

function KnownHostsScreen({
  navigate,
  platform,
}: {
  navigate(screenId: ScreenId): void;
  platform: Platform;
}) {
  return (
    <div className="product-page known-hosts-page">
      <SectionHeading eyebrow="安全 / 主机身份" title="已信任的主机身份" />
      <div className="product-info-note known-hosts-note">
        <Icon name="info" />
        <span>
          AnySSH 会在首次接受后记住 Endpoint
          和指纹。后续密钥变化会直接阻断连接。
        </span>
      </div>
      <div className="product-search">
        <Icon name="search" />
        <span>搜索地址或指纹</span>
      </div>
      <div className="known-host-product-list">
        <KnownHostCard
          endpoint="prod.example.com:22"
          fingerprint="SHA256:W8eQ…sdQ"
          lastUsed="12 分钟前使用"
          tone="violet"
        />
        <KnownHostCard
          endpoint="bastion.example.com:22"
          fingerprint="SHA256:Q2mK…v91"
          lastUsed="12 分钟前使用"
          tone="teal"
        />
        <KnownHostCard
          endpoint="192.168.1.20:22"
          fingerprint="SHA256:7LkP…a82"
          lastUsed="昨天使用"
          tone="amber"
        />
      </div>
      <div className="known-host-actions-note">
        <Icon name="warning" />
        <div>
          <strong>忘记信任后</strong>
          <span>下一次连接会重新显示首次确认；不会自动接受新的密钥。</span>
        </div>
      </div>
      <button
        className="product-text-link"
        onClick={() => navigate(NEXT_SCREEN["known-hosts"] ?? "hosts")}
        type="button"
      >
        返回主机首页
      </button>
      {platform === "android" && <span className="mobile-safe-space" />}
    </div>
  );
}

function KnownHostCard({
  endpoint,
  fingerprint,
  lastUsed,
  tone,
}: {
  endpoint: string;
  fingerprint: string;
  lastUsed: string;
  tone: "violet" | "teal" | "amber";
}) {
  return (
    <div className="known-host-product-card">
      <span className={`known-host-icon known-host-${tone}`}>
        <Icon name="fingerprint" />
      </span>
      <div>
        <strong>{endpoint}</strong>
        <span>ssh-ed25519 · {lastUsed}</span>
        <code>{fingerprint}</code>
      </div>
      <MockButton compact tone="danger">
        忘记信任
      </MockButton>
    </div>
  );
}

function navigationSection(screenId: ScreenId) {
  if (
    [
      "hosts",
      "host-editor",
      "host-detail",
      "host-key",
      "otp",
      "terminal",
    ].includes(screenId)
  ) {
    return "hosts";
  }
  if (["sessions", "forwarding"].includes(screenId)) return "sessions";
  if (["credentials", "credential-editor"].includes(screenId)) {
    return "credentials";
  }
  if (["snippets", "snippet-run"].includes(screenId)) return "snippets";
  return "settings";
}
