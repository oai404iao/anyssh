import { type FormEvent, useMemo, useState } from "react";
import type { CredentialSummary } from "../../lib/credential-bridge";
import {
  createGroup,
  deleteGroup,
  updateGroup,
  type GroupSummary,
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

export function GroupWorkspace({
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
