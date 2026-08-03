import { FormEvent, useState } from "react";
import type { VaultStatus } from "../../lib/vault-bridge";
import { LockIcon } from "../../shared/icons/ProductIcons";

interface VaultGateProps {
  error: string | null;
  onClearError(): void;
  onSubmit(pin: string): Promise<void>;
  status: VaultStatus | null;
}

export function VaultGate({
  error,
  onClearError,
  onSubmit,
  status,
}: VaultGateProps) {
  const [pin, setPin] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);
  const [showCreateForm, setShowCreateForm] = useState(false);

  const creating = status?.state === "uninitialized";
  const damaged = status?.state === "damaged";
  const showingWelcome = creating && !showCreateForm;

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (pin.length < 4) {
      setValidationError("Use at least 4 characters for the local PIN.");
      return;
    }
    if (creating && pin !== confirmation) {
      setValidationError("The two PIN entries do not match.");
      return;
    }

    setBusy(true);
    setValidationError(null);
    try {
      await onSubmit(pin);
    } finally {
      setPin("");
      setConfirmation("");
      setBusy(false);
    }
  }

  return (
    <main className="vault-gate">
      <section
        aria-labelledby="vault-gate-title"
        className={`vault-gate-card ${
          showingWelcome ? "vault-gate-card-welcome" : ""
        }`}
      >
        <div className="vault-gate-brand">
          <div className="brand-mark" aria-hidden="true">
            <span />
          </div>
          <div>
            <strong>AnySSH</strong>
            <small>Linux + Android</small>
          </div>
        </div>

        {!status && (
          <div className="vault-gate-content">
            <div className="vault-gate-icon">
              <LockIcon />
            </div>
            <p className="eyebrow">Local security</p>
            <h1 id="vault-gate-title">Preparing AnySSH…</h1>
            <p>Checking the local vault before opening your workspace.</p>
          </div>
        )}

        {damaged && (
          <div className="vault-gate-content">
            <div className="vault-gate-icon vault-gate-icon-danger">
              <LockIcon />
            </div>
            <p className="eyebrow danger-text">Recovery required</p>
            <h1 id="vault-gate-title">Your local vault needs attention</h1>
            <p>
              AnySSH found incomplete local data and stopped before overwriting
              it. Restore a backup or move the damaged files before continuing.
            </p>
            {error && <div className="vault-gate-error">{error}</div>}
          </div>
        )}

        {showingWelcome && (
          <div className="vault-gate-content vault-welcome">
            <div className="vault-welcome-hero">
              <div className="vault-gate-icon">
                <LockIcon />
              </div>
              <p className="eyebrow">Private by default</p>
              <h1 id="vault-gate-title">
                Your servers, securely within reach.
              </h1>
              <p className="vault-gate-intro">
                Manage hosts, credentials, and sessions from one workspace.
                Saved connection data stays protected locally.
              </p>
            </div>
            <div className="vault-welcome-benefits">
              <span>
                <LockIcon />
                Local protection
              </span>
              <span>
                <SessionIcon />
                Multi-session terminal
              </span>
              <span>
                <DeviceIcon />
                Linux and Android
              </span>
            </div>
            <button
              className="connect-button vault-primary-action"
              onClick={() => setShowCreateForm(true)}
              type="button"
            >
              <span>Get started</span>
              <span aria-hidden="true">→</span>
            </button>
            <span className="vault-welcome-caption">
              No account required · No host inventory uploaded
            </span>
          </div>
        )}

        {status && !damaged && !showingWelcome && (
          <div className="vault-gate-content">
            <div className="vault-gate-icon">
              <LockIcon />
            </div>
            <p className="eyebrow">
              {creating ? "First setup" : "Welcome back"}
            </p>
            <h1
              aria-label={
                creating ? "Create your encrypted Vault" : "Unlock AnySSH"
              }
              id="vault-gate-title"
            >
              {creating ? "Create your local vault" : "Unlock your workspace"}
            </h1>
            <p className="vault-gate-intro">
              {creating
                ? "Choose a PIN for this device. Hosts and credentials stay unreadable whenever the vault is locked."
                : "Enter your local PIN to continue to hosts and sessions."}
            </p>

            <form onSubmit={submit}>
              <label htmlFor="vault-pin">
                {creating ? "Create PIN" : "Local PIN"}
                <input
                  aria-label="PIN"
                  autoComplete="off"
                  autoFocus
                  id="vault-pin"
                  inputMode="numeric"
                  maxLength={1024}
                  onChange={(event) => {
                    setPin(event.target.value);
                    setValidationError(null);
                    onClearError();
                  }}
                  type="password"
                  value={pin}
                />
                {creating && (
                  <span className="vault-field-support">
                    At least 4 characters. This PIN is only for this device.
                  </span>
                )}
              </label>

              {creating && (
                <label htmlFor="vault-pin-confirmation">
                  Confirm PIN
                  <input
                    aria-label="Confirm PIN"
                    autoComplete="off"
                    id="vault-pin-confirmation"
                    inputMode="numeric"
                    maxLength={1024}
                    onChange={(event) => {
                      setConfirmation(event.target.value);
                      setValidationError(null);
                      onClearError();
                    }}
                    type="password"
                    value={confirmation}
                  />
                </label>
              )}

              {(validationError || error) && (
                <div className="vault-gate-error" role="alert">
                  {validationError || error}
                </div>
              )}

              <button
                aria-label={creating ? "Create encrypted Vault" : "Unlock"}
                className="connect-button vault-primary-action"
                disabled={busy}
                type="submit"
              >
                <span>
                  {busy
                    ? creating
                      ? "Creating…"
                      : "Unlocking…"
                    : creating
                      ? "Create and continue"
                      : "Unlock"}
                </span>
                <span aria-hidden="true">→</span>
              </button>
            </form>

            <div className="vault-gate-footnote">
              <LockIcon />
              <span>
                No account required. Saved connection data stays protected on
                this device.
              </span>
            </div>
          </div>
        )}
      </section>
    </main>
  );
}

function SessionIcon() {
  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 24 24">
      <path
        d="M4 5h16v14H4zM7.5 9l3 3-3 3M12.5 15H17"
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth="1.7"
      />
    </svg>
  );
}

function DeviceIcon() {
  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 24 24">
      <path
        d="M3.5 5.5h11v9h-11zM7 18h4m-2-3.5V18m8-9h3.5v10H17z"
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth="1.7"
      />
    </svg>
  );
}
