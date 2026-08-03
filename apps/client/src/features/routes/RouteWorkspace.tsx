import { type FormEvent, useMemo, useState } from "react";
import {
  createJumpRoute,
  deleteJumpRoute,
  updateJumpRoute,
  type HostSummary,
  type JumpRouteSummary,
} from "../../lib/host-bridge";
import {
  EditorActions,
  EditorDialog,
  ManagerEmpty,
  ManagerError,
  ManagerShell,
  type ManagerProps,
} from "../configuration/ManagerPrimitives";

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

export function RouteWorkspace({
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
