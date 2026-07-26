import {
  FormEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { TerminalPane, type TerminalHandle } from "./components/TerminalPane";
import { VaultGate } from "./components/VaultGate";
import {
  confirmHostKey,
  connectSsh,
  disconnectSsh,
  isNativeRuntime,
  resizeSsh,
  sendSshInput,
  type HostKeyEvent,
  type SshClientEvent,
} from "./lib/ssh-bridge";
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

const SAVED_HOSTS = [
  { id: "local", name: "Local lab", target: "127.0.0.1:2222", tone: "cyan" },
  { id: "edge", name: "Edge gateway", target: "10.0.0.8:22", tone: "violet" },
  { id: "db", name: "Database", target: "db.internal:22", tone: "amber" },
] as const;

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

  const connected = status === "connected";
  const busy = ["connecting", "verifying", "authenticated"].includes(status);
  const statusTone = useMemo(() => {
    if (connected) return "success";
    if (status === "error") return "danger";
    if (busy) return "warning";
    return "neutral";
  }, [busy, connected, status]);

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
    if (!form.host.trim() || !form.username.trim() || !Number.isInteger(port)) {
      setError("Host, port, and username are required.");
      return;
    }

    setError(null);
    setPendingHostKey(null);
    setStatus("connecting");
    setStatusDetail("Preparing connection…");
    terminalRef.current?.reset();
    terminalRef.current?.write(
      "\x1b[1;36mAnySSH Phase 0\x1b[0m\r\nStarting a secure SSH session…\r\n",
    );

    try {
      const id = await connectSsh(
        {
          host: form.host.trim(),
          port,
          username: form.username.trim(),
          password: form.password,
          columns: terminalSizeRef.current.columns,
          rows: terminalSizeRef.current.rows,
        },
        {
          onEvent: handleClientEvent,
          onData: (data) =>
            new Promise<void>((resolve) => {
              const terminal = terminalRef.current;
              if (terminal) {
                terminal.write(data, resolve);
              } else {
                resolve();
              }
            }),
        },
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

  function selectSavedHost(hostId: (typeof SAVED_HOSTS)[number]["id"]) {
    const host = SAVED_HOSTS.find((item) => item.id === hostId);
    if (!host) return;

    const separator = host.target.lastIndexOf(":");
    setForm((current) => ({
      ...current,
      name: host.name,
      host: host.target.slice(0, separator),
      port: host.target.slice(separator + 1),
    }));
  }

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
          <button className="nav-item active" type="button">
            <NavIcon name="hosts" />
            Hosts
            <span className="nav-count">3</span>
          </button>
          <button className="nav-item" type="button" disabled>
            <NavIcon name="keys" />
            Keys
            <span className="coming-soon">Soon</span>
          </button>
          <button className="nav-item" type="button" disabled>
            <NavIcon name="scripts" />
            Scripts
            <span className="coming-soon">Soon</span>
          </button>
        </nav>

        <div className="section-heading">
          <span>Saved hosts</span>
          <button type="button" aria-label="Add host" disabled>
            +
          </button>
        </div>

        <div className="host-list">
          {SAVED_HOSTS.map((host, index) => (
            <button
              className={`host-card ${form.name === host.name ? "selected" : ""}`}
              key={host.id}
              onClick={() => selectSavedHost(host.id)}
              type="button"
            >
              <span className={`host-avatar ${host.tone}`}>
                {host.name.slice(0, 2)}
              </span>
              <span>
                <strong>{host.name}</strong>
                <small>{host.target}</small>
              </span>
              {index === 0 && (
                <span className="online-dot" title="Fixture available" />
              )}
            </button>
          ))}
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
            <p className="eyebrow">SSH workspace</p>
            <h1>{form.name || "New connection"}</h1>
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

        <div className="workspace-body">
          <section className="terminal-card" aria-label="SSH terminal">
            <div className="terminal-toolbar">
              <div className="window-controls" aria-hidden="true">
                <span />
                <span />
                <span />
              </div>
              <div className="terminal-title">
                <span>{form.username || "user"}@</span>
                {form.host || "host"}
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
                <h2>Open a session</h2>
              </div>
              <span className="protocol-badge">SSH</span>
            </div>

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
                    placeholder="Not stored in Phase 0"
                    type={passwordVisible ? "text" : "password"}
                    value={form.password}
                  />
                  <button
                    aria-label={
                      passwordVisible ? "Hide password" : "Show password"
                    }
                    onClick={() => setPasswordVisible((visible) => !visible)}
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

function NavIcon({ name }: { name: "hosts" | "keys" | "scripts" }) {
  const paths = {
    hosts: "M4 5.5h16v11H4zM8 19h8M12 16.5V19",
    keys: "M15.5 7.5a4 4 0 1 1-3.7 5.5L4 20.8V17h3v-3h3l1.8-1.8",
    scripts: "M7 3.5h8l3 3V20.5H7zM15 3.5v4h3M10 12h5M10 16h5",
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
