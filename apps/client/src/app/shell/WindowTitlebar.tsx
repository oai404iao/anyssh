import { isNativeRuntime } from "../../lib/ssh-bridge";

interface WindowTitlebarProps {
  workspaceTitle: string;
}

type WindowAction = "close" | "minimize" | "toggleMaximize";

async function runWindowAction(action: WindowAction): Promise<void> {
  if (!isNativeRuntime) return;

  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const window = getCurrentWindow();
  await window[action]();
}

export function WindowTitlebar({ workspaceTitle }: WindowTitlebarProps) {
  return (
    <header
      className="window-titlebar"
      data-tauri-drag-region
      onDoubleClick={() => void runWindowAction("toggleMaximize")}
    >
      <div className="window-titlebar-brand" data-tauri-drag-region>
        <span className="window-titlebar-mark" aria-hidden="true">
          <span />
        </span>
        <span className="window-titlebar-product" data-tauri-drag-region>
          AnySSH
        </span>
        <span className="window-titlebar-divider" aria-hidden="true" />
        <span className="window-titlebar-workspace" data-tauri-drag-region>
          {workspaceTitle}
        </span>
      </div>

      <div
        className="window-titlebar-actions"
        onDoubleClick={(event) => event.stopPropagation()}
      >
        <button
          aria-label="Minimize window"
          className="window-control"
          onClick={() => void runWindowAction("minimize")}
          type="button"
        >
          <svg aria-hidden="true" viewBox="0 0 20 20">
            <path d="M5 10.5h10" />
          </svg>
        </button>
        <button
          aria-label="Maximize or restore window"
          className="window-control"
          onClick={() => void runWindowAction("toggleMaximize")}
          type="button"
        >
          <svg aria-hidden="true" viewBox="0 0 20 20">
            <rect height="9" rx="1" width="9" x="5.5" y="5.5" />
          </svg>
        </button>
        <button
          aria-label="Close window"
          className="window-control window-control-close"
          onClick={() => void runWindowAction("close")}
          type="button"
        >
          <svg aria-hidden="true" viewBox="0 0 20 20">
            <path d="m6 6 8 8m0-8-8 8" />
          </svg>
        </button>
      </div>
    </header>
  );
}
