import { useState } from "react";
import { NavigationIcon } from "../../shared/icons/ProductIcons";
import type { WorkspaceView } from "../workspace";

interface MobileNavigationCounts {
  credentials: number;
  groups: number;
  hosts: number;
  knownHosts: number;
  routes: number;
  sessions: number;
  snippets: number;
}

interface MobileNavigationProps {
  counts: MobileNavigationCounts;
  nativeRuntime: boolean;
  onLockVault(): void;
  onNavigate(view: WorkspaceView): void;
  workspaceView: WorkspaceView;
}

const PRIMARY_ITEMS = [
  { view: "hosts", label: "Hosts", icon: "hosts", count: "hosts" },
  {
    view: "terminal",
    label: "Sessions",
    icon: "terminal",
    count: "sessions",
  },
  {
    view: "credentials",
    label: "Credentials",
    icon: "keys",
    count: "credentials",
  },
  {
    view: "snippets",
    label: "Snippets",
    icon: "snippets",
    count: "snippets",
  },
] as const;

const MORE_ITEMS = [
  { view: "groups", label: "Groups", icon: "groups", count: "groups" },
  { view: "routes", label: "Jump routes", icon: "routes", count: "routes" },
  {
    view: "knownHosts",
    label: "Known hosts",
    icon: "knownHosts",
    count: "knownHosts",
  },
  {
    view: "appearance",
    label: "Appearance",
    icon: "appearance",
    count: null,
  },
] as const;

export function MobileNavigation({
  counts,
  nativeRuntime,
  onLockVault,
  onNavigate,
  workspaceView,
}: MobileNavigationProps) {
  const [moreOpen, setMoreOpen] = useState(false);
  const moreActive = MORE_ITEMS.some((item) => item.view === workspaceView);

  const navigate = (view: WorkspaceView) => {
    setMoreOpen(false);
    onNavigate(view);
  };

  return (
    <>
      {moreOpen && (
        <>
          <button
            aria-label="Close more navigation"
            className="mobile-navigation-scrim"
            onClick={() => setMoreOpen(false)}
            type="button"
          />
          <section
            aria-label="More workspace navigation"
            className="mobile-navigation-sheet"
          >
            <header>
              <div>
                <p className="eyebrow">Workspace</p>
                <h2>More</h2>
              </div>
              <button
                aria-label="Close more navigation"
                onClick={() => setMoreOpen(false)}
                type="button"
              >
                ×
              </button>
            </header>
            <div className="mobile-navigation-sheet-grid">
              {MORE_ITEMS.map((item) => (
                <button
                  className={workspaceView === item.view ? "active" : ""}
                  key={item.view}
                  onClick={() => navigate(item.view)}
                  type="button"
                >
                  <NavigationIcon name={item.icon} />
                  <span>{item.label}</span>
                  {item.count && <strong>{counts[item.count]}</strong>}
                </button>
              ))}
            </div>
            {nativeRuntime && (
              <button
                className="mobile-lock-vault"
                onClick={() => {
                  setMoreOpen(false);
                  onLockVault();
                }}
                type="button"
              >
                Lock Vault
              </button>
            )}
          </section>
        </>
      )}

      <nav className="mobile-primary-nav" aria-label="Mobile workspace">
        {PRIMARY_ITEMS.map((item) => (
          <button
            className={workspaceView === item.view ? "active" : ""}
            key={item.view}
            onClick={() => navigate(item.view)}
            type="button"
          >
            <span className="mobile-nav-icon">
              <NavigationIcon name={item.icon} />
              <strong>{counts[item.count]}</strong>
            </span>
            {item.label}
          </button>
        ))}
        <button
          aria-expanded={moreOpen}
          className={moreActive || moreOpen ? "active" : ""}
          onClick={() => setMoreOpen((current) => !current)}
          type="button"
        >
          <span className="mobile-nav-icon">
            <NavigationIcon name="appearance" />
          </span>
          More
        </button>
      </nav>
    </>
  );
}
