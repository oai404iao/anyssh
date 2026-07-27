import {
  FormEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
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
  resizeSsh,
  sendSshInput,
  type HostKeyEvent,
  type SshClientEvent,
} from "./lib/ssh-bridge";
import {
  listCredentials,
  type CredentialSummary,
} from "./lib/credential-bridge";
import {
  listHosts,
  listJumpRoutes,
  type HostSummary,
  type JumpRouteSummary,
} from "./lib/host-bridge";
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
  | "authenticated"
  | "connected"
  | "error"
  | "closed";

interface ConnectionForm {
  name: string;
  host: string;
  port: string;
  username: string;
  password: string;
}

type WorkspaceView = "terminal" | ConfigurationSection;

const INITIAL_FORM: ConnectionForm = {
  name: "Local lab",
  host: "127.0.0.1",
  port: "2222",
  username: "anyssh",
  password: "",
};

const STATUS_LABEL: Record<ConnectionStatus, string> = {
  idle: "Ready",
  connecting: "Connecting",
  verifying: "Verify host",
  authenticated: "Authenticated",
  connected: "Connected",
  error: "Connection failed",
  closed: "Disconnected",
};

function App() {
  const terminalRef = useRef<TerminalHandle>(null);
  const terminalSizeRef = useRef({ columns: 120, rows: 32 });
  const [form, setForm] = useState<ConnectionForm>(INITIAL_FORM);
  const [status, setStatus] = useState<ConnectionStatus>("idle");
  const [statusDetail, setStatusDetail] = useState(
    isNativeRuntime
      ? "Native SSH runtime is available."
      : "Browser QA mode uses a local terminal simulation.",
  );
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [pendingHostKey, setPendingHostKey] = useState<HostKeyEvent | null>(
    null,
  );
  const [passwordVisible, setPasswordVisible] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [vaultStatus, setVaultStatus] = useState<VaultStatus | null>(null);
  const [vaultError, setVaultError] = useState<string | null>(null);
  const [workspaceView, setWorkspaceView] = useState<WorkspaceView>("terminal");
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [hosts, setHosts] = useState<HostSummary[]>([]);
  const [routes, setRoutes] = useState<JumpRouteSummary[]>([]);
  const [repositoryLoading, setRepositoryLoading] = useState(false);
  const [repositoryError, setRepositoryError] = useState<string | null>(null);
  const [selectedSavedHostId, setSelectedSavedHostId] = useState<string | null>(
    null,
  );

  const refreshRepository = useCallback(async () => {
    setRepositoryLoading(true);
    setRepositoryError(null);
    try {
      const [nextCredentials, nextHosts, nextRoutes] = await Promise.all([
        listCredentials(),
        listHosts(),
        listJumpRoutes(),
      ]);
      setCredentials(nextCredentials);
      setHosts(nextHosts);
      setRoutes(nextRoutes);
      setSelectedSavedHostId((current) =>
        current && nextHosts.some((host) => host.id === current)
          ? current
          : null,
      );
    } catch (loadError) {
      setRepositoryError(String(loadError));
    } finally {
      setRepositoryLoading(false);
    }
  }, []);

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

  const connected = status === "connected";
  const busy = ["connecting", "verifying", "authenticated"].includes(status);
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
      selectedSavedHost?.credentialId
        ? (credentials.find(
            (credential) => credential.id === selectedSavedHost.credentialId,
          ) ?? null)
        : null,
    [credentials, selectedSavedHost],
  );
  const selectedRoute = useMemo(
    () =>
      selectedSavedHost?.jumpRouteId
        ? (routes.find((route) => route.id === selectedSavedHost.jumpRouteId) ??
          null)
        : null,
    [routes, selectedSavedHost],
  );

  const writeSystemLine = useCallback((message: string) => {
    terminalRef.current?.write(`\r\n\x1b[38;5;110m${message}\x1b[0m\r\n`);
  }, []);

  const handleClientEvent = useCallback(
    (event: SshClientEvent) => {
      switch (event.type) {
        case "connecting":
          setStatus("connecting");
          setStatusDetail("Negotiating SSH transport…");
          break;
        case "hostKey":
          setStatus("verifying");
          setStatusDetail(
            event.hop.kind === "target"
              ? "Target host confirmation is required."
              : `Jump host ${event.hop.index} confirmation is required.`,
          );
          setPendingHostKey(event);
          break;
        case "authenticated":
          setStatus("authenticated");
          setStatusDetail("Opening an interactive PTY…");
          break;
        case "connected":
          setStatus("connected");
          setStatusDetail("Interactive shell is active.");
          setError(null);
          terminalRef.current?.focus();
          break;
        case "exitStatus":
          writeSystemLine(`Remote process exited with status ${event.code}.`);
          break;
        case "error":
          setStatus("error");
          setStatusDetail(event.message);
          setError(event.message);
          setForm((current) => ({ ...current, password: "" }));
          setPasswordVisible(false);
          writeSystemLine(`Connection error: ${event.message}`);
          break;
        case "closed":
          setStatus((current) => (current === "error" ? current : "closed"));
          setStatusDetail("The SSH session has ended.");
          setSessionId(null);
          setPendingHostKey(null);
          setForm((current) => ({ ...current, password: "" }));
          setPasswordVisible(false);
          break;
      }
    },
    [writeSystemLine],
  );

  async function handleConnect(event: FormEvent) {
    event.preventDefault();

    const port = Number(form.port);
    if (
      !selectedSavedHost &&
      (!form.host.trim() || !form.username.trim() || !Number.isInteger(port))
    ) {
      setError("Host, port, and username are required.");
      return;
    }

    setError(null);
    setPendingHostKey(null);
    setStatus("connecting");
    setStatusDetail("Preparing connection…");
    terminalRef.current?.reset();
    terminalRef.current?.write(
      `\x1b[1;36mAnySSH Phase 0\x1b[0m\r\nStarting ${
        selectedSavedHost ? "saved Host" : "a secure"
      } SSH session…\r\n`,
    );

    try {
      const callbacks = {
        onEvent: handleClientEvent,
        onData: (data: Uint8Array) =>
          new Promise<void>((resolve) => {
            const terminal = terminalRef.current;
            if (terminal) {
              terminal.write(data, resolve);
            } else {
              resolve();
            }
          }),
      };
      const id = selectedSavedHost
        ? await connectSavedHost(
            {
              hostId: selectedSavedHost.id,
              columns: terminalSizeRef.current.columns,
              rows: terminalSizeRef.current.rows,
            },
            callbacks,
          )
        : await connectSsh(
            {
              host: form.host.trim(),
              port,
              authentication: {
                kind: "temporaryPassword",
                username: form.username.trim(),
                password: form.password,
              },
              columns: terminalSizeRef.current.columns,
              rows: terminalSizeRef.current.rows,
            },
            callbacks,
          );

      setSessionId(id);
      setForm((current) => ({ ...current, password: "" }));
      setPasswordVisible(false);
    } catch (connectionError) {
      const message =
        connectionError instanceof Error
          ? connectionError.message
          : String(connectionError);
      handleClientEvent({ type: "error", message });
    }
  }

  async function handleHostKeyDecision(accepted: boolean) {
    if (!sessionId || !pendingHostKey) return;

    setPendingHostKey(null);
    try {
      await confirmHostKey(sessionId, pendingHostKey.requestId, accepted);
      if (!accepted) {
        setStatus("closed");
        setStatusDetail("Host key was rejected.");
      }
    } catch (decisionError) {
      const message =
        decisionError instanceof Error
          ? decisionError.message
          : String(decisionError);
      handleClientEvent({ type: "error", message });
    }
  }

  async function handleDisconnect() {
    if (!sessionId) return;
    await disconnectSsh(sessionId);
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
      setStatus("idle");
      setStatusDetail("Native SSH runtime is available.");
      setError(null);
    } catch (vaultOperationError) {
      setVaultError(String(vaultOperationError));
    }
  }

  async function handleVaultLock() {
    setForm((current) => ({ ...current, password: "" }));
    setPasswordVisible(false);
    setPendingHostKey(null);
    setSessionId(null);
    setSelectedSavedHostId(null);
    setCredentials([]);
    setHosts([]);
    setRoutes([]);
    setWorkspaceView("terminal");
    setStatus("closed");
    setStatusDetail("The Vault is locked.");
    setVaultError(null);

    try {
      setVaultStatus(await lockVault());
    } catch (vaultOperationError) {
      setVaultError(String(vaultOperationError));
    }
  }

  const handleTerminalInput = useCallback(
    (input: string) => {
      if (!sessionId || !connected) return;
      void sendSshInput(sessionId, input);
    },
    [connected, sessionId],
  );

  const handleTerminalResize = useCallback(
    (columns: number, rows: number) => {
      terminalSizeRef.current = { columns, rows };
      if (sessionId && connected) {
        void resizeSsh(sessionId, columns, rows);
      }
    },
    [connected, sessionId],
  );

  function selectSavedHost(host: HostSummary) {
    setSelectedSavedHostId(host.id);
    setWorkspaceView("terminal");
    setError(null);
    setPasswordVisible(false);
    setForm((current) => ({
      ...current,
      name: host.displayName,
      host: host.host,
      port: String(host.port),
      password: "",
    }));
  }

  function useQuickConnection() {
    setSelectedSavedHostId(null);
    setForm(INITIAL_FORM);
    setError(null);
    setPasswordVisible(false);
  }

  const configurationTitle: Record<ConfigurationSection, string> = {
    hosts: "Hosts",
    credentials: "Credentials",
    routes: "Jump Routes",
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
            <small>Phase 0 prototype</small>
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
            <span className="nav-count">{sessionId ? "1" : "0"}</span>
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
              ["hosts", "Hosts"],
              ["credentials", "Credentials"],
              ["routes", "Routes"],
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

        {workspaceView === "terminal" ? (
          <div className="workspace-body">
            <section className="terminal-card" aria-label="SSH terminal">
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
              <TerminalPane
                onInput={handleTerminalInput}
                onResize={handleTerminalResize}
                ref={terminalRef}
              />
            </section>

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
                      busy || connected || !selectedSavedHost.credentialId
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
          </div>
        ) : (
          <ConfigurationWorkspace
            credentials={credentials}
            hosts={hosts}
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
                Trust for this session
              </button>
            </div>
          </section>
        </div>
      )}
    </main>
  );
}

function NavIcon({ name }: { name: "terminal" | "hosts" | "keys" | "routes" }) {
  const paths = {
    terminal: "M4 5h16v14H4zM7.5 9l3 3-3 3M12.5 15H17",
    hosts: "M4 5.5h16v11H4zM8 19h8M12 16.5V19",
    keys: "M15.5 7.5a4 4 0 1 1-3.7 5.5L4 20.8V17h3v-3h3l1.8-1.8",
    routes: "M6 5.5h4v4H6zM14 14.5h4v4h-4zM10 7.5h3a3 3 0 0 1 3 3v4",
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
