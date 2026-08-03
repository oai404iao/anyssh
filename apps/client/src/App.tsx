import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { AppFrame } from "./app/AppFrame";
import { AppSidebar } from "./app/shell/AppSidebar";
import { MobileNavigation } from "./app/shell/MobileNavigation";
import { WorkspaceHeader } from "./app/shell/WorkspaceHeader";
import { useRepositoryWorkspace } from "./app/useRepositoryWorkspace";
import { isConfigurationSection, type WorkspaceView } from "./app/workspace";
import { AppearanceWorkspace } from "./components/AppearanceWorkspace";
import { SnippetWorkspace } from "./components/SnippetWorkspace";
import type { TerminalHandle } from "./components/TerminalPane";
import {
  ConfigurationWorkspace,
  type ConfigurationSection,
} from "./features/configuration/ConfigurationWorkspace";
import { VaultGate } from "./features/vault/VaultGate";
import {
  disconnectSsh,
  isNativeRuntime,
  type HostKeyChangedEvent,
} from "./lib/ssh-bridge";
import { type HostSummary } from "./lib/host-bridge";
import {
  createVault,
  getVaultStatus,
  lockVault,
  unlockVault,
  type VaultStatus,
} from "./lib/vault-bridge";
import {
  createSessionTab,
  INITIAL_CONNECTION_FORM,
  MAX_SESSION_TABS,
  STATUS_LABEL,
  type ConnectionForm,
  type PortForwardForm,
  type SessionTab,
} from "./features/sessions/session-model";
import {
  AuthenticationChallengeDialog,
  ChangedHostKeyDialog,
  HostKeyDialog,
} from "./features/sessions/SessionDialogs";
import { SessionWorkspace } from "./features/sessions/SessionWorkspace";
import { useSessionRuntime } from "./features/sessions/useSessionRuntime";
import { useCompactProductShell } from "./shared/platform/useCompactProductShell";
import "./styles/index.css";

function App() {
  const [initialTab] = useState<SessionTab>(() => createSessionTab());
  const terminalRefs = useRef(new Map<string, TerminalHandle>());
  const sessionTabsRef = useRef<SessionTab[]>([initialTab]);
  const activeTabIdRef = useRef(initialTab.id);
  const [sessionTabs, setSessionTabsState] = useState<SessionTab[]>(() => [
    initialTab,
  ]);
  const [activeTabId, setActiveTabIdState] = useState(initialTab.id);
  const [vaultStatus, setVaultStatus] = useState<VaultStatus | null>(null);
  const [vaultError, setVaultError] = useState<string | null>(null);
  const [workspaceView, setWorkspaceView] = useState<WorkspaceView>("terminal");
  const compactProductShell = useCompactProductShell();

  const replaceSessionTabs = useCallback(
    (update: (current: SessionTab[]) => SessionTab[]) => {
      const next = update(sessionTabsRef.current);
      sessionTabsRef.current = next;
      setSessionTabsState(next);
    },
    [],
  );

  const updateSessionTab = useCallback(
    (
      tabId: string,
      update: (current: SessionTab) => SessionTab,
      expectedGeneration?: number,
    ) => {
      replaceSessionTabs((current) =>
        current.map((tab) =>
          tab.id === tabId &&
          (expectedGeneration === undefined ||
            tab.generation === expectedGeneration)
            ? update(tab)
            : tab,
        ),
      );
    },
    [replaceSessionTabs],
  );

  const setActiveTabId = useCallback((tabId: string) => {
    activeTabIdRef.current = tabId;
    setActiveTabIdState(tabId);
  }, []);

  const activateSessionTab = useCallback(
    (tabId: string) => {
      const previousTabId = activeTabIdRef.current;
      if (previousTabId !== tabId) {
        updateSessionTab(previousTabId, (tab) => ({
          ...tab,
          form: { ...tab.form, password: "" },
          passwordVisible: false,
        }));
      }
      setActiveTabId(tabId);
      setWorkspaceView("terminal");
    },
    [setActiveTabId, updateSessionTab],
  );

  const appendSessionTab = useCallback(
    (tab: SessionTab) => {
      if (sessionTabsRef.current.length >= MAX_SESSION_TABS) {
        updateSessionTab(activeTabIdRef.current, (current) => ({
          ...current,
          error: `AnySSH supports up to ${MAX_SESSION_TABS} open Session Tabs in v1.`,
        }));
        return false;
      }
      const previousTabId = activeTabIdRef.current;
      replaceSessionTabs((current) => [
        ...current.map((currentTab) =>
          currentTab.id === previousTabId
            ? {
                ...currentTab,
                form: { ...currentTab.form, password: "" },
                passwordVisible: false,
              }
            : currentTab,
        ),
        tab,
      ]);
      setActiveTabId(tab.id);
      setWorkspaceView("terminal");
      return true;
    },
    [replaceSessionTabs, setActiveTabId, updateSessionTab],
  );

  const handleRepositoryHostsChanged = useCallback(
    (nextHosts: HostSummary[]) => {
      replaceSessionTabs((current) =>
        current.map((tab) =>
          tab.selectedSavedHostId &&
          !nextHosts.some((host) => host.id === tab.selectedSavedHostId)
            ? {
                ...tab,
                selectedSavedHostId: null,
                title: tab.form.name || "New connection",
              }
            : tab,
        ),
      );
    },
    [replaceSessionTabs],
  );

  const {
    appearance,
    applyAppearance: applyAppearanceSettings,
    clear: clearRepository,
    credentials,
    fontAssets,
    groups,
    hosts,
    knownHosts,
    loadError: repositoryError,
    loading: repositoryLoading,
    refresh: refreshRepository,
    routes,
    snippets,
    systemFonts,
    terminalAppearance,
    terminalThemes,
  } = useRepositoryWorkspace({
    nativeRuntime: isNativeRuntime,
    onHostsChanged: handleRepositoryHostsChanged,
    vaultState: vaultStatus?.state,
  });

  const {
    handleAuthenticationDecision,
    handleConnect,
    handleDisconnect,
    handleHostKeyDecision,
    handleStartPortForward,
    handleStopPortForward,
    handleTerminalInput,
    handleTerminalResize,
    startTabConnection,
  } = useSessionRuntime({
    activateSessionTab,
    activeTabIdRef,
    hosts,
    refreshRepository,
    sessionTabsRef,
    terminalRefs,
    updateSessionTab,
  });

  useEffect(() => {
    if (!isNativeRuntime) return;

    let active = true;
    void getVaultStatus()
      .then((nextStatus) => {
        if (active) setVaultStatus(nextStatus);
      })
      .catch((statusError) => {
        if (!active) return;
        setVaultStatus({
          state: "damaged",
          vaultId: null,
          cipherVersion: null,
        });
        setVaultError(String(statusError));
      });

    return () => {
      active = false;
    };
  }, []);

  const activeTab =
    sessionTabs.find((tab) => tab.id === activeTabId) ?? sessionTabs[0];
  const form = activeTab.form;
  const status = activeTab.status;
  const statusDetail = activeTab.statusDetail;
  const sessionId = activeTab.sessionId;
  const pendingHostKey = activeTab.pendingHostKey;
  const changedHostKey = activeTab.changedHostKey;
  const pendingAuthentication = activeTab.pendingAuthentication;
  const passwordVisible = activeTab.passwordVisible;
  const error = activeTab.error;
  const selectedSavedHostId = activeTab.selectedSavedHostId;
  const portForwardForm = activeTab.portForwardForm;
  const portForwards = activeTab.portForwards;
  const portForwardError = activeTab.portForwardError;
  const portForwardBusy = activeTab.portForwardBusy;

  useEffect(() => {
    if (workspaceView === "terminal") return;
    updateSessionTab(activeTabIdRef.current, (tab) => ({
      ...tab,
      form: { ...tab.form, password: "" },
      passwordVisible: false,
    }));
  }, [updateSessionTab, workspaceView]);

  const setForm = useCallback(
    (
      update: ConnectionForm | ((current: ConnectionForm) => ConnectionForm),
    ) => {
      updateSessionTab(activeTabIdRef.current, (tab) => {
        const nextForm =
          typeof update === "function" ? update(tab.form) : update;
        return {
          ...tab,
          form: nextForm,
          title: tab.selectedSavedHostId
            ? tab.title
            : nextForm.name || "New connection",
        };
      });
    },
    [updateSessionTab],
  );

  const setChangedHostKey = useCallback(
    (changedHostKey: HostKeyChangedEvent | null) => {
      updateSessionTab(activeTabIdRef.current, (tab) => ({
        ...tab,
        changedHostKey,
      }));
    },
    [updateSessionTab],
  );

  const setPortForwardForm = useCallback(
    (
      update: PortForwardForm | ((current: PortForwardForm) => PortForwardForm),
    ) => {
      updateSessionTab(activeTabIdRef.current, (tab) => ({
        ...tab,
        portForwardForm:
          typeof update === "function" ? update(tab.portForwardForm) : update,
      }));
    },
    [updateSessionTab],
  );

  const setPasswordVisible = useCallback(
    (update: boolean | ((current: boolean) => boolean)) => {
      updateSessionTab(activeTabIdRef.current, (tab) => ({
        ...tab,
        passwordVisible:
          typeof update === "function" ? update(tab.passwordVisible) : update,
      }));
    },
    [updateSessionTab],
  );

  const connected = status === "connected";
  const busy = [
    "connecting",
    "verifying",
    "authenticating",
    "authenticated",
  ].includes(status);
  const statusTone = useMemo(() => {
    if (connected) return "success";
    if (status === "error") return "danger";
    if (busy) return "warning";
    return "neutral";
  }, [busy, connected, status]);
  const selectedSavedHost = useMemo(
    () => hosts.find((host) => host.id === selectedSavedHostId) ?? null,
    [hosts, selectedSavedHostId],
  );
  const selectedCredential = useMemo(
    () =>
      selectedSavedHost?.effectiveCredentialId
        ? (credentials.find(
            (credential) =>
              credential.id === selectedSavedHost.effectiveCredentialId,
          ) ?? null)
        : null,
    [credentials, selectedSavedHost],
  );
  const selectedRoute = useMemo(
    () =>
      selectedSavedHost?.effectiveJumpRouteId
        ? (routes.find(
            (route) => route.id === selectedSavedHost.effectiveJumpRouteId,
          ) ?? null)
        : null,
    [routes, selectedSavedHost],
  );

  async function handleVaultSubmit(pin: string) {
    setVaultError(null);
    try {
      const nextStatus =
        vaultStatus?.state === "uninitialized"
          ? await createVault(pin)
          : await unlockVault(pin);
      setVaultStatus(nextStatus);
      await refreshRepository();
      updateSessionTab(activeTabIdRef.current, (tab) => ({
        ...tab,
        status: "idle",
        statusDetail: "Native SSH runtime is available.",
        error: null,
      }));
    } catch (error) {
      setVaultError(String(error));
    }
  }

  async function handleVaultLock() {
    const freshTab = createSessionTab();
    replaceSessionTabs(() => [freshTab]);
    setActiveTabId(freshTab.id);
    terminalRefs.current.clear();
    clearRepository();
    setWorkspaceView("terminal");
    setVaultError(null);

    try {
      setVaultStatus(await lockVault());
    } catch (error) {
      setVaultError(String(error));
    }
  }

  function prepareSavedHost(host: HostSummary): SessionTab | null {
    const current = sessionTabsRef.current.find(
      (tab) => tab.id === activeTabIdRef.current,
    );
    if (
      current &&
      !current.sessionId &&
      current.status === "idle" &&
      current.generation === 0 &&
      current.selectedSavedHostId === null
    ) {
      updateSessionTab(current.id, (tab) => ({
        ...tab,
        title: host.displayName,
        form: {
          ...INITIAL_CONNECTION_FORM,
          name: host.displayName,
          host: host.host,
          port: String(host.port),
        },
        statusDetail: "Ready to connect the saved Host.",
        error: null,
        changedHostKey: null,
        pendingAuthentication: null,
        passwordVisible: false,
        selectedSavedHostId: host.id,
      }));
      activateSessionTab(current.id);
      return (
        sessionTabsRef.current.find((tab) => tab.id === current.id) ?? null
      );
    }
    const tab = createSessionTab({ kind: "savedHost", host });
    return appendSessionTab(tab) ? tab : null;
  }

  function selectSavedHost(host: HostSummary) {
    prepareSavedHost(host);
  }

  async function connectSavedHostFromWorkspace(host: HostSummary) {
    const tab = prepareSavedHost(host);
    if (!tab) return;
    await startTabConnection(tab);
  }

  function useQuickConnection() {
    const current = sessionTabsRef.current.find(
      (tab) => tab.id === activeTabIdRef.current,
    );
    if (current && !current.sessionId && current.status === "idle") {
      const quick = createSessionTab();
      updateSessionTab(current.id, () => ({
        ...quick,
        id: current.id,
      }));
      activateSessionTab(current.id);
      return;
    }
    appendSessionTab(createSessionTab());
  }

  function newQuickSessionTab() {
    appendSessionTab(createSessionTab());
  }

  function handleSessionTabKeyDown(
    event: ReactKeyboardEvent<HTMLButtonElement>,
    tabId: string,
  ) {
    const tabs = sessionTabsRef.current;
    const currentIndex = tabs.findIndex((tab) => tab.id === tabId);
    if (currentIndex < 0) return;

    let nextIndex: number | null = null;
    if (event.key === "ArrowRight") {
      nextIndex = (currentIndex + 1) % tabs.length;
    } else if (event.key === "ArrowLeft") {
      nextIndex = (currentIndex - 1 + tabs.length) % tabs.length;
    } else if (event.key === "Home") {
      nextIndex = 0;
    } else if (event.key === "End") {
      nextIndex = tabs.length - 1;
    }
    if (nextIndex === null) return;

    event.preventDefault();
    const nextTab = tabs[nextIndex];
    activateSessionTab(nextTab.id);
    window.requestAnimationFrame(() => {
      document.getElementById(`session-tab-${nextTab.id}`)?.focus();
    });
  }

  async function closeSessionTab(tabId: string) {
    const currentTabs = sessionTabsRef.current;
    const closingIndex = currentTabs.findIndex((tab) => tab.id === tabId);
    if (closingIndex < 0) return;

    const closingTab = currentTabs[closingIndex];
    let nextTabs = currentTabs.filter((tab) => tab.id !== tabId);
    if (nextTabs.length === 0) {
      nextTabs = [createSessionTab()];
    }

    replaceSessionTabs(() => nextTabs);
    terminalRefs.current.delete(tabId);

    if (activeTabIdRef.current === tabId) {
      const nextActive =
        nextTabs[Math.min(closingIndex, nextTabs.length - 1)] ?? nextTabs[0];
      setActiveTabId(nextActive.id);
    }

    if (closingTab.sessionId) {
      await disconnectSsh(closingTab.sessionId).catch(() => undefined);
    }
  }

  const configurationTitle: Record<ConfigurationSection, string> = {
    groups: "Groups",
    hosts: "Hosts",
    credentials: "Credentials",
    routes: "Jump Routes",
    knownHosts: "Known Hosts",
  };
  const workspaceTitle =
    workspaceView === "terminal"
      ? selectedSavedHost?.displayName || form.name || "New connection"
      : workspaceView === "appearance"
        ? "Appearance"
        : workspaceView === "snippets"
          ? "Snippets"
          : configurationTitle[workspaceView];
  const navigationCounts = {
    credentials: credentials.length,
    groups: groups.length,
    hosts: hosts.length,
    knownHosts: knownHosts.length,
    routes: routes.length,
    sessions: sessionTabs.length,
    snippets: snippets.length,
  };

  if (isNativeRuntime && vaultStatus?.state !== "unlocked") {
    return (
      <AppFrame workspaceTitle="Vault">
        <VaultGate
          error={vaultError}
          onClearError={() => setVaultError(null)}
          onSubmit={handleVaultSubmit}
          status={vaultStatus}
        />
      </AppFrame>
    );
  }

  return (
    <AppFrame workspaceTitle={workspaceTitle}>
      <main
        className={`app-shell ${
          compactProductShell ? "compact-product-shell" : ""
        }`}
      >
        <AppSidebar
          counts={navigationCounts}
          hosts={hosts}
          loading={repositoryLoading}
          nativeRuntime={isNativeRuntime}
          onNavigate={setWorkspaceView}
          onSelectHost={selectSavedHost}
          selectedHostId={selectedSavedHostId}
          vaultCipherVersion={vaultStatus?.cipherVersion}
          workspaceView={workspaceView}
        />

        <section className={`workspace workspace-${workspaceView}`}>
          <WorkspaceHeader
            connected={Boolean(sessionId && connected)}
            nativeRuntime={isNativeRuntime}
            onDisconnect={() => void handleDisconnect()}
            onLockVault={() => void handleVaultLock()}
            statusLabel={STATUS_LABEL[status]}
            statusTone={statusTone}
            title={workspaceTitle}
            workspaceView={workspaceView}
          />

          <SessionWorkspace
            activeStatus={status}
            compactProductShell={compactProductShell}
            connected={connected}
            connectionPanelProps={{
              busy,
              connected,
              error,
              form,
              nativeRuntime: isNativeRuntime,
              onConnect: handleConnect,
              onFormChange: setForm,
              onPasswordVisibleChange: setPasswordVisible,
              onPortForwardFormChange: setPortForwardForm,
              onStartPortForward: handleStartPortForward,
              onStopPortForward: handleStopPortForward,
              onUseQuickConnection: useQuickConnection,
              passwordVisible,
              portForwardBusy,
              portForwardError,
              portForwardForm,
              portForwards,
              selectedCredential,
              selectedRoute,
              selectedSavedHost,
              statusDetail,
              statusLabel: STATUS_LABEL[status],
              statusTone,
            }}
            onDisconnect={() => void handleDisconnect()}
            onNavigate={setWorkspaceView}
            onTerminalInput={handleTerminalInput}
            statusLabel={STATUS_LABEL[status]}
            statusTone={statusTone}
            terminalProps={{
              activeTabId: activeTab.id,
              appearance: terminalAppearance,
              maxTabs: MAX_SESSION_TABS,
              onActivate: activateSessionTab,
              onClose: closeSessionTab,
              onNew: newQuickSessionTab,
              onResize: handleTerminalResize,
              onTabKeyDown: handleSessionTabKeyDown,
              tabs: sessionTabs,
              terminalHost: selectedSavedHost?.host || form.host,
              terminalRefs,
              terminalUsername: selectedCredential?.username || form.username,
            }}
            workspaceVisible={workspaceView === "terminal"}
          />
          {isConfigurationSection(workspaceView) && (
            <ConfigurationWorkspace
              credentials={credentials}
              groups={groups}
              hosts={hosts}
              knownHosts={knownHosts}
              loadError={repositoryError}
              loading={repositoryLoading}
              nativeRuntime={isNativeRuntime}
              onChanged={refreshRepository}
              onConnectHost={connectSavedHostFromWorkspace}
              onOpenHost={selectSavedHost}
              routes={routes}
              section={workspaceView}
            />
          )}
          {workspaceView === "appearance" && (
            <AppearanceWorkspace
              fonts={fontAssets}
              key={JSON.stringify(appearance)}
              loadError={repositoryError}
              loading={repositoryLoading}
              onChanged={refreshRepository}
              onUpdate={applyAppearanceSettings}
              settings={appearance}
              systemFonts={systemFonts}
              themes={terminalThemes}
            />
          )}
          {workspaceView === "snippets" && (
            <SnippetWorkspace
              activeSessionId={connected ? sessionId : null}
              activeSessionTitle={
                selectedSavedHost?.displayName || form.name || "Current Session"
              }
              loadError={repositoryError}
              loading={repositoryLoading}
              onChanged={refreshRepository}
              snippets={snippets}
            />
          )}
          {workspaceView !== "terminal" && (
            <MobileNavigation
              counts={navigationCounts}
              nativeRuntime={isNativeRuntime}
              onLockVault={() => void handleVaultLock()}
              onNavigate={setWorkspaceView}
              workspaceView={workspaceView}
            />
          )}
        </section>

        {pendingHostKey && (
          <HostKeyDialog
            event={pendingHostKey}
            onDecision={handleHostKeyDecision}
          />
        )}
        {pendingAuthentication && (
          <AuthenticationChallengeDialog
            challenge={pendingAuthentication}
            key={`${sessionId ?? "pending"}-${pendingAuthentication.requestId}`}
            onDecision={handleAuthenticationDecision}
          />
        )}
        {changedHostKey && (
          <ChangedHostKeyDialog
            event={changedHostKey}
            onClose={() => setChangedHostKey(null)}
            onOpenKnownHosts={() => {
              setChangedHostKey(null);
              setWorkspaceView("knownHosts");
              void refreshRepository();
            }}
          />
        )}
      </main>
    </AppFrame>
  );
}

export default App;
