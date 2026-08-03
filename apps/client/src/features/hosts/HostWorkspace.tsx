import { type FormEvent, useMemo, useState } from "react";
import type { CredentialSummary } from "../../lib/credential-bridge";
import {
  createHost,
  deleteHost,
  updateHost,
  type GroupSummary,
  type HostSummary,
  type JumpRouteSummary,
  type ReferenceOverride,
} from "../../lib/host-bridge";
import { ReferenceOverrideEditor } from "../configuration/ReferenceOverrideEditor";
import {
  cloneReferenceOverride,
  overrideHasSelection,
  overrideStateLabel,
} from "../configuration/reference-override";
import {
  EditorActions,
  EditorDialog,
  ManagerEmpty,
  ManagerError,
  ManagerShell,
  type ManagerProps,
} from "../configuration/ManagerPrimitives";
import { credentialKindLabel } from "../credentials/credential-labels";
import { NavigationIcon } from "../../shared/icons/ProductIcons";

interface HostManagerProps extends ManagerProps {
  groups: GroupSummary[];
  hosts: HostSummary[];
  credentials: CredentialSummary[];
  routes: JumpRouteSummary[];
  nativeRuntime: boolean;
  onConnectHost(host: HostSummary): void;
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

export function HostWorkspace({
  hosts,
  groups,
  credentials,
  routes,
  loading,
  nativeRuntime,
  onChanged,
  onConnectHost,
  onOpenHost,
}: HostManagerProps) {
  const [draft, setDraft] = useState<HostDraft | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [groupFilter, setGroupFilter] = useState<string>("all");
  const [detailHostId, setDetailHostId] = useState<string | null>(null);
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
  const visibleHosts = useMemo(() => {
    const normalizedQuery = searchQuery.trim().toLocaleLowerCase();
    return hosts.filter((host) => {
      if (groupFilter !== "all" && host.groupId !== groupFilter) {
        return false;
      }
      if (!normalizedQuery) return true;
      const groupLabel = host.groupId
        ? groupLabels.get(host.groupId)?.label
        : "";
      return [host.displayName, host.host, String(host.port), groupLabel]
        .filter(Boolean)
        .some((value) => value?.toLocaleLowerCase().includes(normalizedQuery));
    });
  }, [groupFilter, groupLabels, hosts, searchQuery]);
  const detailHost = detailHostId
    ? (hosts.find((host) => host.id === detailHostId) ?? null)
    : null;

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
      if (detailHostId === hostId) {
        setDetailHostId(null);
      }
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
          aria-label="New host"
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
          Add host
        </button>
      }
      description="Open a saved server, or add a new connection without duplicating credentials."
      eyebrow="Workspace"
      title="Hosts"
    >
      {error && <ManagerError message={error} />}
      {!loading && hosts.length > 0 && (
        <div className="host-manager-toolbar">
          <label className="host-search">
            <span aria-hidden="true">⌕</span>
            <input
              aria-label="Search Hosts"
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="Search name, address, or group"
              type="search"
              value={searchQuery}
            />
          </label>
          <div aria-label="Filter Hosts by Group" className="host-filter-list">
            <button
              aria-pressed={groupFilter === "all"}
              className={groupFilter === "all" ? "active" : ""}
              onClick={() => setGroupFilter("all")}
              type="button"
            >
              All hosts
              <span>{hosts.length}</span>
            </button>
            {groups.map((group) => {
              const count = hosts.filter(
                (host) => host.groupId === group.id,
              ).length;
              return (
                <button
                  aria-pressed={groupFilter === group.id}
                  className={groupFilter === group.id ? "active" : ""}
                  key={group.id}
                  onClick={() => setGroupFilter(group.id)}
                  type="button"
                >
                  {group.label}
                  <span>{count}</span>
                </button>
              );
            })}
          </div>
        </div>
      )}

      {detailHost && (
        <HostDetail
          credential={
            detailHost.effectiveCredentialId
              ? (credentialLabels.get(detailHost.effectiveCredentialId) ?? null)
              : null
          }
          group={
            detailHost.groupId
              ? (groupLabels.get(detailHost.groupId) ?? null)
              : null
          }
          host={detailHost}
          connectLabel={nativeRuntime ? "Connect" : "Open session"}
          onClose={() => setDetailHostId(null)}
          onConnect={() =>
            nativeRuntime ? onConnectHost(detailHost) : onOpenHost(detailHost)
          }
          onEdit={() => editHost(detailHost)}
          route={
            detailHost.effectiveJumpRouteId
              ? (routeLabels.get(detailHost.effectiveJumpRouteId) ?? null)
              : null
          }
        />
      )}

      {loading ? (
        <ManagerEmpty>Loading Hosts…</ManagerEmpty>
      ) : hosts.length === 0 ? (
        <ManagerEmpty>
          No saved Hosts yet. Add a server to begin your first connection.
        </ManagerEmpty>
      ) : visibleHosts.length === 0 ? (
        <ManagerEmpty>
          No Hosts match the current search and filter.
        </ManagerEmpty>
      ) : (
        <div className="resource-list host-resource-grid">
          {visibleHosts.map((host) => {
            const credential = host.effectiveCredentialId
              ? credentialLabels.get(host.effectiveCredentialId)
              : null;
            const route = host.effectiveJumpRouteId
              ? routeLabels.get(host.effectiveJumpRouteId)
              : null;
            const group = host.groupId ? groupLabels.get(host.groupId) : null;
            return (
              <article
                className="resource-card host-resource-card"
                key={host.id}
              >
                <div className="resource-icon host-resource-icon">
                  <NavigationIcon name="hosts" />
                </div>
                <div className="resource-main">
                  <button
                    className="host-detail-trigger"
                    onClick={() => setDetailHostId(host.id)}
                    type="button"
                  >
                    <strong>{host.displayName}</strong>
                    <code>
                      {host.host}:{host.port}
                    </code>
                  </button>
                  <div className="resource-tags">
                    <span>{group?.label ?? "Ungrouped"}</span>
                    <span>
                      {credential
                        ? `${credential.username} · ${credentialKindLabel(credential.kind)}`
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
                  <button
                    onClick={() => setDetailHostId(host.id)}
                    type="button"
                  >
                    Details
                  </button>
                  <button
                    aria-label="Open"
                    className="host-connect-action"
                    onClick={() => onOpenHost(host)}
                    type="button"
                  >
                    Open session
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
          <form className="editor-form host-editor-form" onSubmit={saveHost}>
            <section className="host-editor-section">
              <header>
                <span>1</span>
                <div>
                  <strong>Connection target</strong>
                  <p>Name the server and enter its network endpoint.</p>
                </div>
              </header>
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
            </section>

            <section className="host-editor-section">
              <header>
                <span>2</span>
                <div>
                  <strong>Authentication</strong>
                  <p>
                    Reference a saved Credential without copying its secret.
                  </p>
                </div>
              </header>
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
            </section>

            <section className="host-editor-section">
              <header>
                <span>3</span>
                <div>
                  <strong>Advanced connection</strong>
                  <p>Choose direct access or an ordered Jump Route.</p>
                </div>
              </header>
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
            </section>
            {error && <ManagerError message={error} />}
            <EditorActions busy={busy} submitLabel="Save Host" />
          </form>
        </EditorDialog>
      )}
    </ManagerShell>
  );
}

interface HostDetailProps {
  host: HostSummary;
  group: GroupSummary | null;
  credential: CredentialSummary | null;
  route: JumpRouteSummary | null;
  connectLabel: string;
  onClose(): void;
  onConnect(): void;
  onEdit(): void;
}

function HostDetail({
  host,
  group,
  credential,
  route,
  connectLabel,
  onClose,
  onConnect,
  onEdit,
}: HostDetailProps) {
  return (
    <section aria-labelledby="host-detail-title" className="host-detail-panel">
      <div className="host-detail-identity">
        <span className="host-detail-icon">
          <NavigationIcon name="hosts" />
        </span>
        <div>
          <div className="host-detail-chips">
            <span>{group?.label ?? "Ungrouped"}</span>
            <span>{route?.label ?? "Direct connection"}</span>
          </div>
          <h3 id="host-detail-title">{host.displayName}</h3>
          <code>
            {credential ? `${credential.username}@` : ""}
            {host.host}:{host.port}
          </code>
        </div>
      </div>
      <div className="host-detail-plan">
        <div>
          <span>Credential</span>
          <strong>
            {credential
              ? `${credential.label} · ${credentialKindLabel(credential.kind)}`
              : "No saved Credential"}
          </strong>
        </div>
        <div>
          <span>Route</span>
          <strong>{route?.label ?? "Connect directly"}</strong>
        </div>
      </div>
      <div className="host-detail-actions">
        <button className="connect-button" onClick={onConnect} type="button">
          {connectLabel}
        </button>
        <button className="secondary-button" onClick={onEdit} type="button">
          Edit
        </button>
        <button className="secondary-button" onClick={onClose} type="button">
          Close details
        </button>
      </div>
    </section>
  );
}
