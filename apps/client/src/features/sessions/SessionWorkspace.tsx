import { useCallback, useState } from "react";
import type { WorkspaceView } from "../../app/workspace";
import { ConnectionPanel, type ConnectionPanelProps } from "./ConnectionPanel";
import {
  MobileTerminalActions,
  TerminalAuxiliaryKeyboard,
} from "./TerminalMobileControls";
import { SessionTerminal, type SessionTerminalProps } from "./SessionTerminal";
import type { SessionTab } from "./session-model";
import {
  applyTerminalModifiers,
  terminalAuxiliaryInput,
  type TerminalAuxiliaryKey,
  type TerminalModifiers,
} from "./terminal-input";

type MobilePanel = "connection" | "terminal";

interface SessionWorkspaceProps {
  activeStatus: SessionTab["status"];
  compactProductShell: boolean;
  connected: boolean;
  connectionPanelProps: ConnectionPanelProps;
  onDisconnect(): void;
  onNavigate(view: WorkspaceView): void;
  onTerminalInput(tabId: string, input: string): void;
  terminalProps: Omit<
    SessionTerminalProps,
    | "onExitMobileTerminal"
    | "onDisconnect"
    | "onInput"
    | "statusLabel"
    | "statusTone"
    | "workspaceVisible"
  >;
  statusLabel: string;
  statusTone: string;
  workspaceVisible: boolean;
}

const EMPTY_MODIFIERS: TerminalModifiers = {
  alt: false,
  control: false,
};

export function SessionWorkspace({
  activeStatus,
  compactProductShell,
  connected,
  connectionPanelProps,
  onDisconnect,
  onNavigate,
  onTerminalInput,
  statusLabel,
  statusTone,
  terminalProps,
  workspaceVisible,
}: SessionWorkspaceProps) {
  const [panelSelection, setPanelSelection] = useState<{
    panel: MobilePanel;
    tabId: string;
  } | null>(null);
  const activeTab = terminalProps.tabs.find(
    (tab) => tab.id === terminalProps.activeTabId,
  );
  const modifierScope = `${terminalProps.activeTabId}:${
    activeTab?.generation ?? 0
  }`;
  const [modifierState, setModifierState] = useState<{
    modifiers: TerminalModifiers;
    scope: string;
  }>({
    modifiers: EMPTY_MODIFIERS,
    scope: modifierScope,
  });
  const modifiers =
    connected && modifierState.scope === modifierScope
      ? modifierState.modifiers
      : EMPTY_MODIFIERS;
  const mobilePanel =
    panelSelection?.tabId === terminalProps.activeTabId
      ? panelSelection.panel
      : activeStatus === "idle"
        ? "connection"
        : "terminal";

  const selectMobilePanel = useCallback(
    (panel: MobilePanel) => {
      setPanelSelection({ panel, tabId: terminalProps.activeTabId });
    },
    [terminalProps.activeTabId],
  );

  const clearModifiers = useCallback(() => {
    setModifierState({
      modifiers: EMPTY_MODIFIERS,
      scope: modifierScope,
    });
  }, [modifierScope]);

  const focusTerminal = useCallback(() => {
    selectMobilePanel("terminal");
    window.requestAnimationFrame(() => {
      terminalProps.terminalRefs.current
        .get(terminalProps.activeTabId)
        ?.focus();
    });
  }, [
    selectMobilePanel,
    terminalProps.activeTabId,
    terminalProps.terminalRefs,
  ]);

  const handleTerminalInput = useCallback(
    (tabId: string, input: string) => {
      onTerminalInput(tabId, applyTerminalModifiers(input, modifiers));
      if (input.length > 0 && (modifiers.alt || modifiers.control)) {
        clearModifiers();
      }
    },
    [clearModifiers, modifiers, onTerminalInput],
  );

  const sendAuxiliaryKey = useCallback(
    (key: TerminalAuxiliaryKey) => {
      if (!connected) return;
      onTerminalInput(
        terminalProps.activeTabId,
        terminalAuxiliaryInput(key, modifiers),
      );
      clearModifiers();
      window.requestAnimationFrame(() => {
        terminalProps.terminalRefs.current
          .get(terminalProps.activeTabId)
          ?.focus();
      });
    },
    [
      connected,
      clearModifiers,
      modifiers,
      onTerminalInput,
      terminalProps.activeTabId,
      terminalProps.terminalRefs,
    ],
  );

  const showForwarding = useCallback(() => {
    selectMobilePanel("connection");
    window.requestAnimationFrame(() => {
      document
        .getElementById("port-forwarding-panel")
        ?.scrollIntoView({ block: "start" });
    });
  }, [selectMobilePanel]);

  const showSessions = useCallback(() => {
    selectMobilePanel("terminal");
    window.requestAnimationFrame(() => {
      document
        .querySelector(".session-tab-strip")
        ?.scrollIntoView({ block: "start" });
    });
  }, [selectMobilePanel]);

  const navigate = useCallback(
    (view: WorkspaceView) => {
      clearModifiers();
      onNavigate(view);
    },
    [clearModifiers, onNavigate],
  );

  const terminalPanelVisible =
    !compactProductShell || mobilePanel === "terminal";
  const connectionPanelVisible =
    !compactProductShell || mobilePanel === "connection";

  return (
    <div
      aria-hidden={!workspaceVisible}
      className={`workspace-body session-workspace ${
        workspaceVisible ? "" : "workspace-body-hidden"
      }`}
      inert={!workspaceVisible}
    >
      <div
        className="session-terminal-pane"
        hidden={!terminalPanelVisible}
        inert={!terminalPanelVisible}
      >
        <SessionTerminal
          {...terminalProps}
          onActivate={(tabId) => {
            clearModifiers();
            setPanelSelection(null);
            terminalProps.onActivate(tabId);
          }}
          onClose={async (tabId) => {
            clearModifiers();
            setPanelSelection(null);
            await terminalProps.onClose(tabId);
          }}
          onDisconnect={() => {
            clearModifiers();
            onDisconnect();
          }}
          onExitMobileTerminal={() => navigate("hosts")}
          onInput={handleTerminalInput}
          onNew={() => {
            clearModifiers();
            setPanelSelection(null);
            terminalProps.onNew();
          }}
          statusLabel={statusLabel}
          statusTone={statusTone}
          workspaceVisible={workspaceVisible && terminalPanelVisible}
        />
      </div>

      {workspaceVisible && (
        <div
          className="session-connection-pane"
          hidden={!connectionPanelVisible}
          inert={!connectionPanelVisible}
        >
          <ConnectionPanel
            {...connectionPanelProps}
            onConnect={(event) => {
              setPanelSelection(null);
              connectionPanelProps.onConnect(event);
            }}
            onUseQuickConnection={() => {
              clearModifiers();
              setPanelSelection(null);
              connectionPanelProps.onUseQuickConnection();
            }}
          />
        </div>
      )}

      {compactProductShell && workspaceVisible && terminalPanelVisible && (
        <TerminalAuxiliaryKeyboard
          connected={connected}
          modifiers={modifiers}
          onSend={sendAuxiliaryKey}
          onToggleModifier={(modifier) => {
            setModifierState((current) => {
              const currentModifiers =
                current.scope === modifierScope
                  ? current.modifiers
                  : EMPTY_MODIFIERS;
              return {
                modifiers: {
                  ...currentModifiers,
                  [modifier]: !currentModifiers[modifier],
                },
                scope: modifierScope,
              };
            });
            window.requestAnimationFrame(() => {
              terminalProps.terminalRefs.current
                .get(terminalProps.activeTabId)
                ?.focus();
            });
          }}
        />
      )}

      {compactProductShell && workspaceVisible && (
        <MobileTerminalActions
          connectionPanelVisible={connectionPanelVisible}
          onFocusKeyboard={focusTerminal}
          onOpenForwarding={showForwarding}
          onOpenSessions={showSessions}
          onOpenSnippets={() => navigate("snippets")}
        />
      )}
    </div>
  );
}
