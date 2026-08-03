import type { WorkspaceView } from "../workspace";
import type { HostSummary } from "../../lib/host-bridge";
import { NavigationIcon } from "../../shared/icons/ProductIcons";

interface SidebarCounts {
  credentials: number;
  groups: number;
  hosts: number;
  knownHosts: number;
  routes: number;
  sessions: number;
  snippets: number;
}

interface AppSidebarProps {
  counts: SidebarCounts;
  hosts: HostSummary[];
  loading: boolean;
  nativeRuntime: boolean;
  onNavigate(view: WorkspaceView): void;
  onSelectHost(host: HostSummary): void;
  selectedHostId: string | null;
  vaultCipherVersion: number | string | null | undefined;
  workspaceView: WorkspaceView;
}

const NAVIGATION_ITEMS = [
  { view: "terminal", label: "Terminal", icon: "terminal", count: "sessions" },
  { view: "groups", label: "Groups", icon: "groups", count: "groups" },
  { view: "hosts", label: "Hosts", icon: "hosts", count: "hosts" },
  {
    view: "credentials",
    label: "Credentials",
    icon: "keys",
    count: "credentials",
  },
  {
    view: "routes",
    label: "Jump routes",
    icon: "routes",
    count: "routes",
  },
  {
    view: "knownHosts",
    label: "Known hosts",
    icon: "knownHosts",
    count: "knownHosts",
  },
  {
    view: "snippets",
    label: "Snippets",
    icon: "snippets",
    count: "snippets",
  },
] as const;

export function AppSidebar({
  counts,
  hosts,
  loading,
  nativeRuntime,
  onNavigate,
  onSelectHost,
  selectedHostId,
  vaultCipherVersion,
  workspaceView,
}: AppSidebarProps) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark" aria-hidden="true">
          <span />
        </div>
        <div>
          <strong>AnySSH</strong>
          <small>Linux + Android</small>
        </div>
      </div>

      <nav className="primary-nav" aria-label="Primary">
        {NAVIGATION_ITEMS.map((item) => (
          <button
            className={`nav-item ${
              workspaceView === item.view ? "active" : ""
            }`}
            key={item.view}
            onClick={() => onNavigate(item.view)}
            type="button"
          >
            <NavigationIcon name={item.icon} />
            {item.label}
            <span className="nav-count">{counts[item.count]}</span>
          </button>
        ))}
        <button
          className={`nav-item ${
            workspaceView === "appearance" ? "active" : ""
          }`}
          onClick={() => onNavigate("appearance")}
          type="button"
        >
          <NavigationIcon name="appearance" />
          Appearance
          <span className="coming-soon">Aa</span>
        </button>
      </nav>

      <div className="section-heading">
        <span>Saved hosts</span>
        <button
          aria-label="Manage Hosts"
          onClick={() => onNavigate("hosts")}
          type="button"
        >
          +
        </button>
      </div>

      <div className="host-list">
        {hosts.map((host, index) => (
          <button
            className={`host-card ${
              selectedHostId === host.id ? "selected" : ""
            }`}
            key={host.id}
            onClick={() => onSelectHost(host)}
            type="button"
          >
            <span
              className={`host-avatar ${
                ["cyan", "violet", "amber"][index % 3]
              }`}
            >
              {host.displayName.slice(0, 2)}
            </span>
            <span>
              <strong>{host.displayName}</strong>
              <small>
                {host.host}:{host.port}
              </small>
            </span>
            {host.host === "127.0.0.1" && host.port === 2222 && (
              <span className="online-dot" title="Fixture available" />
            )}
          </button>
        ))}
        {!loading && hosts.length === 0 && (
          <button
            className="empty-host-list"
            onClick={() => onNavigate("hosts")}
            type="button"
          >
            Add your first Host
          </button>
        )}
      </div>

      <div className="sidebar-footer">
        <span
          className={`runtime-dot ${nativeRuntime ? "native" : "preview"}`}
        />
        <div>
          <strong>
            {nativeRuntime ? "Native runtime" : "Browser QA mode"}
          </strong>
          <small>
            {nativeRuntime
              ? vaultCipherVersion
                ? `SQLCipher ${vaultCipherVersion}`
                : "Rust core ready"
              : "No network connections"}
          </small>
        </div>
      </div>
    </aside>
  );
}
