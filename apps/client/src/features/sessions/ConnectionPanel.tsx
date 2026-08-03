import type { FormEvent } from "react";
import type { CredentialSummary } from "../../lib/credential-bridge";
import type { HostSummary, JumpRouteSummary } from "../../lib/host-bridge";
import type {
  SshPortForwardKind,
  SshPortForwardSummary,
} from "../../lib/ssh-bridge";
import { LockIcon } from "../../shared/icons/ProductIcons";
import {
  formatForwardEndpoint,
  type ConnectionForm,
  type PortForwardForm,
} from "./session-model";

type StateUpdater<T> = T | ((current: T) => T);

export interface ConnectionPanelProps {
  busy: boolean;
  connected: boolean;
  error: string | null;
  form: ConnectionForm;
  nativeRuntime: boolean;
  onConnect(event: FormEvent<HTMLFormElement>): void;
  onFormChange(update: StateUpdater<ConnectionForm>): void;
  onPasswordVisibleChange(update: StateUpdater<boolean>): void;
  onPortForwardFormChange(update: StateUpdater<PortForwardForm>): void;
  onStartPortForward(event: FormEvent<HTMLFormElement>): void;
  onStopPortForward(forwardId: string): Promise<void>;
  onUseQuickConnection(): void;
  passwordVisible: boolean;
  portForwardBusy: boolean;
  portForwardError: string | null;
  portForwardForm: PortForwardForm;
  portForwards: SshPortForwardSummary[];
  selectedCredential: CredentialSummary | null;
  selectedRoute: JumpRouteSummary | null;
  selectedSavedHost: HostSummary | null;
  statusDetail: string;
  statusLabel: string;
  statusTone: string;
}

export function ConnectionPanel({
  busy,
  connected,
  error,
  form,
  nativeRuntime,
  onConnect,
  onFormChange,
  onPasswordVisibleChange,
  onPortForwardFormChange,
  onStartPortForward,
  onStopPortForward,
  onUseQuickConnection,
  passwordVisible,
  portForwardBusy,
  portForwardError,
  portForwardForm,
  portForwards,
  selectedCredential,
  selectedRoute,
  selectedSavedHost,
  statusDetail,
  statusLabel,
  statusTone,
}: ConnectionPanelProps) {
  return (
    <aside className="connection-panel">
      <div className="panel-heading">
        <div>
          <p className="eyebrow">Connection</p>
          <h2>{selectedSavedHost ? "Saved Host" : "Open a session"}</h2>
        </div>
        <span className="protocol-badge">SSH</span>
      </div>

      {selectedSavedHost ? (
        <form onSubmit={onConnect}>
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
              busy || connected || !selectedSavedHost.effectiveCredentialId
            }
            type="submit"
          >
            <span>
              {busy
                ? "Connecting…"
                : connected
                  ? "Session active"
                  : nativeRuntime
                    ? "Connect saved Host"
                    : "Native runtime required"}
            </span>
            <span aria-hidden="true">↗</span>
          </button>
          <button
            className="secondary-button full-width-button"
            disabled={busy || connected}
            onClick={onUseQuickConnection}
            type="button"
          >
            Use quick connection
          </button>
        </form>
      ) : (
        <form onSubmit={onConnect}>
          <label>
            Display name
            <input
              autoComplete="off"
              onChange={(event) =>
                onFormChange((current) => ({
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
                  onFormChange((current) => ({
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
                max="65535"
                min="1"
                onChange={(event) =>
                  onFormChange((current) => ({
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
                  onFormChange((current) => ({
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
                  onFormChange((current) => ({
                    ...current,
                    authenticationKind,
                    password:
                      authenticationKind === "password" ? current.password : "",
                  }));
                  onPasswordVisibleChange(false);
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
                    onFormChange((current) => ({
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
                  onClick={() => onPasswordVisibleChange((visible) => !visible)}
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
                AnySSH sends only the responses requested by the SSH server.
                Responses are cleared after each round and are never saved.
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
              {busy ? "Connecting…" : connected ? "Session active" : "Connect"}
            </span>
            <span aria-hidden="true">↗</span>
          </button>
        </form>
      )}

      <section
        aria-labelledby="port-forwarding-title"
        className="forwarding-panel"
        id="port-forwarding-panel"
      >
        <div className="forwarding-heading">
          <div>
            <p className="eyebrow">Session scoped</p>
            <h3 id="port-forwarding-title">Port forwarding</h3>
          </div>
          <span>{portForwards.length}/16</span>
        </div>
        <form className="forwarding-form" onSubmit={onStartPortForward}>
          <label>
            Type
            <select
              aria-label="Port forward type"
              onChange={(event) =>
                onPortForwardFormChange((current) => ({
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
              {portForwardForm.kind === "remote" ? "Server bind" : "Local bind"}
              <select
                aria-label="Port forward bind host"
                onChange={(event) =>
                  onPortForwardFormChange((current) => ({
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
                  onPortForwardFormChange((current) => ({
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
                    onPortForwardFormChange((current) => ({
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
                    onPortForwardFormChange((current) => ({
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
            Loopback only. Payloads stay in Rust and are never sent through the
            WebView.
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
          <ul aria-label="Active port forwards" className="forwarding-list">
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
                    {formatForwardEndpoint(forward.bindHost, forward.boundPort)}
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
                  onClick={() => void onStopPortForward(forward.id)}
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
          <strong>{statusLabel}</strong>
          <p>{statusDetail}</p>
        </div>
      </div>
    </aside>
  );
}
