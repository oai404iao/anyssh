import { type FormEvent, useState } from "react";
import {
  createKeyboardInteractiveCredential,
  createPasswordCredential,
  createSystemAgentCredential,
  credentialOperationsUseNativeRuntime,
  deleteCredential,
  exportPrivateKeyCredential,
  generatePrivateKeyCredential,
  getPrivateKeyPublicSummary,
  importPrivateKeyCredential,
  listSystemAgentIdentities,
  updateKeyboardInteractiveCredential,
  updatePasswordCredential,
  type CredentialSummary,
  type PrivateKeyGenerationAlgorithm,
  type PrivateKeyPublicSummary,
  type SystemAgentIdentitySummary,
} from "../../lib/credential-bridge";
import {
  EditorActions,
  EditorDialog,
  ManagerEmpty,
  ManagerError,
  ManagerShell,
  type ManagerProps,
} from "../configuration/ManagerPrimitives";
import { credentialKindLabel } from "./credential-labels";

interface CredentialManagerProps extends ManagerProps {
  credentials: CredentialSummary[];
}

type CredentialDraft =
  | {
      kind: "password";
      credentialId: string | null;
      label: string;
      username: string;
      password: string;
    }
  | {
      kind: "privateKey";
      label: string;
      username: string;
    }
  | {
      kind: "generatedPrivateKey";
      label: string;
      username: string;
      algorithm: PrivateKeyGenerationAlgorithm;
    }
  | {
      kind: "systemAgent";
      label: string;
      username: string;
      identityFingerprintSha256: string;
    }
  | {
      kind: "keyboardInteractive";
      credentialId: string | null;
      label: string;
      username: string;
    };

export function CredentialWorkspace({
  credentials,
  loading,
  onChanged,
}: CredentialManagerProps) {
  const [draft, setDraft] = useState<CredentialDraft | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [agentIdentities, setAgentIdentities] = useState<
    SystemAgentIdentitySummary[]
  >([]);
  const [agentLoading, setAgentLoading] = useState(false);
  const [publicKey, setPublicKey] = useState<PrivateKeyPublicSummary | null>(
    null,
  );
  const [publicKeyBusyId, setPublicKeyBusyId] = useState<string | null>(null);
  const [publicKeyCopied, setPublicKeyCopied] = useState(false);
  const [exportBusyId, setExportBusyId] = useState<string | null>(null);

  function closeEditor() {
    setDraft(null);
    setError(null);
  }

  function editPassword(credential: CredentialSummary) {
    setError(null);
    setNotice(null);
    setDraft({
      kind: "password",
      credentialId: credential.id,
      label: credential.label,
      username: credential.username,
      password: "",
    });
  }

  function editKeyboardInteractive(credential: CredentialSummary) {
    setError(null);
    setNotice(null);
    setDraft({
      kind: "keyboardInteractive",
      credentialId: credential.id,
      label: credential.label,
      username: credential.username,
    });
  }

  async function openSystemAgentEditor() {
    setError(null);
    setNotice(null);
    setAgentIdentities([]);
    setAgentLoading(true);
    setDraft({
      kind: "systemAgent",
      label: "",
      username: "",
      identityFingerprintSha256: "",
    });
    try {
      const identities = await listSystemAgentIdentities();
      setAgentIdentities(identities);
      if (identities.length === 0) {
        setNotice("The System SSH Agent has no public-key identities.");
      } else {
        setDraft((current) =>
          current?.kind === "systemAgent"
            ? {
                ...current,
                identityFingerprintSha256:
                  identities[0]?.fingerprintSha256 ?? "",
              }
            : current,
        );
      }
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setAgentLoading(false);
    }
  }

  async function saveCredential(event: FormEvent) {
    event.preventDefault();
    if (!draft) return;
    if (!draft.label.trim() || !draft.username.trim()) {
      setError("Label and Username are required.");
      return;
    }
    if (draft.kind === "password" && !draft.password) {
      setError("Password is required and is never returned by the repository.");
      return;
    }
    if (draft.kind === "systemAgent" && !draft.identityFingerprintSha256) {
      setError("Select an SSH Agent identity.");
      return;
    }

    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      if (draft.kind === "privateKey") {
        const imported = await importPrivateKeyCredential({
          label: draft.label,
          username: draft.username,
        });
        if (!imported) {
          setNotice("Private Key selection was cancelled.");
          return;
        }
      } else if (draft.kind === "generatedPrivateKey") {
        await generatePrivateKeyCredential({
          label: draft.label,
          username: draft.username,
          algorithm: draft.algorithm,
        });
      } else if (draft.kind === "systemAgent") {
        await createSystemAgentCredential({
          label: draft.label,
          username: draft.username,
          identityFingerprintSha256: draft.identityFingerprintSha256,
        });
      } else if (draft.kind === "keyboardInteractive") {
        if (draft.credentialId) {
          await updateKeyboardInteractiveCredential({
            credentialId: draft.credentialId,
            label: draft.label,
            username: draft.username,
          });
        } else {
          await createKeyboardInteractiveCredential({
            label: draft.label,
            username: draft.username,
          });
        }
      } else if (draft.credentialId) {
        await updatePasswordCredential({
          credentialId: draft.credentialId,
          label: draft.label,
          username: draft.username,
          password: draft.password,
        });
      } else {
        await createPasswordCredential({
          label: draft.label,
          username: draft.username,
          password: draft.password,
        });
      }
      await onChanged();
      setDraft(null);
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      if (draft.kind === "password") {
        setDraft((current) =>
          current?.kind === "password" ? { ...current, password: "" } : current,
        );
      }
      setBusy(false);
    }
  }

  async function showPublicKey(credentialId: string) {
    setPublicKeyBusyId(credentialId);
    setPublicKeyCopied(false);
    setError(null);
    setNotice(null);
    try {
      setPublicKey(await getPrivateKeyPublicSummary(credentialId));
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setPublicKeyBusyId(null);
    }
  }

  async function copyPublicKey() {
    if (!publicKey) return;
    try {
      await navigator.clipboard.writeText(publicKey.opensshPublicKey);
      setPublicKeyCopied(true);
    } catch {
      setPublicKeyCopied(false);
      setError(
        "Clipboard access was unavailable. Select the Public Key text and copy it manually.",
      );
    }
  }

  async function exportPrivateKey(credentialId: string) {
    setError(null);
    setNotice(null);
    if (!credentialOperationsUseNativeRuntime()) {
      setNotice(
        "Encrypted Private Key export is available in the native AnySSH runtime. Browser QA writes no file.",
      );
      return;
    }

    setExportBusyId(credentialId);
    try {
      const exported = await exportPrivateKeyCredential(credentialId);
      if (exported) {
        setNotice(
          `Encrypted ${exported.algorithm} Private Key exported to “${exported.fileName}”.`,
        );
      } else {
        setNotice("Encrypted Private Key export was cancelled.");
      }
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setExportBusyId(null);
    }
  }

  async function removeCredential(credentialId: string) {
    if (confirmDelete !== credentialId) {
      setConfirmDelete(credentialId);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await deleteCredential(credentialId);
      await onChanged();
      setConfirmDelete(null);
    } catch (operationError) {
      setError(String(operationError));
      setConfirmDelete(null);
    } finally {
      setBusy(false);
    }
  }

  return (
    <ManagerShell
      action={
        <div className="manager-actions">
          <button
            className="secondary-button compact-button"
            onClick={() => {
              setError(null);
              setNotice(null);
              setDraft({
                kind: "generatedPrivateKey",
                label: "",
                username: "",
                algorithm: "ed25519",
              });
            }}
            type="button"
          >
            Generate key
          </button>
          <button
            className="secondary-button compact-button"
            onClick={() => {
              setError(null);
              setNotice(null);
              setDraft({
                kind: "keyboardInteractive",
                credentialId: null,
                label: "",
                username: "",
              });
            }}
            type="button"
          >
            New interactive
          </button>
          <button
            className="secondary-button compact-button"
            onClick={() => void openSystemAgentEditor()}
            type="button"
          >
            New system agent
          </button>
          <button
            className="secondary-button compact-button"
            onClick={() => {
              setError(null);
              setNotice(null);
              setDraft({
                kind: "privateKey",
                label: "",
                username: "",
              });
            }}
            type="button"
          >
            Import private key
          </button>
          <button
            className="connect-button compact-button"
            onClick={() => {
              setError(null);
              setNotice(null);
              setDraft({
                kind: "password",
                credentialId: null,
                label: "",
                username: "",
                password: "",
              });
            }}
            type="button"
          >
            New password
          </button>
        </div>
      }
      description="Secrets stay encrypted in the Vault; list responses contain metadata only."
      eyebrow="Authentication"
      title="Credentials"
    >
      {error && <ManagerError message={error} />}
      {notice && <div className="manager-notice">{notice}</div>}
      {loading ? (
        <ManagerEmpty>Loading Credentials…</ManagerEmpty>
      ) : credentials.length === 0 ? (
        <ManagerEmpty>No Credentials yet.</ManagerEmpty>
      ) : (
        <div className="resource-list">
          {credentials.map((credential) => (
            <article className="resource-card" key={credential.id}>
              <div className={`resource-icon ${credential.kind}`}>
                {credential.kind === "privateKey"
                  ? "PK"
                  : credential.kind === "systemAgent"
                    ? "AG"
                    : credential.kind === "keyboardInteractive"
                      ? "KI"
                      : "PW"}
              </div>
              <div className="resource-main">
                <strong>{credential.label}</strong>
                <span>{credential.username}</span>
                <div className="resource-tags">
                  <span>{credentialKindLabel(credential.kind)}</span>
                  <span>
                    {credential.kind === "systemAgent"
                      ? "External signer"
                      : credential.kind === "keyboardInteractive"
                        ? "Responses are session-only"
                        : "Secret hidden"}
                  </span>
                </div>
              </div>
              <div className="resource-actions">
                {credential.kind === "privateKey" && (
                  <>
                    <button
                      disabled={
                        busy ||
                        publicKeyBusyId !== null ||
                        exportBusyId !== null
                      }
                      onClick={() => void showPublicKey(credential.id)}
                      type="button"
                    >
                      {publicKeyBusyId === credential.id
                        ? "Reading public key…"
                        : "Public key"}
                    </button>
                    <button
                      disabled={
                        busy ||
                        publicKeyBusyId !== null ||
                        exportBusyId !== null
                      }
                      onClick={() => void exportPrivateKey(credential.id)}
                      type="button"
                    >
                      {exportBusyId === credential.id
                        ? "Waiting for native prompts…"
                        : "Export encrypted…"}
                    </button>
                  </>
                )}
                {credential.kind === "password" && (
                  <button
                    onClick={() => editPassword(credential)}
                    type="button"
                  >
                    Replace password
                  </button>
                )}
                {credential.kind === "keyboardInteractive" && (
                  <button
                    onClick={() => editKeyboardInteractive(credential)}
                    type="button"
                  >
                    Edit metadata
                  </button>
                )}
                <button
                  className={
                    confirmDelete === credential.id ? "danger-action" : ""
                  }
                  disabled={busy}
                  onClick={() => void removeCredential(credential.id)}
                  type="button"
                >
                  {confirmDelete === credential.id
                    ? "Confirm delete"
                    : "Delete"}
                </button>
              </div>
            </article>
          ))}
        </div>
      )}

      {draft && (
        <EditorDialog
          closeDisabled={busy}
          onClose={closeEditor}
          title={
            draft.kind === "privateKey"
              ? "Import Private Key"
              : draft.kind === "generatedPrivateKey"
                ? "Generate Private Key"
                : draft.kind === "systemAgent"
                  ? "New System Agent Credential"
                  : draft.kind === "keyboardInteractive"
                    ? draft.credentialId
                      ? "Edit Interactive Credential"
                      : "New Interactive Credential"
                    : draft.credentialId
                      ? "Replace Password"
                      : "New Password Credential"
          }
        >
          <form className="editor-form" onSubmit={saveCredential}>
            <label>
              Credential label
              <input
                autoFocus
                onChange={(event) =>
                  setDraft((current) =>
                    current
                      ? { ...current, label: event.target.value }
                      : current,
                  )
                }
                value={draft.label}
              />
            </label>
            <label>
              Username
              <input
                autoCapitalize="none"
                autoComplete="username"
                onChange={(event) =>
                  setDraft((current) =>
                    current
                      ? { ...current, username: event.target.value }
                      : current,
                  )
                }
                value={draft.username}
              />
            </label>
            {draft.kind === "password" ? (
              <label>
                Password
                <input
                  autoComplete="new-password"
                  onChange={(event) =>
                    setDraft((current) =>
                      current?.kind === "password"
                        ? { ...current, password: event.target.value }
                        : current,
                    )
                  }
                  type="password"
                  value={draft.password}
                />
              </label>
            ) : draft.kind === "privateKey" ? (
              <div className="security-note">
                <strong>Rust-owned file import</strong>
                <p>
                  The native picker opens after you continue. File path and Key
                  content never enter the WebView. On supported desktop
                  platforms, encrypted Keys use an OS-native Passphrase prompt
                  that also stays outside the WebView.
                </p>
              </div>
            ) : draft.kind === "generatedPrivateKey" ? (
              <>
                <label>
                  Algorithm
                  <select
                    onChange={(event) =>
                      setDraft((current) =>
                        current?.kind === "generatedPrivateKey"
                          ? {
                              ...current,
                              algorithm: event.target
                                .value as PrivateKeyGenerationAlgorithm,
                            }
                          : current,
                      )
                    }
                    value={draft.algorithm}
                  >
                    <option value="ed25519">Ed25519 (recommended)</option>
                    <option value="rsa4096">RSA 4096</option>
                  </select>
                </label>
                <div className="security-note">
                  <strong>Rust-owned generation</strong>
                  <p>
                    AnySSH generates this Key with the Rust CSPRNG and stores it
                    directly in the encrypted Vault. Private Key material never
                    enters the WebView.
                  </p>
                </div>
              </>
            ) : draft.kind === "systemAgent" ? (
              <>
                <label>
                  SSH Agent identity
                  <select
                    disabled={agentLoading || agentIdentities.length === 0}
                    onChange={(event) =>
                      setDraft((current) =>
                        current?.kind === "systemAgent"
                          ? {
                              ...current,
                              identityFingerprintSha256: event.target.value,
                            }
                          : current,
                      )
                    }
                    value={draft.identityFingerprintSha256}
                  >
                    {agentLoading && (
                      <option value="">Loading identities…</option>
                    )}
                    {!agentLoading && agentIdentities.length === 0 && (
                      <option value="">No identities available</option>
                    )}
                    {agentIdentities.map((identity) => (
                      <option
                        key={identity.fingerprintSha256}
                        value={identity.fingerprintSha256}
                      >
                        {identity.algorithm} · {identity.fingerprintSha256}
                        {identity.comment ? ` · ${identity.comment}` : ""}
                      </option>
                    ))}
                  </select>
                </label>
                <div className="security-note">
                  <strong>External signing only</strong>
                  <p>
                    AnySSH stores the selected SHA-256 fingerprint and username.
                    The system Agent keeps the Private Key and performs each
                    signature.
                  </p>
                </div>
              </>
            ) : (
              <div className="security-note">
                <strong>Session-only responses</strong>
                <p>
                  AnySSH stores only this label and username. Verification
                  codes, challenge responses, OTP seeds, and prompt rules are
                  never saved.
                </p>
              </div>
            )}
            {error && <ManagerError message={error} />}
            <EditorActions
              busy={busy}
              busyLabel={
                draft.kind === "generatedPrivateKey" ? "Generating…" : undefined
              }
              submitLabel={
                draft.kind === "privateKey"
                  ? "Choose private key"
                  : draft.kind === "generatedPrivateKey"
                    ? "Generate key"
                    : draft.kind === "systemAgent"
                      ? "Save Agent Credential"
                      : draft.kind === "keyboardInteractive"
                        ? "Save Interactive Credential"
                        : "Save Credential"
              }
            />
          </form>
        </EditorDialog>
      )}

      {publicKey && (
        <EditorDialog
          onClose={() => {
            setPublicKey(null);
            setPublicKeyCopied(false);
          }}
          title="Public Key"
        >
          <div className="public-key-details">
            <div>
              <span>Algorithm</span>
              <code>{publicKey.algorithm}</code>
            </div>
            <div>
              <span>SHA-256 fingerprint</span>
              <code>{publicKey.fingerprintSha256}</code>
            </div>
            <label>
              OpenSSH Public Key
              <textarea
                aria-label="OpenSSH Public Key"
                readOnly
                rows={5}
                value={publicKey.opensshPublicKey}
              />
            </label>
            <div className="editor-actions public-key-actions">
              <button
                className="connect-button"
                onClick={() => void copyPublicKey()}
                type="button"
              >
                {publicKeyCopied ? "Public key copied" : "Copy public key"}
              </button>
            </div>
            <div className="security-note">
              <strong>Public material only</strong>
              <p>
                This dialog contains the deployable Public Key. The Private Key
                and its stored Passphrase remain inside Rust and the encrypted
                Vault.
              </p>
            </div>
          </div>
        </EditorDialog>
      )}
    </ManagerShell>
  );
}
