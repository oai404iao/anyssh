import type {
  KeyboardEvent as ReactKeyboardEvent,
  MutableRefObject,
} from "react";
import {
  TerminalPane,
  type TerminalAppearance,
  type TerminalHandle,
} from "../../components/TerminalPane";
import { LockIcon } from "../../shared/icons/ProductIcons";
import type { SessionTab } from "./session-model";

export interface SessionTerminalProps {
  activeTabId: string;
  appearance: TerminalAppearance;
  maxTabs: number;
  onActivate(tabId: string): void;
  onClose(tabId: string): Promise<void>;
  onDisconnect(): void;
  onExitMobileTerminal(): void;
  onInput(tabId: string, input: string): void;
  onNew(): void;
  onResize(tabId: string, columns: number, rows: number): void;
  onTabKeyDown(event: ReactKeyboardEvent, tabId: string): void;
  tabs: SessionTab[];
  terminalHost: string;
  terminalRefs: MutableRefObject<Map<string, TerminalHandle>>;
  terminalUsername: string;
  statusLabel: string;
  statusTone: string;
  workspaceVisible: boolean;
}

export function SessionTerminal({
  activeTabId,
  appearance,
  maxTabs,
  onActivate,
  onClose,
  onDisconnect,
  onExitMobileTerminal,
  onInput,
  onNew,
  onResize,
  onTabKeyDown,
  tabs,
  terminalHost,
  terminalRefs,
  terminalUsername,
  statusLabel,
  statusTone,
  workspaceVisible,
}: SessionTerminalProps) {
  return (
    <section className="terminal-card" aria-label="SSH terminal">
      <div className="session-tab-strip">
        <div
          aria-label="SSH sessions"
          className="session-tab-list"
          role="tablist"
        >
          {tabs.map((tab) => {
            const tabConnected = tab.status === "connected";
            const tabPending =
              tab.pendingHostKey !== null || tab.pendingAuthentication !== null;
            return (
              <div
                className={`session-tab ${
                  tab.id === activeTabId ? "active" : ""
                }`}
                key={tab.id}
              >
                <button
                  aria-controls={`session-panel-${tab.id}`}
                  aria-selected={tab.id === activeTabId}
                  className="session-tab-activate"
                  id={`session-tab-${tab.id}`}
                  onClick={() => onActivate(tab.id)}
                  onKeyDown={(event) => onTabKeyDown(event, tab.id)}
                  role="tab"
                  tabIndex={tab.id === activeTabId ? 0 : -1}
                  type="button"
                >
                  <span
                    className={`session-tab-status ${
                      tabConnected
                        ? "connected"
                        : tab.status === "error"
                          ? "error"
                          : tabPending
                            ? "pending"
                            : ""
                    }`}
                  />
                  <span className="session-tab-title">{tab.title}</span>
                  {tabPending && (
                    <span className="session-tab-pending">Action</span>
                  )}
                </button>
                <button
                  aria-label={`Close ${tab.title} session tab`}
                  className="session-tab-close"
                  onClick={() => void onClose(tab.id)}
                  title={
                    tab.sessionId
                      ? "Disconnect and close session"
                      : "Close session tab"
                  }
                  type="button"
                >
                  ×
                </button>
              </div>
            );
          })}
        </div>
        <button
          aria-label="New session tab"
          className="new-session-tab"
          disabled={tabs.length >= maxTabs}
          onClick={onNew}
          type="button"
        >
          +
        </button>
      </div>
      <div className="terminal-toolbar">
        <button
          aria-label="Back to Hosts"
          className="mobile-terminal-back"
          onClick={onExitMobileTerminal}
          type="button"
        >
          ←
        </button>
        <div className="terminal-title">
          <span>{terminalUsername || "user"}@</span>
          {terminalHost || "host"}
        </div>
        <div className="terminal-toolbar-meta">
          <span className={`terminal-session-status ${statusTone}`}>
            <i />
            {statusLabel}
          </span>
          <span className="terminal-security">
            <LockIcon />
            Host key verification
          </span>
        </div>
        <button
          aria-label="Disconnect active session"
          className="mobile-terminal-disconnect"
          disabled={statusTone !== "success"}
          onClick={onDisconnect}
          type="button"
        >
          ⏻
        </button>
      </div>
      <div className="terminal-tab-panels">
        {tabs.map((tab) => {
          const visible = workspaceVisible && tab.id === activeTabId;
          return (
            <div
              aria-labelledby={`session-tab-${tab.id}`}
              className="terminal-tab-panel"
              hidden={!visible}
              id={`session-panel-${tab.id}`}
              key={tab.id}
              role="tabpanel"
            >
              <TerminalPane
                appearance={appearance}
                onInput={(input) => onInput(tab.id, input)}
                onResize={(columns, rows) => onResize(tab.id, columns, rows)}
                ref={(handle) => {
                  if (handle) {
                    terminalRefs.current.set(tab.id, handle);
                  } else {
                    terminalRefs.current.delete(tab.id);
                  }
                }}
                visible={visible}
              />
            </div>
          );
        })}
      </div>
    </section>
  );
}
