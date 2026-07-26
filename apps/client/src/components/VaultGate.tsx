import { FormEvent, useState } from "react";
import type { VaultStatus } from "../lib/vault-bridge";

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

  const creating = status?.state === "uninitialized";
  const damaged = status?.state === "damaged";

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (pin.length < 4) {
      setValidationError("PIN must contain at least 4 characters.");
      return;
    }
    if (creating && pin !== confirmation) {
      setValidationError("PIN confirmation does not match.");
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
      <section aria-labelledby="vault-gate-title" className="vault-gate-card">
        <div className="vault-gate-brand">
          <div className="brand-mark" aria-hidden="true">
            <span />
          </div>
          <div>
            <strong>AnySSH</strong>
            <small>Encrypted local vault</small>
          </div>
        </div>

        <div className="vault-gate-icon">
          <VaultLockIcon />
        </div>

        {!status && (
          <>
            <p className="eyebrow">Vault</p>
            <h1 id="vault-gate-title">Checking local security…</h1>
            <p>Inspecting the encrypted Vault bootstrap.</p>
          </>
        )}

        {damaged && (
          <>
            <p className="eyebrow danger-text">Recovery required</p>
            <h1 id="vault-gate-title">Vault files are incomplete</h1>
            <p>
              AnySSH will not overwrite this Vault. Restore a backup or move the
              damaged files before creating a replacement.
            </p>
            {error && <div className="vault-gate-error">{error}</div>}
          </>
        )}

        {status && !damaged && (
          <>
            <p className="eyebrow">{creating ? "First launch" : "Locked"}</p>
            <h1 id="vault-gate-title">
              {creating ? "Create your encrypted Vault" : "Unlock AnySSH"}
            </h1>
            <p>
              {creating
                ? "Your PIN protects a random master key. It is never used directly as the database key."
                : "Enter your local PIN to unlock the SQLCipher database."}
            </p>

            <form onSubmit={submit}>
              <label htmlFor="vault-pin">
                PIN
                <input
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
              </label>

              {creating && (
                <label htmlFor="vault-pin-confirmation">
                  Confirm PIN
                  <input
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

              <button className="connect-button" disabled={busy} type="submit">
                <span>
                  {busy
                    ? creating
                      ? "Creating Vault…"
                      : "Unlocking…"
                    : creating
                      ? "Create encrypted Vault"
                      : "Unlock"}
                </span>
                <span aria-hidden="true">→</span>
              </button>
            </form>

            <div className="vault-gate-security">
              <span>Argon2id</span>
              <span>XChaCha20-Poly1305</span>
              <span>SQLCipher</span>
            </div>
          </>
        )}
      </section>
    </main>
  );
}

function VaultLockIcon() {
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
