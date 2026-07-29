import {
  FormEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import {
  ConfigurationWorkspace,
  type ConfigurationSection,
} from "./components/ConfigurationWorkspace";
import { TerminalPane, type TerminalHandle } from "./components/TerminalPane";
import { VaultGate } from "./components/VaultGate";
import {
  confirmHostKey,
  connectSavedHost,
  connectSsh,
  disconnectSsh,
  isNativeRuntime,
  respondAuthentication,
  resizeSsh,
  sendSshInput,
  startSshPortForward,
  stopSshPortForward,
  type AuthenticationChallengeEvent,
  type HostKeyChangedEvent,
  type HostKeyEvent,
  type SshClientEvent,
  type SshPortForwardKind,
  type SshPortForwardSummary,
} from "./lib/ssh-bridge";
import {
  listCredentials,
  type CredentialSummary,
} from "./lib/credential-bridge";
import {
  listGroups,
  listHosts,
  listJumpRoutes,
  type GroupSummary,
  type HostSummary,
  type JumpRouteSummary,
} from "./lib/host-bridge";
import { listKnownHosts, type KnownHostSummary } from "./lib/known-host-bridge";
import {
  createVault,
  getVaultStatus,
  lockVault,
  unlockVault,
  type VaultStatus,
} from "./lib/vault-bridge";
import "./App.css";

type ConnectionStatus =
  | "idle"
  | "connecting"
  | "verifying"
  | "authenticating"
  | "authenticated"
  | "connected"
  | "error"
  | "closed";

interface ConnectionForm {
  name: string;
  host: string;
  port: string;
  username: string;
  authenticationKind: "password" | "keyboardInteractive";
  password: string;
}

interface PortForwardForm {
  kind: SshPortForwardKind;
  bindHost: string;
  bindPort: string;
  destinationHost: string;
  destinationPort: string;
}

type WorkspaceView = "terminal" | ConfigurationSection;

interface SessionTab {
  id: string;
  generation: number;
  title: string;
  form: ConnectionForm;
  status: ConnectionStatus;
  statusDetail: string;
  sessionId: string | null;
  pendingHostKey: HostKeyEvent | null;
  changedHostKey: HostKeyChangedEvent | null;
  pendingAuthentication: AuthenticationChallengeEvent | null;
  passwordVisible: boolean;
  error: string | null;
  selectedSavedHostId: string | null;
  terminalSize: { columns: number; rows: number };
  portForwardForm: PortForwardForm;
  portForwards: SshPortForwardSummary[];
  portForwardError: string | null;
  portForwardBusy: boolean;
}

const INITIAL_FORM: ConnectionForm = {
  name: "Local lab",
  host: "127.0.0.1",
  port: "2222",
  username: "anyssh",
  authenticationKind: "password",
  password: "",
};

const INITIAL_PORT_FORWARD_FORM: PortForwardForm = {
  kind: "local",
  bindHost: "127.0.0.1",
  bindPort: "0",
  destinationHost: "127.0.0.1",
  destinationPort: "8080",
};

const MAX_SESSION_TABS = 8;

const STATUS_LABEL: Record<ConnectionStatus, string> = {
  idle: "Ready",
  connecting: "Connecting",
  verifying: "Verify host",
  authenticating: "Authentication required",
  authenticated: "Authenticated",
  connected: "Connected",
  error: "Connection failed",
  closed: "Disconnected",
};

let nextSessionTabId = 1;

function formatForwardEndpoint(host: string, port: number): string {
  return `${host.includes(":") ? `[${host}]` : host}:${port}`;
}

function createSessionTab(
  source:
    | { kind: "quick" }
    | {
        kind: "savedHost";
        host: HostSummary;
      } = { kind: "quick" },
): SessionTab {
  const id = `session-tab-${nextSessionTabId++}`;
  if (source.kind === "savedHost") {
    return {
      id,
      generation: 0,
      title: source.host.displayName,
      form: {
        ...INITIAL_FORM,
        name: source.host.displayName,
        host: source.host.host,
        port: String(source.host.port),
      },
      status: "idle",
      statusDetail: "Ready to connect the saved Host.",
      sessionId: null,
      pendingHostKey: null,
      changedHostKey: null,
      pendingAuthentication: null,
      passwordVisible: false,
      error: null,
      selectedSavedHostId: source.host.id,
      terminalSize: { columns: 120, rows: 32 },
      portForwardForm: { ...INITIAL_PORT_FORWARD_FORM },
      portForwards: [],
      portForwardError: null,
      portForwardBusy: false,
    };
  }

  return {
    id,
    generation: 0,
    title: INITIAL_FORM.name,
    form: { ...INITIAL_FORM },
    status: "idle",
    statusDetail: isNativeRuntime
      ? "Native SSH runtime is available."
      : "Browser QA mode uses a local terminal simulation.",
    sessionId: null,
    pendingHostKey: null,
    changedHostKey: null,
    pendingAuthentication: null,
    passwordVisible: false,
    error: null,
    selectedSavedHostId: null,
    terminalSize: { columns: 120, rows: 32 },
    portForwardForm: { ...INITIAL_PORT_FORWARD_FORM },
    portForwards: [],
    portForwardError: null,
    portForwardBusy: false,
  };
}

function App() {
  const [initialTab] = useState<SessionTab>(() => createSessionTab());
  const appMountedRef = useRef(false);
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
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [groups, setGroups] = useState<GroupSummary[]>([]);
  const [hosts, setHosts] = useState<HostSummary[]>([]);
  const [routes, setRoutes] = useState<JumpRouteSummary[]>([]);
  const [knownHosts, setKnownHosts] = useState<KnownHostSummary[]>([]);
  const [repositoryLoading, setRepositoryLoading] = useState(false);
  const [repositoryError, setRepositoryError] = useState<string | null>(null);

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
      setRepositoryError(null);
      return true;
    },
    [replaceSessionTabs, setActiveTabId, updateSessionTab],
  );

  const refreshRepository = useCallback(async () => {
    setRepositoryLoading(true);
    setRepositoryError(null);
    try {
      const [
        nextCredentials,
        nextGroups,
        nextHosts,
        nextRoutes,
        nextKnownHosts,
      ] = await Promise.all([
        listCredentials(),
        listGroups(),
        listHosts(),
        listJumpRoutes(),
        listKnownHosts(),
      ]);
      setCredentials(nextCredentials);
      setGroups(nextGroups);
      setHosts(nextHosts);
      setRoutes(nextRoutes);
      setKnownHosts(nextKnownHosts);
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
    } catch (loadError) {
      setRepositoryError(String(loadError));
    } finally {
      setRepositoryLoading(false);
    }
  }, [replaceSessionTabs]);

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

  useEffect(() => {
    if (isNativeRuntime && vaultStatus?.state !== "unlocked") return;
    const refreshTimer = window.setTimeout(() => {
      void refreshRepository();
    }, 0);
    return () => window.clearTimeout(refreshTimer);
  }, [refreshRepository, vaultStatus?.state]);

  useEffect(() => {
    appMountedRef.current = true;
    return () => {
      appMountedRef.current = false;
      for (const tab of sessionTabsRef.current) {
        if (tab.sessionId) {
          void disconnectSsh(tab.sessionId).catch(() => undefined);
        }
      }
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

  const setError = useCallback(
    (error: string | null) => {
      updateSessionTab(activeTabIdRef.current, (tab) => ({
        ...tab,
        error,
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

  const writeSystemLine = useCallback((tabId: string, message: string) => {
    terminalRefs.current
      .get(tabId)
      ?.write(`\r\n\x1b[38;5;110m${message}\x1b[0m\r\n`);
  }, []);

  const handleClientEvent = useCallback(
    (tabId: string, generation: number, event: SshClientEvent) => {
      if (!appMountedRef.current) {
        return;
      }
      const currentTab = sessionTabsRef.current.find((tab) => tab.id === tabId);
      if (!currentTab || currentTab.generation !== generation) {
        return;
      }

      let shouldActivate = false;
      let systemLine: string | null = null;

      switch (event.type) {
        case "connecting":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "connecting",
              statusDetail: "Negotiating SSH transport…",
            }),
            generation,
          );
          break;
        case "hostKey":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "verifying",
              statusDetail:
                event.hop.kind === "target"
                  ? "Target host confirmation is required."
                  : `Jump host ${event.hop.index} confirmation is required.`,
              pendingHostKey: event,
            }),
            generation,
          );
          shouldActivate = true;
          break;
        case "hostKeyChanged": {
          const hop =
            event.hop.kind === "target"
              ? "Target host"
              : `Jump host ${event.hop.index}`;
          const message = `${hop} key changed for ${event.host}:${event.port}. Connection blocked.`;
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "error",
              statusDetail: message,
              error: message,
              pendingHostKey: null,
              pendingAuthentication: null,
              changedHostKey: event,
              portForwards: [],
              portForwardError: null,
              portForwardBusy: false,
              form: { ...tab.form, password: "" },
              passwordVisible: false,
            }),
            generation,
          );
          shouldActivate = true;
          systemLine = message;
          break;
        }
        case "authenticationChallenge": {
          const hop =
            event.hop.kind === "target"
              ? "Target host"
              : `Jump host ${event.hop.index}`;
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "authenticating",
              statusDetail: `${hop} requires additional authentication.`,
              pendingAuthentication: event,
            }),
            generation,
          );
          shouldActivate = true;
          break;
        }
        case "authenticated":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "authenticated",
              statusDetail: "Opening an interactive PTY…",
              pendingAuthentication: null,
            }),
            generation,
          );
          break;
        case "connected":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "connected",
              statusDetail: "Interactive shell is active.",
              error: null,
            }),
            generation,
          );
          if (activeTabIdRef.current === tabId) {
            terminalRefs.current.get(tabId)?.focus();
          }
          break;
        case "exitStatus":
          systemLine = `Remote process exited with status ${event.code}.`;
          break;
        case "error":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: "error",
              statusDetail: event.message,
              error: event.message,
              pendingAuthentication: null,
              portForwards: [],
              portForwardError: null,
              portForwardBusy: false,
              form: { ...tab.form, password: "" },
              passwordVisible: false,
            }),
            generation,
          );
          systemLine = `Connection error: ${event.message}`;
          break;
        case "closed":
          updateSessionTab(
            tabId,
            (tab) => ({
              ...tab,
              status: tab.status === "error" ? tab.status : "closed",
              statusDetail: "The SSH session has ended.",
              sessionId: null,
              pendingHostKey: null,
              pendingAuthentication: null,
              portForwards: [],
              portForwardError: null,
              portForwardBusy: false,
              form: { ...tab.form, password: "" },
              passwordVisible: false,
            }),
            generation,
          );
          break;
      }

      if (systemLine) {
        writeSystemLine(tabId, systemLine);
      }
      if (shouldActivate) {
        const currentActiveTab = sessionTabsRef.current.find(
          (tab) => tab.id === activeTabIdRef.current,
        );
        const anotherTabOwnsVisibleAction =
          currentActiveTab !== undefined &&
          currentActiveTab.id !== tabId &&
          (currentActiveTab.pendingHostKey !== null ||
            currentActiveTab.pendingAuthentication !== null ||
            currentActiveTab.changedHostKey !== null);
        if (!anotherTabOwnsVisibleAction) {
          activateSessionTab(tabId);
        }
      }
    },
    [activateSessionTab, updateSessionTab, writeSystemLine],
  );

  async function handleConnect(event: FormEvent) {
    event.preventDefault();

    const tab = sessionTabsRef.current.find(
      (candidate) => candidate.id === activeTabIdRef.current,
    );
    if (!tab) return;

    const selectedHost =
      hosts.find((host) => host.id === tab.selectedSavedHostId) ?? null;
    const port = Number(tab.form.port);
    if (
      !selectedHost &&
      (!tab.form.host.trim() ||
        !tab.form.username.trim() ||
        !Number.isInteger(port))
    ) {
      setError("Host, port, and username are required.");
      return;
    }

    const tabId = tab.id;
    const generation = tab.generation + 1;
    const terminalSize = tab.terminalSize;
    const connectionForm = { ...tab.form };
    updateSessionTab(tabId, (current) => ({
      ...current,
      generation,
      status: "connecting",
      statusDetail: "Preparing connection…",
      sessionId: null,
      pendingHostKey: null,
      changedHostKey: null,
      pendingAuthentication: null,
      portForwards: [],
      portForwardError: null,
      portForwardBusy: false,
      passwordVisible: false,
      error: null,
      form: { ...current.form, password: "" },
    }));
    const terminal = terminalRefs.current.get(tabId);
    terminal?.reset();
    terminal?.write(
      `\x1b[1;36mAnySSH Phase 1\x1b[0m\r\nStarting ${
        selectedHost ? "saved Host" : "a secure"
      } SSH session…\r\n`,
    );

    try {
      const callbacks = {
        onEvent: (clientEvent: SshClientEvent) =>
          handleClientEvent(tabId, generation, clientEvent),
        onData: (data: Uint8Array) =>
          new Promise<void>((resolve) => {
            const targetTerminal = terminalRefs.current.get(tabId);
            if (targetTerminal) {
              targetTerminal.write(data, resolve);
            } else {
              resolve();
            }
          }),
      };
      const id = selectedHost
        ? await connectSavedHost(
            {
              hostId: selectedHost.id,
              columns: terminalSize.columns,
              rows: terminalSize.rows,
            },
            callbacks,
          )
        : await connectSsh(
            {
              host: connectionForm.host.trim(),
              port,
              authentication:
                connectionForm.authenticationKind === "keyboardInteractive"
                  ? {
                      kind: "keyboardInteractive",
                      username: connectionForm.username.trim(),
                    }
                  : {
                      kind: "temporaryPassword",
                      username: connectionForm.username.trim(),
                      password: connectionForm.password,
                    },
              columns: terminalSize.columns,
              rows: terminalSize.rows,
            },
            callbacks,
          );

      const current = sessionTabsRef.current.find(
        (candidate) => candidate.id === tabId,
      );
      if (
        !appMountedRef.current ||
        !current ||
        current.generation !== generation ||
        current.status === "closed" ||
        current.status === "error"
      ) {
        await disconnectSsh(id).catch(() => undefined);
        return;
      }
      updateSessionTab(
        tabId,
        (currentTab) => ({
          ...currentTab,
          sessionId: id,
          form: { ...currentTab.form, password: "" },
          passwordVisible: false,
        }),
        generation,
      );
    } catch (connectionError) {
      const message =
        connectionError instanceof Error
          ? connectionError.message
          : String(connectionError);
      handleClientEvent(tabId, generation, { type: "error", message });
    }
  }

  async function handleHostKeyDecision(accepted: boolean) {
    const tab = sessionTabsRef.current.find(
      (candidate) => candidate.id === activeTabIdRef.current,
    );
    if (!tab?.sessionId || !tab.pendingHostKey) return;

    const { id: tabId, generation, sessionId: targetSessionId } = tab;
    const requestId = tab.pendingHostKey.requestId;
    updateSessionTab(
      tabId,
      (current) => ({ ...current, pendingHostKey: null }),
      generation,
    );
    try {
      await confirmHostKey(targetSessionId, requestId, accepted);
      if (accepted) {
        await refreshRepository();
      }
      if (!accepted) {
        updateSessionTab(
          tabId,
          (current) => ({
            ...current,
            status: "closed",
            statusDetail: "Host key was rejected.",
          }),
          generation,
        );
      }
    } catch (decisionError) {
      const message =
        decisionError instanceof Error
          ? decisionError.message
          : String(decisionError);
      handleClientEvent(tabId, generation, { type: "error", message });
    }
  }

  async function handleAuthenticationDecision(responses: string[] | null) {
    const tab = sessionTabsRef.current.find(
      (candidate) => candidate.id === activeTabIdRef.current,
    );
    if (!tab?.sessionId || !tab.pendingAuthentication) return;

    const { id: tabId, generation, sessionId: targetSessionId } = tab;
    const requestId = tab.pendingAuthentication.requestId;
    updateSessionTab(
      tabId,
      (current) => ({ ...current, pendingAuthentication: null }),
      generation,
    );
    try {
      await respondAuthentication(targetSessionId, requestId, responses);
      if (responses === null) {
        updateSessionTab(
          tabId,
          (current) => ({
            ...current,
            status: "closed",
            statusDetail: "Additional authentication was cancelled.",
          }),
          generation,
        );
      }
    } catch (responseError) {
      const message =
        responseError instanceof Error
          ? responseError.message
          : String(responseError);
      handleClientEvent(tabId, generation, { type: "error", message });
    }
  }

  async function handleDisconnect() {
    const tab = sessionTabsRef.current.find(
      (candidate) => candidate.id === activeTabIdRef.current,
    );
    if (!tab?.sessionId) return;
    updateSessionTab(tab.id, (current) => ({
      ...current,
      pendingAuthentication: null,
      form: { ...current.form, password: "" },
      passwordVisible: false,
    }));
    await disconnectSsh(tab.sessionId);
  }

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
    } catch (vaultOperationError) {
      setVaultError(String(vaultOperationError));
    }
  }

  async function handleVaultLock() {
    const freshTab = createSessionTab();
    replaceSessionTabs(() => [freshTab]);
    setActiveTabId(freshTab.id);
    terminalRefs.current.clear();
    setCredentials([]);
    setGroups([]);
    setHosts([]);
    setRoutes([]);
    setKnownHosts([]);
    setWorkspaceView("terminal");
    setVaultError(null);

    try {
      setVaultStatus(await lockVault());
    } catch (vaultOperationError) {
      setVaultError(String(vaultOperationError));
    }
  }

  const handleTerminalInput = useCallback((tabId: string, input: string) => {
    const tab = sessionTabsRef.current.find(
      (candidate) => candidate.id === tabId,
    );
    if (!tab?.sessionId || tab.status !== "connected") return;
    void sendSshInput(tab.sessionId, input);
  }, []);

  const handleTerminalResize = useCallback(
    (tabId: string, columns: number, rows: number) => {
      if (columns < 2 || rows < 1) return;
      const tab = sessionTabsRef.current.find(
        (candidate) => candidate.id === tabId,
      );
      if (!tab) return;
      updateSessionTab(tabId, (current) => ({
        ...current,
        terminalSize: { columns, rows },
      }));
      if (tab.sessionId && tab.status === "connected") {
        void resizeSsh(tab.sessionId, columns, rows);
      }
    },
    [updateSessionTab],
  );

  async function handleStartPortForward(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const tabId = activeTabIdRef.current;
    const tab = sessionTabsRef.current.find((item) => item.id === tabId);
    if (!tab?.sessionId || tab.status !== "connected") {
      updateSessionTab(tabId, (current) => ({
        ...current,
        portForwardError: "Connect this session before starting a forward.",
      }));
      return;
    }

    const bindPort = Number(portForwardForm.bindPort);
    const destinationPort = Number(portForwardForm.destinationPort);
    if (
      !Number.isInteger(bindPort) ||
      bindPort < 0 ||
      bindPort > 65_535 ||
      (portForwardForm.kind !== "dynamic" &&
        (!portForwardForm.destinationHost.trim() ||
          !Number.isInteger(destinationPort) ||
          destinationPort < 1 ||
          destinationPort > 65_535))
    ) {
      updateSessionTab(tabId, (current) => ({
        ...current,
        portForwardError: "Enter valid bind and destination ports.",
      }));
      return;
    }

    const sessionId = tab.sessionId;
    const generation = tab.generation;
    updateSessionTab(tabId, (current) => ({
      ...current,
      portForwardError: null,
      portForwardBusy: true,
    }));
    try {
      const summary = await startSshPortForward(sessionId, {
        kind: portForwardForm.kind,
        bindHost: portForwardForm.bindHost,
        bindPort,
        ...(portForwardForm.kind === "dynamic"
          ? {}
          : {
              destinationHost: portForwardForm.destinationHost.trim(),
              destinationPort,
            }),
      });
      updateSessionTab(tabId, (current) =>
        current.sessionId === sessionId && current.generation === generation
          ? {
              ...current,
              portForwards: [...current.portForwards, summary],
              portForwardBusy: false,
            }
          : current,
      );
    } catch (forwardError) {
      updateSessionTab(tabId, (current) =>
        current.sessionId === sessionId && current.generation === generation
          ? {
              ...current,
              portForwardError:
                forwardError instanceof Error
                  ? forwardError.message
                  : String(forwardError),
              portForwardBusy: false,
            }
          : current,
      );
    }
  }

  async function handleStopPortForward(forwardId: string) {
    const tabId = activeTabIdRef.current;
    const tab = sessionTabsRef.current.find((item) => item.id === tabId);
    if (!tab?.sessionId) return;
    const sessionId = tab.sessionId;
    try {
      await stopSshPortForward(sessionId, forwardId);
      updateSessionTab(tabId, (current) =>
        current.sessionId === sessionId
          ? {
              ...current,
              portForwards: current.portForwards.filter(
                (forward) => forward.id !== forwardId,
              ),
              portForwardError: null,
            }
          : current,
      );
    } catch (forwardError) {
      updateSessionTab(tabId, (current) => ({
        ...current,
        portForwardError:
          forwardError instanceof Error
            ? forwardError.message
            : String(forwardError),
      }));
    }
  }

  function selectSavedHost(host: HostSummary) {
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
          ...INITIAL_FORM,
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
      return;
    }
    appendSessionTab(createSessionTab({ kind: "savedHost", host }));
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
      : configurationTitle[workspaceView];

  if (isNativeRuntime && vaultStatus?.state !== "unlocked") {
    return (
      <VaultGate
        error={vaultError}
        onClearError={() => setVaultError(null)}
        onSubmit={handleVaultSubmit}
        status={vaultStatus}
      />
    );
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true">
            <span />
          </div>
          <div>
            <strong>AnySSH</strong>
            <small>Phase 1 desktop MVP</small>
          </div>
        </div>

        <nav className="primary-nav" aria-label="Primary">
          <button
            className={`nav-item ${workspaceView === "terminal" ? "active" : ""}`}
            onClick={() => setWorkspaceView("terminal")}
            type="button"
          >
            <NavIcon name="terminal" />
            Terminal
            <span className="nav-count">{sessionTabs.length}</span>
          </button>
          <button
            className={`nav-item ${workspaceView === "groups" ? "active" : ""}`}
            onClick={() => setWorkspaceView("groups")}
            type="button"
          >
            <NavIcon name="groups" />
            Groups
            <span className="nav-count">{groups.length}</span>
          </button>
          <button
            className={`nav-item ${workspaceView === "hosts" ? "active" : ""}`}
            onClick={() => setWorkspaceView("hosts")}
            type="button"
          >
            <NavIcon name="hosts" />
            Hosts
            <span className="nav-count">{hosts.length}</span>
          </button>
          <button
            className={`nav-item ${workspaceView === "credentials" ? "active" : ""}`}
            onClick={() => setWorkspaceView("credentials")}
            type="button"
          >
            <NavIcon name="keys" />
            Credentials
            <span className="nav-count">{credentials.length}</span>
          </button>
          <button
            className={`nav-item ${workspaceView === "routes" ? "active" : ""}`}
            onClick={() => setWorkspaceView("routes")}
            type="button"
          >
            <NavIcon name="routes" />
            Jump routes
            <span className="nav-count">{routes.length}</span>
          </button>
          <button
            className={`nav-item ${workspaceView === "knownHosts" ? "active" : ""}`}
            onClick={() => setWorkspaceView("knownHosts")}
            type="button"
          >
            <NavIcon name="knownHosts" />
            Known hosts
            <span className="nav-count">{knownHosts.length}</span>
          </button>
        </nav>

        <div className="section-heading">
          <span>Saved hosts</span>
          <button
            aria-label="Manage Hosts"
            onClick={() => setWorkspaceView("hosts")}
            type="button"
          >
            +
          </button>
        </div>

        <div className="host-list">
          {hosts.map((host, index) => (
            <button
              className={`host-card ${
                selectedSavedHostId === host.id ? "selected" : ""
              }`}
              key={host.id}
              onClick={() => selectSavedHost(host)}
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
          {!repositoryLoading && hosts.length === 0 && (
            <button
              className="empty-host-list"
              onClick={() => setWorkspaceView("hosts")}
              type="button"
            >
              Add your first Host
            </button>
          )}
        </div>

        <div className="sidebar-footer">
          <span
            className={`runtime-dot ${isNativeRuntime ? "native" : "preview"}`}
          />
          <div>
            <strong>
              {isNativeRuntime ? "Native runtime" : "Browser QA mode"}
            </strong>
            <small>
              {isNativeRuntime
                ? vaultStatus?.cipherVersion
                  ? `SQLCipher ${vaultStatus.cipherVersion}`
                  : "Rust core ready"
                : "No network connections"}
            </small>
          </div>
        </div>
      </aside>

      <section className="workspace">
        <header className="workspace-header">
          <div>
            <p className="eyebrow">
              {workspaceView === "terminal"
                ? "SSH workspace"
                : "Vault configuration"}
            </p>
            <h1>{workspaceTitle}</h1>
          </div>
          <div className="header-actions">
            <div className={`status-pill ${statusTone}`} aria-live="polite">
              <span />
              {STATUS_LABEL[status]}
            </div>
            {sessionId && connected && (
              <button
                className="secondary-button"
                onClick={handleDisconnect}
                type="button"
              >
                Disconnect
              </button>
            )}
            {isNativeRuntime && (
              <button
                className="secondary-button"
                onClick={() => void handleVaultLock()}
                type="button"
              >
                Lock Vault
              </button>
            )}
          </div>
        </header>

        <nav className="mobile-primary-nav" aria-label="Mobile workspace">
          {(
            [
              ["terminal", "Terminal"],
              ["groups", "Groups"],
              ["hosts", "Hosts"],
              ["credentials", "Credentials"],
              ["routes", "Routes"],
              ["knownHosts", "Known"],
            ] as const
          ).map(([view, label]) => (
            <button
              className={workspaceView === view ? "active" : ""}
              key={view}
              onClick={() => setWorkspaceView(view)}
              type="button"
            >
              {label}
            </button>
          ))}
        </nav>

        <div
          aria-hidden={workspaceView !== "terminal"}
          className={`workspace-body ${
            workspaceView === "terminal" ? "" : "workspace-body-hidden"
          }`}
          inert={workspaceView !== "terminal"}
        >
          <section className="terminal-card" aria-label="SSH terminal">
            <div className="session-tab-strip">
              <div
                aria-label="SSH sessions"
                className="session-tab-list"
                role="tablist"
              >
                {sessionTabs.map((tab) => {
                  const tabConnected = tab.status === "connected";
                  const tabPending =
                    tab.pendingHostKey !== null ||
                    tab.pendingAuthentication !== null;
                  return (
                    <div
                      className={`session-tab ${
                        tab.id === activeTab.id ? "active" : ""
                      }`}
                      key={tab.id}
                    >
                      <button
                        aria-controls={`session-panel-${tab.id}`}
                        aria-selected={tab.id === activeTab.id}
                        className="session-tab-activate"
                        id={`session-tab-${tab.id}`}
                        onClick={() => activateSessionTab(tab.id)}
                        onKeyDown={(event) =>
                          handleSessionTabKeyDown(event, tab.id)
                        }
                        role="tab"
                        tabIndex={tab.id === activeTab.id ? 0 : -1}
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
                        onClick={() => void closeSessionTab(tab.id)}
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
                disabled={sessionTabs.length >= MAX_SESSION_TABS}
                onClick={newQuickSessionTab}
                type="button"
              >
                +
              </button>
            </div>
            <div className="terminal-toolbar">
              <div className="window-controls" aria-hidden="true">
                <span />
                <span />
                <span />
              </div>
              <div className="terminal-title">
                <span>
                  {selectedCredential?.username || form.username || "user"}@
                </span>
                {selectedSavedHost?.host || form.host || "host"}
              </div>
              <span className="terminal-security">
                <LockIcon />
                Host key verification
              </span>
            </div>
            <div className="terminal-tab-panels">
              {sessionTabs.map((tab) => {
                const visible =
                  workspaceView === "terminal" && tab.id === activeTab.id;
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
                      onInput={(input) => handleTerminalInput(tab.id, input)}
                      onResize={(columns, rows) =>
                        handleTerminalResize(tab.id, columns, rows)
                      }
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

          {workspaceView === "terminal" && (
            <aside className="connection-panel">
              <div className="panel-heading">
                <div>
                  <p className="eyebrow">Connection</p>
                  <h2>{selectedSavedHost ? "Saved Host" : "Open a session"}</h2>
                </div>
                <span className="protocol-badge">SSH</span>
              </div>

              {selectedSavedHost ? (
                <form onSubmit={handleConnect}>
                  <div className="saved-connection-summary">
                    <div>
                      <span>Endpoint</span>
                      <strong>
                        {selectedSavedHost.host}:{selectedSavedHost.port}
                      </strong>
                    </div>
                    <div>
                      <span>Credential</span>
                      <strong>
                        {selectedCredential
                          ? `${selectedCredential.label} · ${selectedCredential.username}`
                          : "No Credential selected"}
                      </strong>
                    </div>
                    <div>
                      <span>Jump Route</span>
                      <strong>
                        {selectedRoute
                          ? `${selectedRoute.label} · ${selectedRoute.hostIds.length} hop(s)`
                          : "Direct connection"}
                      </strong>
                    </div>
                  </div>

                  {error && (
                    <div className="inline-error" role="alert">
                      {error}
                    </div>
                  )}

                  <button
                    className="connect-button"
                    disabled={
                      busy ||
                      connected ||
                      !selectedSavedHost.effectiveCredentialId
                    }
                    type="submit"
                  >
                    <span>
                      {busy
                        ? "Connecting…"
                        : connected
                          ? "Session active"
                          : isNativeRuntime
                            ? "Connect saved Host"
                            : "Native runtime required"}
                    </span>
                    <span aria-hidden="true">↗</span>
                  </button>
                  <button
                    className="secondary-button full-width-button"
                    disabled={busy || connected}
                    onClick={useQuickConnection}
                    type="button"
                  >
                    Use quick connection
                  </button>
                </form>
              ) : (
                <form onSubmit={handleConnect}>
                  <label>
                    Display name
                    <input
                      autoComplete="off"
                      onChange={(event) =>
                        setForm((current) => ({
                          ...current,
                          name: event.target.value,
                        }))
                      }
                      value={form.name}
                    />
                  </label>

                  <div className="field-grid">
                    <label>
                      Host
                      <input
                        autoCapitalize="none"
                        autoComplete="off"
                        onChange={(event) =>
                          setForm((current) => ({
                            ...current,
                            host: event.target.value,
                          }))
                        }
                        placeholder="server.example.com"
                        spellCheck={false}
                        value={form.host}
                      />
                    </label>
                    <label className="port-field">
                      Port
                      <input
                        inputMode="numeric"
                        min="1"
                        max="65535"
                        onChange={(event) =>
                          setForm((current) => ({
                            ...current,
                            port: event.target.value,
                          }))
                        }
                        type="number"
                        value={form.port}
                      />
                    </label>
                  </div>

                  <div className="field-grid authentication-field-grid">
                    <label>
                      Username
                      <input
                        autoCapitalize="none"
                        autoComplete="username"
                        onChange={(event) =>
                          setForm((current) => ({
                            ...current,
                            username: event.target.value,
                          }))
                        }
                        value={form.username}
                      />
                    </label>

                    <label>
                      Authentication
                      <select
                        onChange={(event) => {
                          const authenticationKind = event.target.value as
                            "password" | "keyboardInteractive";
                          setForm((current) => ({
                            ...current,
                            authenticationKind,
                            password:
                              authenticationKind === "password"
                                ? current.password
                                : "",
                          }));
                          setPasswordVisible(false);
                        }}
                        value={form.authenticationKind}
                      >
                        <option value="password">Temporary password</option>
                        <option value="keyboardInteractive">
                          Keyboard-interactive / OTP
                        </option>
                      </select>
                    </label>
                  </div>

                  {form.authenticationKind === "password" ? (
                    <div className="form-field">
                      <label htmlFor="connection-password">Password</label>
                      <span className="password-field">
                        <input
                          autoComplete="current-password"
                          id="connection-password"
                          onChange={(event) =>
                            setForm((current) => ({
                              ...current,
                              password: event.target.value,
                            }))
                          }
                          placeholder="Temporary, not stored"
                          type={passwordVisible ? "text" : "password"}
                          value={form.password}
                        />
                        <button
                          aria-label={
                            passwordVisible ? "Hide password" : "Show password"
                          }
                          onClick={() =>
                            setPasswordVisible((visible) => !visible)
                          }
                          type="button"
                        >
                          {passwordVisible ? "Hide" : "Show"}
                        </button>
                      </span>
                    </div>
                  ) : (
                    <div className="security-note compact-security-note">
                      <strong>Prompted during this session</strong>
                      <p>
                        AnySSH sends only the responses requested by the SSH
                        server. Responses are cleared after each round and are
                        never saved.
                      </p>
                    </div>
                  )}

                  {error && (
                    <div className="inline-error" role="alert">
                      {error}
                    </div>
                  )}

                  <button
                    className="connect-button"
                    disabled={busy || connected}
                    type="submit"
                  >
                    <span>
                      {busy
                        ? "Connecting…"
                        : connected
                          ? "Session active"
                          : "Connect"}
                    </span>
                    <span aria-hidden="true">↗</span>
                  </button>
                </form>
              )}

              <section
                aria-labelledby="port-forwarding-title"
                className="forwarding-panel"
              >
                <div className="forwarding-heading">
                  <div>
                    <p className="eyebrow">Session scoped</p>
                    <h3 id="port-forwarding-title">Port forwarding</h3>
                  </div>
                  <span>{portForwards.length}/16</span>
                </div>
                <form
                  className="forwarding-form"
                  onSubmit={handleStartPortForward}
                >
                  <label>
                    Type
                    <select
                      aria-label="Port forward type"
                      onChange={(event) =>
                        setPortForwardForm((current) => ({
                          ...current,
                          kind: event.target.value as SshPortForwardKind,
                        }))
                      }
                      value={portForwardForm.kind}
                    >
                      <option value="local">Local</option>
                      <option value="remote">Remote</option>
                      <option value="dynamic">Dynamic SOCKS5</option>
                    </select>
                  </label>
                  <div className="field-grid">
                    <label>
                      {portForwardForm.kind === "remote"
                        ? "Server bind"
                        : "Local bind"}
                      <select
                        aria-label="Port forward bind host"
                        onChange={(event) =>
                          setPortForwardForm((current) => ({
                            ...current,
                            bindHost: event.target.value,
                          }))
                        }
                        value={portForwardForm.bindHost}
                      >
                        <option value="127.0.0.1">127.0.0.1</option>
                        <option value="::1">::1</option>
                      </select>
                    </label>
                    <label className="port-field">
                      Bind port
                      <input
                        aria-label="Forward bind number"
                        inputMode="numeric"
                        max="65535"
                        min="0"
                        onChange={(event) =>
                          setPortForwardForm((current) => ({
                            ...current,
                            bindPort: event.target.value,
                          }))
                        }
                        type="number"
                        value={portForwardForm.bindPort}
                      />
                    </label>
                  </div>
                  {portForwardForm.kind !== "dynamic" && (
                    <div className="field-grid">
                      <label>
                        {portForwardForm.kind === "remote"
                          ? "Local destination"
                          : "Target destination"}
                        <input
                          aria-label="Port forward destination host"
                          autoCapitalize="none"
                          onChange={(event) =>
                            setPortForwardForm((current) => ({
                              ...current,
                              destinationHost: event.target.value,
                            }))
                          }
                          spellCheck={false}
                          value={portForwardForm.destinationHost}
                        />
                      </label>
                      <label className="port-field">
                        Port
                        <input
                          aria-label="Forward destination number"
                          inputMode="numeric"
                          max="65535"
                          min="1"
                          onChange={(event) =>
                            setPortForwardForm((current) => ({
                              ...current,
                              destinationPort: event.target.value,
                            }))
                          }
                          type="number"
                          value={portForwardForm.destinationPort}
                        />
                      </label>
                    </div>
                  )}
                  <p className="forwarding-policy">
                    Loopback only. Payloads stay in Rust and are never sent
                    through the WebView.
                  </p>
                  {portForwardError && (
                    <div className="inline-error" role="alert">
                      {portForwardError}
                    </div>
                  )}
                  <button
                    className="secondary-button full-width-button"
                    disabled={
                      !connected || portForwardBusy || portForwards.length >= 16
                    }
                    type="submit"
                  >
                    {!connected
                      ? "Session required"
                      : portForwardBusy
                        ? "Starting…"
                        : "Start forward"}
                  </button>
                </form>

                {portForwards.length > 0 && (
                  <ul
                    aria-label="Active port forwards"
                    className="forwarding-list"
                  >
                    {portForwards.map((forward) => (
                      <li key={forward.id}>
                        <div>
                          <strong>
                            {forward.kind === "dynamic"
                              ? "SOCKS5"
                              : forward.kind === "local"
                                ? "Local"
                                : "Remote"}{" "}
                            ·{" "}
                            {formatForwardEndpoint(
                              forward.bindHost,
                              forward.boundPort,
                            )}
                          </strong>
                          <span>
                            {forward.kind === "dynamic"
                              ? "Unauthenticated CONNECT"
                              : `→ ${formatForwardEndpoint(
                                  forward.destinationHost ?? "",
                                  forward.destinationPort ?? 0,
                                )}`}
                          </span>
                        </div>
                        <button
                          aria-label={`Stop ${forward.kind} forward on port ${forward.boundPort}`}
                          onClick={() => void handleStopPortForward(forward.id)}
                          type="button"
                        >
                          Stop
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <div className="connection-state" aria-live="polite">
                <span className={`state-icon ${statusTone}`}>
                  <LockIcon />
                </span>
                <div>
                  <strong>{STATUS_LABEL[status]}</strong>
                  <p>{statusDetail}</p>
                </div>
              </div>
            </aside>
          )}
        </div>
        {workspaceView !== "terminal" && (
          <ConfigurationWorkspace
            credentials={credentials}
            groups={groups}
            hosts={hosts}
            knownHosts={knownHosts}
            loadError={repositoryError}
            loading={repositoryLoading}
            onChanged={refreshRepository}
            onOpenHost={selectSavedHost}
            routes={routes}
            section={workspaceView}
          />
        )}
      </section>

      {pendingHostKey && (
        <div className="dialog-backdrop">
          <section
            aria-labelledby="host-key-title"
            aria-modal="true"
            className="host-key-dialog"
            role="dialog"
          >
            <div className="dialog-icon">
              <LockIcon />
            </div>
            <p className="eyebrow">
              {pendingHostKey.hop.kind === "target"
                ? "Target host"
                : `Jump host ${pendingHostKey.hop.index}`}
            </p>
            <h2 id="host-key-title">Verify server identity</h2>
            <p>
              Confirm this fingerprint through a trusted channel before
              continuing.
            </p>
            <dl>
              <div>
                <dt>Host</dt>
                <dd>
                  {pendingHostKey.host}:{pendingHostKey.port}
                </dd>
              </div>
              <div>
                <dt>Algorithm</dt>
                <dd>{pendingHostKey.algorithm}</dd>
              </div>
            </dl>
            <code>{pendingHostKey.fingerprintSha256}</code>
            <div className="dialog-actions">
              <button
                className="secondary-button"
                onClick={() => void handleHostKeyDecision(false)}
                type="button"
              >
                Reject
              </button>
              <button
                className="connect-button"
                onClick={() => void handleHostKeyDecision(true)}
                type="button"
              >
                Trust and continue
              </button>
            </div>
          </section>
        </div>
      )}
      {pendingAuthentication && (
        <AuthenticationChallengeDialog
          challenge={pendingAuthentication}
          key={`${sessionId ?? "pending"}-${pendingAuthentication.requestId}`}
          onDecision={handleAuthenticationDecision}
        />
      )}
      {changedHostKey && (
        <div className="dialog-backdrop">
          <section
            aria-labelledby="changed-host-key-title"
            aria-modal="true"
            className="host-key-dialog changed-host-key-dialog"
            role="alertdialog"
          >
            <div className="dialog-icon danger-icon">
              <LockIcon />
            </div>
            <p className="eyebrow">
              {changedHostKey.hop.kind === "target"
                ? "Target host"
                : `Jump host ${changedHostKey.hop.index}`}
            </p>
            <h2 id="changed-host-key-title">Host key changed</h2>
            <p>
              AnySSH blocked the connection. Verify the server through a trusted
              channel before forgetting the existing trust.
            </p>
            <dl>
              <div>
                <dt>Host</dt>
                <dd>
                  {changedHostKey.host}:{changedHostKey.port}
                </dd>
              </div>
              <div>
                <dt>Algorithm</dt>
                <dd>{changedHostKey.algorithm}</dd>
              </div>
            </dl>
            <div className="changed-key-comparison">
              <div>
                <span>Trusted</span>
                {changedHostKey.trustedFingerprintsSha256.map((fingerprint) => (
                  <code key={fingerprint}>{fingerprint}</code>
                ))}
              </div>
              <div>
                <span>Received</span>
                <code>{changedHostKey.receivedFingerprintSha256}</code>
              </div>
            </div>
            <div className="dialog-actions">
              <button
                className="secondary-button"
                onClick={() => setChangedHostKey(null)}
                type="button"
              >
                Close
              </button>
              <button
                className="connect-button"
                onClick={() => {
                  setChangedHostKey(null);
                  setWorkspaceView("knownHosts");
                  void refreshRepository();
                }}
                type="button"
              >
                Open Known Hosts
              </button>
            </div>
          </section>
        </div>
      )}
    </main>
  );
}

function AuthenticationChallengeDialog({
  challenge,
  onDecision,
}: {
  challenge: AuthenticationChallengeEvent;
  onDecision(responses: string[] | null): Promise<void>;
}) {
  const [responses, setResponses] = useState(() =>
    challenge.prompts.map(() => ""),
  );

  function clearResponses() {
    setResponses(challenge.prompts.map(() => ""));
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    const submitted = [...responses];
    clearResponses();
    void onDecision(submitted);
  }

  function cancel() {
    clearResponses();
    void onDecision(null);
  }

  return (
    <div className="dialog-backdrop">
      <section
        aria-labelledby="authentication-challenge-title"
        aria-modal="true"
        className="host-key-dialog authentication-dialog"
        role="dialog"
      >
        <div className="dialog-icon">
          <LockIcon />
        </div>
        <p className="eyebrow">
          {challenge.hop.kind === "target"
            ? "Target host"
            : `Jump host ${challenge.hop.index}`}
        </p>
        <h2 id="authentication-challenge-title">
          {challenge.name || "Additional authentication"}
        </h2>
        {challenge.instructions && (
          <p className="authentication-instructions">
            {challenge.instructions}
          </p>
        )}
        <dl>
          <div>
            <dt>Host</dt>
            <dd>
              {challenge.host}:{challenge.port}
            </dd>
          </div>
        </dl>
        <form className="authentication-form" onSubmit={submit}>
          {challenge.prompts.map((prompt, index) => (
            <label key={`${challenge.requestId}-${index}`}>
              {prompt.text || `Response ${index + 1}`}
              <input
                autoComplete={prompt.echo ? "off" : "one-time-code"}
                autoFocus={index === 0}
                onChange={(event) =>
                  setResponses((current) =>
                    current.map((value, responseIndex) =>
                      responseIndex === index ? event.target.value : value,
                    ),
                  )
                }
                spellCheck={false}
                type={prompt.echo ? "text" : "password"}
                value={responses[index] ?? ""}
              />
            </label>
          ))}
          <div className="dialog-actions">
            <button className="secondary-button" onClick={cancel} type="button">
              Cancel authentication
            </button>
            <button className="connect-button" type="submit">
              Continue
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

function NavIcon({
  name,
}: {
  name: "terminal" | "groups" | "hosts" | "keys" | "routes" | "knownHosts";
}) {
  const paths = {
    terminal: "M4 5h16v14H4zM7.5 9l3 3-3 3M12.5 15H17",
    groups: "M5 5h6v5H5zM13 14h6v5h-6zM8 10v2a2 2 0 0 0 2 2h3",
    hosts: "M4 5.5h16v11H4zM8 19h8M12 16.5V19",
    keys: "M15.5 7.5a4 4 0 1 1-3.7 5.5L4 20.8V17h3v-3h3l1.8-1.8",
    routes: "M6 5.5h4v4H6zM14 14.5h4v4h-4zM10 7.5h3a3 3 0 0 1 3 3v4",
    knownHosts:
      "M12 3.5 19 6v5.5c0 4.2-2.8 7.4-7 9-4.2-1.6-7-4.8-7-9V6l7-2.5Zm-3 8 2 2 4-4",
  };

  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 24 24">
      <path
        d={paths[name]}
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth="1.7"
      />
    </svg>
  );
}

function LockIcon() {
  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 24 24">
      <path
        d="M7.5 10V7.8a4.5 4.5 0 0 1 9 0V10m-10 0h11a1 1 0 0 1 1 1v8h-13v-8a1 1 0 0 1 1-1Z"
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth="1.7"
      />
    </svg>
  );
}

export default App;
