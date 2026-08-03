import type { WorkspaceView } from "../workspace";

interface WorkspaceHeaderProps {
  connected: boolean;
  nativeRuntime: boolean;
  onDisconnect(): void;
  onLockVault(): void;
  statusLabel: string;
  statusTone: string;
  title: string;
  workspaceView: WorkspaceView;
}

export function WorkspaceHeader({
  connected,
  nativeRuntime,
  onDisconnect,
  onLockVault,
  statusLabel,
  statusTone,
  title,
  workspaceView,
}: WorkspaceHeaderProps) {
  return (
    <header className="workspace-header">
      <div>
        <p className="eyebrow">
          {workspaceView === "terminal"
            ? "SSH workspace"
            : "Vault configuration"}
        </p>
        <h1>{title}</h1>
      </div>
      <div className="header-actions">
        <div className={`status-pill ${statusTone}`} aria-live="polite">
          <span />
          {statusLabel}
        </div>
        {connected && (
          <button
            className="secondary-button"
            onClick={onDisconnect}
            type="button"
          >
            Disconnect
          </button>
        )}
        {nativeRuntime && (
          <button
            className="secondary-button"
            onClick={onLockVault}
            type="button"
          >
            Lock Vault
          </button>
        )}
      </div>
    </header>
  );
}
