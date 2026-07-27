import { FormEvent, useMemo, useState } from "react";
import {
  createPasswordCredential,
  deleteCredential,
  importPrivateKeyCredential,
  updatePasswordCredential,
  type CredentialSummary,
} from "../lib/credential-bridge";
import {
  createGroup,
  createHost,
  createJumpRoute,
  deleteGroup,
  deleteHost,
  deleteJumpRoute,
  updateGroup,
  updateHost,
  updateJumpRoute,
  type GroupSummary,
  type HostSummary,
  type JumpRouteSummary,
  type ReferenceOverride,
} from "../lib/host-bridge";

export type ConfigurationSection =
  "groups" | "hosts" | "credentials" | "routes";

interface ConfigurationWorkspaceProps {
  section: ConfigurationSection;
  groups: GroupSummary[];
  hosts: HostSummary[];
  credentials: CredentialSummary[];
  routes: JumpRouteSummary[];
  loading: boolean;
  loadError: string | null;
  onChanged(): Promise<void>;
  onOpenHost(host: HostSummary): void;
}

export function ConfigurationWorkspace({
  section,
  groups,
  hosts,
  credentials,
  routes,
  loading,
  loadError,
  onChanged,
  onOpenHost,
}: ConfigurationWorkspaceProps) {
  return (
    <div className="configuration-body">
      {loadError && (
        <div className="manager-error" role="alert">
          {loadError}
        </div>
      )}
      {section === "hosts" && (
        <HostManager
          credentials={credentials}
          groups={groups}
          hosts={hosts}
          loading={loading}
          onChanged={onChanged}
          onOpenHost={onOpenHost}
          routes={routes}
        />
      )}
      {section === "groups" && (
        <GroupManager
          credentials={credentials}
          groups={groups}
          loading={loading}
          onChanged={onChanged}
          routes={routes}
        />
      )}
      {section === "credentials" && (
        <CredentialManager
          credentials={credentials}
          loading={loading}
          onChanged={onChanged}
        />
      )}
      {section === "routes" && (
        <RouteManager
          hosts={hosts}
          loading={loading}
          onChanged={onChanged}
          routes={routes}
        />
      )}
    </div>
  );
}

interface ManagerProps {
  loading: boolean;
  onChanged(): Promise<void>;
}

interface GroupManagerProps extends ManagerProps {
  groups: GroupSummary[];
  credentials: CredentialSummary[];
  routes: JumpRouteSummary[];
}

interface GroupDraft {
  groupId: string | null;
  label: string;
  parentGroupId: string;
  credentialOverride: ReferenceOverride;
  jumpRouteOverride: ReferenceOverride;
}

const EMPTY_GROUP_DRAFT: GroupDraft = {
  groupId: null,
  label: "",
  parentGroupId: "",
  credentialOverride: { kind: "inherit" },
  jumpRouteOverride: { kind: "inherit" },
};

function GroupManager({
  groups,
  credentials,
  routes,
  loading,
  onChanged,
}: GroupManagerProps) {
  const [draft, setDraft] = useState<GroupDraft | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const orderedGroups = useMemo(() => flattenGroupTree(groups), [groups]);
  const groupLabels = useMemo(
    () => new Map(groups.map((group) => [group.id, group])),
    [groups],
  );
  const credentialLabels = useMemo(
    () => new Map(credentials.map((credential) => [credential.id, credential])),
    [credentials],
  );
  const routeLabels = useMemo(
    () => new Map(routes.map((route) => [route.id, route])),
    [routes],
  );

  function editGroup(group: GroupSummary) {
    setError(null);
    setDraft({
      groupId: group.id,
      label: group.label,
      parentGroupId: group.parentGroupId ?? "",
      credentialOverride: cloneReferenceOverride(group.credentialOverride),
      jumpRouteOverride: cloneReferenceOverride(group.jumpRouteOverride),
    });
  }

  async function saveGroup(event: FormEvent) {
    event.preventDefault();
    if (!draft) return;
    if (!draft.label.trim()) {
      setError("Group label is required.");
      return;
    }
    if (
      !overrideHasSelection(draft.credentialOverride) ||
      !overrideHasSelection(draft.jumpRouteOverride)
    ) {
      setError("Set overrides require a selected Credential or Jump Route.");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      if (draft.groupId) {
        await updateGroup({
          groupId: draft.groupId,
          label: draft.label,
          parentGroupId: draft.parentGroupId || null,
          credentialOverride: draft.credentialOverride,
          jumpRouteOverride: draft.jumpRouteOverride,
        });
      } else {
        await createGroup({
          label: draft.label,
          parentGroupId: draft.parentGroupId || null,
          credentialOverride: draft.credentialOverride,
          jumpRouteOverride: draft.jumpRouteOverride,
        });
      }
      await onChanged();
      setDraft(null);
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function removeGroup(groupId: string) {
    if (confirmDelete !== groupId) {
      setConfirmDelete(groupId);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await deleteGroup(groupId);
      await onChanged();
      setConfirmDelete(null);
    } catch (operationError) {
      setError(String(operationError));
      setConfirmDelete(null);
    } finally {
      setBusy(false);
    }
  }

  const unavailableParents = draft?.groupId
    ? descendantGroupIds(groups, draft.groupId)
    : new Set<string>();

  return (
    <ManagerShell
      action={
        <button
          className="connect-button compact-button"
          onClick={() => {
            setError(null);
            setDraft({
              ...EMPTY_GROUP_DRAFT,
              credentialOverride: { kind: "inherit" },
              jumpRouteOverride: { kind: "inherit" },
            });
          }}
          type="button"
        >
          New group
        </button>
      }
      description="Groups form a bounded tree and preserve explicit Inherit, Set, and Clear state."
      eyebrow="Organization"
      title="Groups"
    >
      {error && <ManagerError message={error} />}
      {loading ? (
        <ManagerEmpty>Loading Groups…</ManagerEmpty>
      ) : groups.length === 0 ? (
        <ManagerEmpty>No Groups yet.</ManagerEmpty>
      ) : (
        <div className="resource-list group-tree">
          {orderedGroups.map(({ group, depth }) => {
            const credential = group.effectiveCredentialId
              ? credentialLabels.get(group.effectiveCredentialId)
              : null;
            const route = group.effectiveJumpRouteId
              ? routeLabels.get(group.effectiveJumpRouteId)
              : null;
            return (
              <article
                className="resource-card group-card"
                key={group.id}
                style={{ marginInlineStart: `${Math.min(depth, 4) * 12}px` }}
              >
                <div className="resource-icon group-resource-icon">
                  {depth + 1}
                </div>
                <div className="resource-main">
                  <strong>{group.label}</strong>
                  <span>
                    {group.parentGroupId
                      ? `Child of ${groupLabels.get(group.parentGroupId)?.label ?? group.parentGroupId}`
                      : "Root Group"}
                  </span>
                  <div className="resource-tags">
                    <span>
                      {credential
                        ? `${credential.label} · ${overrideStateLabel(group.credentialOverride)}`
                        : `No Credential · ${overrideStateLabel(group.credentialOverride)}`}
                    </span>
                    <span>
                      {route
                        ? `${route.label} · ${overrideStateLabel(group.jumpRouteOverride)}`
                        : `Direct · ${overrideStateLabel(group.jumpRouteOverride)}`}
                    </span>
                  </div>
                </div>
                <div className="resource-actions">
                  <button onClick={() => editGroup(group)} type="button">
                    Edit
                  </button>
                  <button
                    className={
                      confirmDelete === group.id ? "danger-action" : ""
                    }
                    disabled={busy}
                    onClick={() => void removeGroup(group.id)}
                    type="button"
                  >
                    {confirmDelete === group.id ? "Confirm delete" : "Delete"}
                  </button>
                </div>
              </article>
            );
          })}
        </div>
      )}

      {draft && (
        <EditorDialog
          onClose={() => {
            setDraft(null);
            setError(null);
          }}
          title={draft.groupId ? "Edit Group" : "New Group"}
        >
          <form className="editor-form" onSubmit={saveGroup}>
            <label>
              Group label
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
              Parent Group
              <select
                onChange={(event) =>
                  setDraft((current) =>
                    current
                      ? { ...current, parentGroupId: event.target.value }
                      : current,
                  )
                }
                value={draft.parentGroupId}
              >
                <option value="">Root Group</option>
                {groups
                  .filter((group) => !unavailableParents.has(group.id))
                  .map((group) => (
                    <option key={group.id} value={group.id}>
                      {group.label}
                    </option>
                  ))}
              </select>
            </label>
            <ReferenceOverrideEditor
              inheritLabel="Inherit parent Credential"
              label="Credential"
              onChange={(credentialOverride) =>
                setDraft((current) =>
                  current ? { ...current, credentialOverride } : current,
                )
              }
              options={credentials.map((credential) => ({
                id: credential.id,
                label: `${credential.label} · ${credential.username}`,
              }))}
              setLabel="Set Credential"
              value={draft.credentialOverride}
            />
            <ReferenceOverrideEditor
              inheritLabel="Inherit parent Jump Route"
              label="Jump Route"
              onChange={(jumpRouteOverride) =>
                setDraft((current) =>
                  current ? { ...current, jumpRouteOverride } : current,
                )
              }
              options={routes.map((route) => ({
                id: route.id,
                label: route.label,
              }))}
              setLabel="Set Jump Route"
              value={draft.jumpRouteOverride}
            />
            {error && <ManagerError message={error} />}
            <EditorActions busy={busy} submitLabel="Save Group" />
          </form>
        </EditorDialog>
      )}
    </ManagerShell>
  );
}

interface HostManagerProps extends ManagerProps {
  groups: GroupSummary[];
  hosts: HostSummary[];
  credentials: CredentialSummary[];
  routes: JumpRouteSummary[];
  onOpenHost(host: HostSummary): void;
}

interface HostDraft {
  hostId: string | null;
  displayName: string;
  host: string;
  port: string;
  groupId: string;
  credentialOverride: ReferenceOverride;
  jumpRouteOverride: ReferenceOverride;
}

const EMPTY_HOST_DRAFT: HostDraft = {
  hostId: null,
  displayName: "",
  host: "",
  port: "22",
  groupId: "",
  credentialOverride: { kind: "inherit" },
  jumpRouteOverride: { kind: "inherit" },
};

function HostManager({
  hosts,
  groups,
  credentials,
  routes,
  loading,
  onChanged,
  onOpenHost,
}: HostManagerProps) {
  const [draft, setDraft] = useState<HostDraft | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const credentialLabels = useMemo(
    () => new Map(credentials.map((credential) => [credential.id, credential])),
    [credentials],
  );
  const routeLabels = useMemo(
    () => new Map(routes.map((route) => [route.id, route])),
    [routes],
  );
  const groupLabels = useMemo(
    () => new Map(groups.map((group) => [group.id, group])),
    [groups],
  );

  function editHost(host: HostSummary) {
    setError(null);
    setDraft({
      hostId: host.id,
      displayName: host.displayName,
      host: host.host,
      port: String(host.port),
      groupId: host.groupId ?? "",
      credentialOverride: cloneReferenceOverride(host.credentialOverride),
      jumpRouteOverride: cloneReferenceOverride(host.jumpRouteOverride),
    });
  }

  async function saveHost(event: FormEvent) {
    event.preventDefault();
    if (!draft) return;
    const port = Number(draft.port);
    if (
      !draft.displayName.trim() ||
      !draft.host.trim() ||
      !Number.isInteger(port) ||
      port < 1 ||
      port > 65535
    ) {
      setError("Display name, Host, and a valid port are required.");
      return;
    }
    if (
      !overrideHasSelection(draft.credentialOverride) ||
      !overrideHasSelection(draft.jumpRouteOverride)
    ) {
      setError("Set overrides require a selected Credential or Jump Route.");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      if (draft.hostId) {
        await updateHost({
          hostId: draft.hostId,
          displayName: draft.displayName,
          host: draft.host,
          port,
          groupId: draft.groupId || null,
          credentialOverride: draft.credentialOverride,
          jumpRouteOverride: draft.jumpRouteOverride,
        });
      } else {
        await createHost({
          displayName: draft.displayName,
          host: draft.host,
          port,
          groupId: draft.groupId || null,
          credentialOverride: draft.credentialOverride,
          jumpRouteOverride: draft.jumpRouteOverride,
        });
      }
      await onChanged();
      setDraft(null);
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function removeHost(hostId: string) {
    if (confirmDelete !== hostId) {
      setConfirmDelete(hostId);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await deleteHost(hostId);
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
        <button
          className="connect-button compact-button"
          onClick={() => {
            setError(null);
            setDraft({
              ...EMPTY_HOST_DRAFT,
              credentialOverride: { kind: "inherit" },
              jumpRouteOverride: { kind: "inherit" },
            });
          }}
          type="button"
        >
          New host
        </button>
      }
      description="Endpoints reference Credentials and ordered Jump Routes without copying secrets."
      eyebrow="Inventory"
      title="Hosts"
    >
      {error && <ManagerError message={error} />}
      {loading ? (
        <ManagerEmpty>Loading Hosts…</ManagerEmpty>
      ) : hosts.length === 0 ? (
        <ManagerEmpty>No saved Hosts yet.</ManagerEmpty>
      ) : (
        <div className="resource-list">
          {hosts.map((host) => {
            const credential = host.effectiveCredentialId
              ? credentialLabels.get(host.effectiveCredentialId)
              : null;
            const route = host.effectiveJumpRouteId
              ? routeLabels.get(host.effectiveJumpRouteId)
              : null;
            const group = host.groupId ? groupLabels.get(host.groupId) : null;
            return (
              <article className="resource-card" key={host.id}>
                <div className="resource-icon host-resource-icon">
                  {host.displayName.slice(0, 2)}
                </div>
                <div className="resource-main">
                  <strong>{host.displayName}</strong>
                  <span>
                    {host.host}:{host.port}
                  </span>
                  <div className="resource-tags">
                    {group && <span>{group.label}</span>}
                    <span>
                      {credential
                        ? `${credential.username} · ${kindLabel(credential.kind)} · ${overrideStateLabel(host.credentialOverride)}`
                        : `No Credential · ${overrideStateLabel(host.credentialOverride)}`}
                    </span>
                    <span>
                      {route
                        ? `${route.label} · ${overrideStateLabel(host.jumpRouteOverride)}`
                        : `Direct · ${overrideStateLabel(host.jumpRouteOverride)}`}
                    </span>
                  </div>
                </div>
                <div className="resource-actions">
                  <button onClick={() => onOpenHost(host)} type="button">
                    Open
                  </button>
                  <button onClick={() => editHost(host)} type="button">
                    Edit
                  </button>
                  <button
                    className={confirmDelete === host.id ? "danger-action" : ""}
                    disabled={busy}
                    onClick={() => void removeHost(host.id)}
                    type="button"
                  >
                    {confirmDelete === host.id ? "Confirm delete" : "Delete"}
                  </button>
                </div>
              </article>
            );
          })}
        </div>
      )}

      {draft && (
        <EditorDialog
          onClose={() => {
            setDraft(null);
            setError(null);
          }}
          title={draft.hostId ? "Edit Host" : "New Host"}
        >
          <form className="editor-form" onSubmit={saveHost}>
            <label>
              Display name
              <input
                autoFocus
                onChange={(event) =>
                  setDraft((current) =>
                    current
                      ? { ...current, displayName: event.target.value }
                      : current,
                  )
                }
                value={draft.displayName}
              />
            </label>
            <div className="editor-field-grid">
              <label>
                Host
                <input
                  autoCapitalize="none"
                  onChange={(event) =>
                    setDraft((current) =>
                      current
                        ? { ...current, host: event.target.value }
                        : current,
                    )
                  }
                  placeholder="server.example.com"
                  spellCheck={false}
                  value={draft.host}
                />
              </label>
              <label className="editor-port-field">
                Port
                <input
                  max="65535"
                  min="1"
                  onChange={(event) =>
                    setDraft((current) =>
                      current
                        ? { ...current, port: event.target.value }
                        : current,
                    )
                  }
                  type="number"
                  value={draft.port}
                />
              </label>
            </div>
            <label>
              Group
              <select
                onChange={(event) =>
                  setDraft((current) =>
                    current
                      ? { ...current, groupId: event.target.value }
                      : current,
                  )
                }
                value={draft.groupId}
              >
                <option value="">No Group</option>
                {groups.map((group) => (
                  <option key={group.id} value={group.id}>
                    {group.label}
                  </option>
                ))}
              </select>
            </label>
            <ReferenceOverrideEditor
              inheritLabel={
                draft.groupId
                  ? "Inherit Group Credential"
                  : "Inherit application default"
              }
              label="Credential"
              onChange={(credentialOverride) =>
                setDraft((current) =>
                  current ? { ...current, credentialOverride } : current,
                )
              }
              options={credentials.map((credential) => ({
                id: credential.id,
                label: `${credential.label} · ${credential.username}`,
              }))}
              setLabel="Set Credential"
              value={draft.credentialOverride}
            />
            <ReferenceOverrideEditor
              inheritLabel={
                draft.groupId
                  ? "Inherit Group Jump Route"
                  : "Inherit direct connection"
              }
              label="Jump Route"
              onChange={(jumpRouteOverride) =>
                setDraft((current) =>
                  current ? { ...current, jumpRouteOverride } : current,
                )
              }
              options={routes.map((route) => ({
                id: route.id,
                label: route.label,
              }))}
              setLabel="Set Jump Route"
              value={draft.jumpRouteOverride}
            />
            {error && <ManagerError message={error} />}
            <EditorActions busy={busy} submitLabel="Save Host" />
          </form>
        </EditorDialog>
      )}
    </ManagerShell>
  );
}

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
    };

function CredentialManager({
  credentials,
  loading,
  onChanged,
}: CredentialManagerProps) {
  const [draft, setDraft] = useState<CredentialDraft | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

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
                {credential.kind === "privateKey" ? "PK" : "PW"}
              </div>
              <div className="resource-main">
                <strong>{credential.label}</strong>
                <span>{credential.username}</span>
                <div className="resource-tags">
                  <span>{kindLabel(credential.kind)}</span>
                  <span>Secret hidden</span>
                </div>
              </div>
              <div className="resource-actions">
                {credential.kind === "password" && (
                  <button
                    onClick={() => editPassword(credential)}
                    type="button"
                  >
                    Replace password
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
          onClose={closeEditor}
          title={
            draft.kind === "privateKey"
              ? "Import Private Key"
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
            ) : (
              <div className="security-note">
                <strong>Rust-owned file import</strong>
                <p>
                  The native picker opens after you continue. File path and Key
                  content never enter the WebView. Encrypted Keys are rejected
                  until a native Passphrase prompt is available.
                </p>
              </div>
            )}
            {error && <ManagerError message={error} />}
            <EditorActions
              busy={busy}
              submitLabel={
                draft.kind === "privateKey"
                  ? "Choose private key"
                  : "Save Credential"
              }
            />
          </form>
        </EditorDialog>
      )}
    </ManagerShell>
  );
}

interface RouteManagerProps extends ManagerProps {
  hosts: HostSummary[];
  routes: JumpRouteSummary[];
}

interface RouteDraft {
  routeId: string | null;
  label: string;
  hostIds: string[];
  availableHostId: string;
}

const EMPTY_ROUTE_DRAFT: RouteDraft = {
  routeId: null,
  label: "",
  hostIds: [],
  availableHostId: "",
};

function RouteManager({
  hosts,
  routes,
  loading,
  onChanged,
}: RouteManagerProps) {
  const [draft, setDraft] = useState<RouteDraft | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const hostLabels = useMemo(
    () => new Map(hosts.map((host) => [host.id, host])),
    [hosts],
  );

  function editRoute(route: JumpRouteSummary) {
    setError(null);
    setDraft({
      routeId: route.id,
      label: route.label,
      hostIds: [...route.hostIds],
      availableHostId: "",
    });
  }

  function addRouteHost() {
    setDraft((current) => {
      if (
        !current ||
        !current.availableHostId ||
        current.hostIds.includes(current.availableHostId)
      ) {
        return current;
      }
      return {
        ...current,
        hostIds: [...current.hostIds, current.availableHostId],
        availableHostId: "",
      };
    });
  }

  function moveRouteHost(index: number, direction: -1 | 1) {
    setDraft((current) => {
      if (!current) return current;
      const target = index + direction;
      if (target < 0 || target >= current.hostIds.length) return current;
      const hostIds = [...current.hostIds];
      const currentId = hostIds[index];
      const targetId = hostIds[target];
      if (!currentId || !targetId) return current;
      hostIds[index] = targetId;
      hostIds[target] = currentId;
      return { ...current, hostIds };
    });
  }

  async function saveRoute(event: FormEvent) {
    event.preventDefault();
    if (!draft) return;
    if (!draft.label.trim() || draft.hostIds.length === 0) {
      setError("Route label and at least one ordered Host are required.");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      if (draft.routeId) {
        await updateJumpRoute({
          jumpRouteId: draft.routeId,
          label: draft.label,
          hostIds: draft.hostIds,
        });
      } else {
        await createJumpRoute({
          label: draft.label,
          hostIds: draft.hostIds,
        });
      }
      await onChanged();
      setDraft(null);
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function removeRoute(routeId: string) {
    if (confirmDelete !== routeId) {
      setConfirmDelete(routeId);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await deleteJumpRoute(routeId);
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
        <button
          className="connect-button compact-button"
          disabled={hosts.length === 0}
          onClick={() => {
            setError(null);
            setDraft({ ...EMPTY_ROUTE_DRAFT });
          }}
          type="button"
        >
          New route
        </button>
      }
      description="Routes store an ordered list of Host IDs and are cycle-checked transactionally."
      eyebrow="Topology"
      title="Jump Routes"
    >
      {error && <ManagerError message={error} />}
      {loading ? (
        <ManagerEmpty>Loading Jump Routes…</ManagerEmpty>
      ) : routes.length === 0 ? (
        <ManagerEmpty>
          {hosts.length === 0
            ? "Create a Host before building a Jump Route."
            : "No Jump Routes yet."}
        </ManagerEmpty>
      ) : (
        <div className="resource-list">
          {routes.map((route) => (
            <article className="resource-card route-card" key={route.id}>
              <div className="resource-icon route-resource-icon">
                {route.hostIds.length}
              </div>
              <div className="resource-main">
                <strong>{route.label}</strong>
                <span>{route.hostIds.length} ordered hop(s)</span>
                <ol className="route-preview">
                  {route.hostIds.map((hostId) => (
                    <li key={hostId}>
                      {hostLabels.get(hostId)?.displayName ?? hostId}
                    </li>
                  ))}
                </ol>
              </div>
              <div className="resource-actions">
                <button onClick={() => editRoute(route)} type="button">
                  Edit
                </button>
                <button
                  className={confirmDelete === route.id ? "danger-action" : ""}
                  disabled={busy}
                  onClick={() => void removeRoute(route.id)}
                  type="button"
                >
                  {confirmDelete === route.id ? "Confirm delete" : "Delete"}
                </button>
              </div>
            </article>
          ))}
        </div>
      )}

      {draft && (
        <EditorDialog
          onClose={() => {
            setDraft(null);
            setError(null);
          }}
          title={draft.routeId ? "Edit Jump Route" : "New Jump Route"}
        >
          <form className="editor-form" onSubmit={saveRoute}>
            <label>
              Route label
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
            <div className="route-add-row">
              <label>
                Add Host
                <select
                  onChange={(event) =>
                    setDraft((current) =>
                      current
                        ? { ...current, availableHostId: event.target.value }
                        : current,
                    )
                  }
                  value={draft.availableHostId}
                >
                  <option value="">Select a Host</option>
                  {hosts
                    .filter((host) => !draft.hostIds.includes(host.id))
                    .map((host) => (
                      <option key={host.id} value={host.id}>
                        {host.displayName} · {host.host}:{host.port}
                      </option>
                    ))}
                </select>
              </label>
              <button
                className="secondary-button compact-button"
                disabled={!draft.availableHostId}
                onClick={addRouteHost}
                type="button"
              >
                Add
              </button>
            </div>
            <div className="route-builder" aria-label="Ordered route Hosts">
              {draft.hostIds.length === 0 ? (
                <p>No Hosts in this Route.</p>
              ) : (
                draft.hostIds.map((hostId, index) => (
                  <div className="route-builder-row" key={hostId}>
                    <span className="route-position">{index + 1}</span>
                    <div>
                      <strong>
                        {hostLabels.get(hostId)?.displayName ?? hostId}
                      </strong>
                      <small>
                        {hostLabels.get(hostId)
                          ? `${hostLabels.get(hostId)?.host}:${hostLabels.get(hostId)?.port}`
                          : "Missing Host"}
                      </small>
                    </div>
                    <div className="route-row-actions">
                      <button
                        aria-label={`Move ${hostLabels.get(hostId)?.displayName ?? hostId} up`}
                        disabled={index === 0}
                        onClick={() => moveRouteHost(index, -1)}
                        type="button"
                      >
                        ↑
                      </button>
                      <button
                        aria-label={`Move ${hostLabels.get(hostId)?.displayName ?? hostId} down`}
                        disabled={index === draft.hostIds.length - 1}
                        onClick={() => moveRouteHost(index, 1)}
                        type="button"
                      >
                        ↓
                      </button>
                      <button
                        aria-label={`Remove ${hostLabels.get(hostId)?.displayName ?? hostId}`}
                        className="danger-action"
                        onClick={() =>
                          setDraft((current) =>
                            current
                              ? {
                                  ...current,
                                  hostIds: current.hostIds.filter(
                                    (id) => id !== hostId,
                                  ),
                                }
                              : current,
                          )
                        }
                        type="button"
                      >
                        ×
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>
            {error && <ManagerError message={error} />}
            <EditorActions busy={busy} submitLabel="Save Jump Route" />
          </form>
        </EditorDialog>
      )}
    </ManagerShell>
  );
}

interface ReferenceOverrideEditorProps {
  label: string;
  inheritLabel: string;
  setLabel: string;
  value: ReferenceOverride;
  options: Array<{ id: string; label: string }>;
  onChange(value: ReferenceOverride): void;
}

function ReferenceOverrideEditor({
  label,
  inheritLabel,
  setLabel,
  value,
  options,
  onChange,
}: ReferenceOverrideEditorProps) {
  return (
    <fieldset className="override-editor">
      <legend>{label}</legend>
      <label>
        Behavior
        <select
          aria-label={`${label} behavior`}
          onChange={(event) => {
            switch (event.target.value) {
              case "set":
                onChange({
                  kind: "set",
                  value:
                    value.kind === "set" ? value.value : (options[0]?.id ?? ""),
                });
                break;
              case "clear":
                onChange({ kind: "clear" });
                break;
              default:
                onChange({ kind: "inherit" });
            }
          }}
          value={value.kind}
        >
          <option value="inherit">{inheritLabel}</option>
          <option value="set">{setLabel}</option>
          <option value="clear">Clear inherited value</option>
        </select>
      </label>
      {value.kind === "set" && (
        <label>
          {label} reference
          <select
            aria-label={`${label} reference`}
            onChange={(event) =>
              onChange({ kind: "set", value: event.target.value })
            }
            value={value.value}
          >
            {options.length === 0 && (
              <option value="">No {label} available</option>
            )}
            {options.map((option) => (
              <option key={option.id} value={option.id}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
      )}
    </fieldset>
  );
}

function flattenGroupTree(
  groups: GroupSummary[],
): Array<{ group: GroupSummary; depth: number }> {
  const byParent = new Map<string | null, GroupSummary[]>();
  for (const group of groups) {
    const siblings = byParent.get(group.parentGroupId) ?? [];
    siblings.push(group);
    byParent.set(group.parentGroupId, siblings);
  }
  for (const siblings of byParent.values()) {
    siblings.sort((left, right) => left.label.localeCompare(right.label));
  }

  const output: Array<{ group: GroupSummary; depth: number }> = [];
  const visited = new Set<string>();
  const visit = (group: GroupSummary, depth: number) => {
    if (visited.has(group.id)) return;
    visited.add(group.id);
    output.push({ group, depth });
    for (const child of byParent.get(group.id) ?? []) {
      visit(child, depth + 1);
    }
  };
  for (const root of byParent.get(null) ?? []) visit(root, 0);
  for (const group of groups) visit(group, 0);
  return output;
}

function descendantGroupIds(
  groups: GroupSummary[],
  groupId: string,
): Set<string> {
  const result = new Set([groupId]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const group of groups) {
      if (
        group.parentGroupId &&
        result.has(group.parentGroupId) &&
        !result.has(group.id)
      ) {
        result.add(group.id);
        changed = true;
      }
    }
  }
  return result;
}

function cloneReferenceOverride(value: ReferenceOverride): ReferenceOverride {
  return value.kind === "set"
    ? { kind: "set", value: value.value }
    : { ...value };
}

function overrideHasSelection(value: ReferenceOverride) {
  return value.kind !== "set" || Boolean(value.value);
}

function overrideStateLabel(value: ReferenceOverride) {
  switch (value.kind) {
    case "inherit":
      return "Inherited";
    case "set":
      return "Set here";
    case "clear":
      return "Cleared";
  }
}

interface ManagerShellProps {
  eyebrow: string;
  title: string;
  description: string;
  action: React.ReactNode;
  children: React.ReactNode;
}

function ManagerShell({
  eyebrow,
  title,
  description,
  action,
  children,
}: ManagerShellProps) {
  return (
    <section className="manager-shell">
      <header className="manager-header">
        <div>
          <p className="eyebrow">{eyebrow}</p>
          <h2>{title}</h2>
          <p>{description}</p>
        </div>
        {action}
      </header>
      {children}
    </section>
  );
}

function ManagerError({ message }: { message: string }) {
  return (
    <div className="manager-error" role="alert">
      {message}
    </div>
  );
}

function ManagerEmpty({ children }: { children: React.ReactNode }) {
  return <div className="manager-empty">{children}</div>;
}

interface EditorDialogProps {
  title: string;
  onClose(): void;
  children: React.ReactNode;
}

function EditorDialog({ title, onClose, children }: EditorDialogProps) {
  return (
    <div className="dialog-backdrop resource-dialog-backdrop">
      <section
        aria-labelledby="resource-dialog-title"
        aria-modal="true"
        className="resource-dialog"
        role="dialog"
      >
        <header>
          <div>
            <p className="eyebrow">Vault configuration</p>
            <h2 id="resource-dialog-title">{title}</h2>
          </div>
          <button aria-label="Close editor" onClick={onClose} type="button">
            ×
          </button>
        </header>
        {children}
      </section>
    </div>
  );
}

function EditorActions({
  busy,
  submitLabel,
}: {
  busy: boolean;
  submitLabel: string;
}) {
  return (
    <div className="editor-actions">
      <button className="connect-button" disabled={busy} type="submit">
        {busy ? "Saving…" : submitLabel}
      </button>
    </div>
  );
}

function kindLabel(kind: CredentialSummary["kind"]) {
  return kind === "privateKey" ? "Private Key" : "Password";
}
