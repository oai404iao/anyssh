import { invoke, isTauri } from "@tauri-apps/api/core";

export type ReferenceOverride =
  { kind: "inherit" } | { kind: "set"; value: string } | { kind: "clear" };

export interface GroupSummary {
  id: string;
  label: string;
  parentGroupId: string | null;
  credentialOverride: ReferenceOverride;
  jumpRouteOverride: ReferenceOverride;
  effectiveCredentialId: string | null;
  effectiveJumpRouteId: string | null;
}

export interface GroupInput {
  label: string;
  parentGroupId?: string | null;
  credentialOverride: ReferenceOverride;
  jumpRouteOverride: ReferenceOverride;
}

export interface GroupUpdate extends GroupInput {
  groupId: string;
}

export interface HostSummary {
  id: string;
  displayName: string;
  host: string;
  port: number;
  groupId: string | null;
  credentialOverride: ReferenceOverride;
  jumpRouteOverride: ReferenceOverride;
  effectiveCredentialId: string | null;
  effectiveJumpRouteId: string | null;
}

export interface HostInput {
  displayName: string;
  host: string;
  port: number;
  groupId?: string | null;
  credentialOverride: ReferenceOverride;
  jumpRouteOverride: ReferenceOverride;
}

export interface HostUpdate extends HostInput {
  hostId: string;
}

export interface JumpRouteSummary {
  id: string;
  label: string;
  hostIds: string[];
}

export interface JumpRouteInput {
  label: string;
  hostIds: string[];
}

export interface JumpRouteUpdate extends JumpRouteInput {
  jumpRouteId: string;
}

const MAX_GROUP_DEPTH = 32;

const BROWSER_GROUP_FIXTURES: GroupSummary[] = [
  {
    id: "browser-group-production",
    label: "Production",
    parentGroupId: null,
    credentialOverride: {
      kind: "set",
      value: "browser-credential-database",
    },
    jumpRouteOverride: { kind: "set", value: "browser-route-edge" },
    effectiveCredentialId: "browser-credential-database",
    effectiveJumpRouteId: "browser-route-edge",
  },
];

const BROWSER_HOST_FIXTURES: HostSummary[] = [
  {
    id: "browser-host-local",
    displayName: "Local lab",
    host: "127.0.0.1",
    port: 2222,
    groupId: null,
    credentialOverride: {
      kind: "set",
      value: "browser-credential-local",
    },
    jumpRouteOverride: { kind: "inherit" },
    effectiveCredentialId: "browser-credential-local",
    effectiveJumpRouteId: null,
  },
  {
    id: "browser-host-edge",
    displayName: "Edge gateway",
    host: "10.0.0.8",
    port: 22,
    groupId: null,
    credentialOverride: {
      kind: "set",
      value: "browser-credential-edge",
    },
    jumpRouteOverride: { kind: "inherit" },
    effectiveCredentialId: "browser-credential-edge",
    effectiveJumpRouteId: null,
  },
  {
    id: "browser-host-database",
    displayName: "Database",
    host: "db.internal",
    port: 22,
    groupId: "browser-group-production",
    credentialOverride: { kind: "inherit" },
    jumpRouteOverride: { kind: "inherit" },
    effectiveCredentialId: "browser-credential-database",
    effectiveJumpRouteId: "browser-route-edge",
  },
];

const BROWSER_ROUTE_FIXTURES: JumpRouteSummary[] = [
  {
    id: "browser-route-edge",
    label: "Through edge gateway",
    hostIds: ["browser-host-edge"],
  },
];

let browserGroups = cloneGroups(BROWSER_GROUP_FIXTURES);
let browserHosts = cloneHosts(BROWSER_HOST_FIXTURES);
let browserRoutes = cloneRoutes(BROWSER_ROUTE_FIXTURES);
let nextBrowserGroupId = browserGroups.length + 1;
let nextBrowserHostId = browserHosts.length + 1;
let nextBrowserRouteId = browserRoutes.length + 1;

export async function listGroups(): Promise<GroupSummary[]> {
  if (!isTauri()) return cloneGroups(browserGroups);
  return invoke<GroupSummary[]>("group_list");
}

export async function createGroup(input: GroupInput): Promise<GroupSummary> {
  if (!isTauri()) {
    const previous = snapshotBrowserRepository();
    const summary: GroupSummary = {
      id: `browser-group-${nextBrowserGroupId++}`,
      label: input.label,
      parentGroupId: input.parentGroupId ?? null,
      credentialOverride: cloneOverride(input.credentialOverride),
      jumpRouteOverride: cloneOverride(input.jumpRouteOverride),
      effectiveCredentialId: null,
      effectiveJumpRouteId: null,
    };
    browserGroups.push(summary);
    try {
      validateAndResolveBrowserRepository();
    } catch (error) {
      restoreBrowserRepository(previous);
      throw error;
    }
    return cloneGroup(
      browserGroups.find((group) => group.id === summary.id) ?? summary,
    );
  }
  return invoke<GroupSummary>("group_create", { request: input });
}

export async function updateGroup(input: GroupUpdate): Promise<GroupSummary> {
  if (!isTauri()) {
    const index = browserGroups.findIndex(
      (group) => group.id === input.groupId,
    );
    if (index < 0) throw new Error("Group was not found");
    const previous = snapshotBrowserRepository();
    browserGroups[index] = {
      id: input.groupId,
      label: input.label,
      parentGroupId: input.parentGroupId ?? null,
      credentialOverride: cloneOverride(input.credentialOverride),
      jumpRouteOverride: cloneOverride(input.jumpRouteOverride),
      effectiveCredentialId: null,
      effectiveJumpRouteId: null,
    };
    try {
      validateAndResolveBrowserRepository();
    } catch (error) {
      restoreBrowserRepository(previous);
      throw error;
    }
    return cloneGroup(browserGroups[index]!);
  }
  return invoke<GroupSummary>("group_update", { request: input });
}

export async function deleteGroup(groupId: string): Promise<boolean> {
  if (!isTauri()) {
    if (
      browserGroups.some((group) => group.parentGroupId === groupId) ||
      browserHosts.some((host) => host.groupId === groupId)
    ) {
      throw new Error("Group is in use by a child Group or Host");
    }
    const previousLength = browserGroups.length;
    browserGroups = browserGroups.filter((group) => group.id !== groupId);
    return browserGroups.length !== previousLength;
  }
  return invoke<boolean>("group_delete", { groupId });
}

export async function listHosts(): Promise<HostSummary[]> {
  if (!isTauri()) return cloneHosts(browserHosts);
  return invoke<HostSummary[]>("host_list");
}

export async function createHost(input: HostInput): Promise<HostSummary> {
  if (!isTauri()) {
    const previous = snapshotBrowserRepository();
    const summary: HostSummary = {
      id: `browser-host-${nextBrowserHostId++}`,
      displayName: input.displayName,
      host: input.host,
      port: input.port,
      groupId: input.groupId ?? null,
      credentialOverride: cloneOverride(input.credentialOverride),
      jumpRouteOverride: cloneOverride(input.jumpRouteOverride),
      effectiveCredentialId: null,
      effectiveJumpRouteId: null,
    };
    browserHosts.push(summary);
    try {
      validateAndResolveBrowserRepository();
    } catch (error) {
      restoreBrowserRepository(previous);
      throw error;
    }
    return cloneHost(
      browserHosts.find((host) => host.id === summary.id) ?? summary,
    );
  }
  return invoke<HostSummary>("host_create", { request: input });
}

export async function updateHost(input: HostUpdate): Promise<HostSummary> {
  if (!isTauri()) {
    const index = browserHosts.findIndex((host) => host.id === input.hostId);
    if (index < 0) throw new Error("Host was not found");
    const previous = snapshotBrowserRepository();
    browserHosts[index] = {
      id: input.hostId,
      displayName: input.displayName,
      host: input.host,
      port: input.port,
      groupId: input.groupId ?? null,
      credentialOverride: cloneOverride(input.credentialOverride),
      jumpRouteOverride: cloneOverride(input.jumpRouteOverride),
      effectiveCredentialId: null,
      effectiveJumpRouteId: null,
    };
    try {
      validateAndResolveBrowserRepository();
    } catch (error) {
      restoreBrowserRepository(previous);
      throw error;
    }
    return cloneHost(browserHosts[index]!);
  }
  return invoke<HostSummary>("host_update", { request: input });
}

export async function deleteHost(hostId: string): Promise<boolean> {
  if (!isTauri()) {
    if (browserRoutes.some((route) => route.hostIds.includes(hostId))) {
      throw new Error("Host is in use by a Jump Route");
    }
    const previousLength = browserHosts.length;
    browserHosts = browserHosts.filter((host) => host.id !== hostId);
    return browserHosts.length !== previousLength;
  }
  return invoke<boolean>("host_delete", { hostId });
}

export async function listJumpRoutes(): Promise<JumpRouteSummary[]> {
  if (!isTauri()) return cloneRoutes(browserRoutes);
  return invoke<JumpRouteSummary[]>("jump_route_list");
}

export async function createJumpRoute(
  input: JumpRouteInput,
): Promise<JumpRouteSummary> {
  if (!isTauri()) {
    validateBrowserRoute(input);
    const previous = snapshotBrowserRepository();
    const summary = {
      id: `browser-route-${nextBrowserRouteId++}`,
      label: input.label,
      hostIds: [...input.hostIds],
    };
    browserRoutes.push(summary);
    try {
      validateAndResolveBrowserRepository();
    } catch (error) {
      restoreBrowserRepository(previous);
      throw error;
    }
    return { ...summary, hostIds: [...summary.hostIds] };
  }
  return invoke<JumpRouteSummary>("jump_route_create", { request: input });
}

export async function updateJumpRoute(
  input: JumpRouteUpdate,
): Promise<JumpRouteSummary> {
  if (!isTauri()) {
    validateBrowserRoute(input);
    const index = browserRoutes.findIndex(
      (route) => route.id === input.jumpRouteId,
    );
    if (index < 0) throw new Error("Jump Route was not found");
    const previous = snapshotBrowserRepository();
    const summary = {
      id: input.jumpRouteId,
      label: input.label,
      hostIds: [...input.hostIds],
    };
    browserRoutes[index] = summary;
    try {
      validateAndResolveBrowserRepository();
    } catch (error) {
      restoreBrowserRepository(previous);
      throw error;
    }
    return { ...summary, hostIds: [...summary.hostIds] };
  }
  return invoke<JumpRouteSummary>("jump_route_update", { request: input });
}

export async function deleteJumpRoute(jumpRouteId: string): Promise<boolean> {
  if (!isTauri()) {
    if (
      browserHosts.some(
        (host) =>
          host.jumpRouteOverride.kind === "set" &&
          host.jumpRouteOverride.value === jumpRouteId,
      ) ||
      browserGroups.some(
        (group) =>
          group.jumpRouteOverride.kind === "set" &&
          group.jumpRouteOverride.value === jumpRouteId,
      )
    ) {
      throw new Error("Jump Route is in use by a Host or Group");
    }
    const previousLength = browserRoutes.length;
    browserRoutes = browserRoutes.filter((route) => route.id !== jumpRouteId);
    return browserRoutes.length !== previousLength;
  }
  return invoke<boolean>("jump_route_delete", { jumpRouteId });
}

export function browserCredentialIsReferenced(credentialId: string): boolean {
  return (
    browserHosts.some(
      (host) =>
        host.credentialOverride.kind === "set" &&
        host.credentialOverride.value === credentialId,
    ) ||
    browserGroups.some(
      (group) =>
        group.credentialOverride.kind === "set" &&
        group.credentialOverride.value === credentialId,
    )
  );
}

export function resetBrowserHostsAndRoutesForTests(seed = false) {
  browserGroups = seed ? cloneGroups(BROWSER_GROUP_FIXTURES) : [];
  browserHosts = seed ? cloneHosts(BROWSER_HOST_FIXTURES) : [];
  browserRoutes = seed ? cloneRoutes(BROWSER_ROUTE_FIXTURES) : [];
  nextBrowserGroupId = browserGroups.length + 1;
  nextBrowserHostId = browserHosts.length + 1;
  nextBrowserRouteId = browserRoutes.length + 1;
}

function validateAndResolveBrowserRepository() {
  validateBrowserGroupHierarchy();
  for (const group of browserGroups) {
    validateBrowserOverrideReferences(
      group.credentialOverride,
      group.jumpRouteOverride,
    );
    const effective = resolveBrowserReferences(
      group.parentGroupId,
      group.credentialOverride,
      group.jumpRouteOverride,
    );
    group.effectiveCredentialId = effective.credentialId;
    group.effectiveJumpRouteId = effective.jumpRouteId;
  }
  for (const host of browserHosts) {
    if (
      host.groupId &&
      !browserGroups.some((group) => group.id === host.groupId)
    ) {
      throw new Error("Group was not found");
    }
    validateBrowserOverrideReferences(
      host.credentialOverride,
      host.jumpRouteOverride,
    );
    const effective = resolveBrowserReferences(
      host.groupId,
      host.credentialOverride,
      host.jumpRouteOverride,
    );
    host.effectiveCredentialId = effective.credentialId;
    host.effectiveJumpRouteId = effective.jumpRouteId;
  }
  ensureBrowserRouteGraphHasNoCycles();
}

function validateBrowserGroupHierarchy() {
  const groups = new Map(browserGroups.map((group) => [group.id, group]));
  for (const group of browserGroups) {
    let current: GroupSummary | undefined = group;
    const visited = new Set<string>();
    let depth = 0;
    while (current) {
      if (visited.has(current.id)) throw new Error("Group cycle detected");
      visited.add(current.id);
      depth += 1;
      if (depth > MAX_GROUP_DEPTH) {
        throw new Error(`Group hierarchy exceeds ${MAX_GROUP_DEPTH} levels`);
      }
      if (!current.parentGroupId) break;
      current = groups.get(current.parentGroupId);
      if (!current) throw new Error("Parent Group was not found");
    }
  }
}

function validateBrowserOverrideReferences(
  credentialOverride: ReferenceOverride,
  jumpRouteOverride: ReferenceOverride,
) {
  if (
    jumpRouteOverride.kind === "set" &&
    !browserRoutes.some((route) => route.id === jumpRouteOverride.value)
  ) {
    throw new Error("Jump Route was not found");
  }
  if (credentialOverride.kind === "set" && !credentialOverride.value.trim()) {
    throw new Error("Credential was not found");
  }
}

function resolveBrowserReferences(
  groupId: string | null,
  credentialOverride: ReferenceOverride,
  jumpRouteOverride: ReferenceOverride,
) {
  let credential = resolvedSlot(credentialOverride);
  let jumpRoute = resolvedSlot(jumpRouteOverride);
  let currentGroupId = groupId;
  const visited = new Set<string>();
  let depth = 0;

  while (currentGroupId) {
    if (visited.has(currentGroupId)) throw new Error("Group cycle detected");
    visited.add(currentGroupId);
    depth += 1;
    if (depth > MAX_GROUP_DEPTH) {
      throw new Error(`Group hierarchy exceeds ${MAX_GROUP_DEPTH} levels`);
    }
    const group = browserGroups.find(
      (candidate) => candidate.id === currentGroupId,
    );
    if (!group) throw new Error("Group was not found");
    if (!credential.resolved) {
      credential = resolvedSlot(group.credentialOverride);
    }
    if (!jumpRoute.resolved) {
      jumpRoute = resolvedSlot(group.jumpRouteOverride);
    }
    currentGroupId = group.parentGroupId;
  }

  return {
    credentialId: credential.resolved ? credential.value : null,
    jumpRouteId: jumpRoute.resolved ? jumpRoute.value : null,
  };
}

function resolvedSlot(value: ReferenceOverride): {
  resolved: boolean;
  value: string | null;
} {
  switch (value.kind) {
    case "inherit":
      return { resolved: false, value: null };
    case "set":
      return { resolved: true, value: value.value };
    case "clear":
      return { resolved: true, value: null };
  }
}

function validateBrowserRoute(input: JumpRouteInput) {
  if (input.hostIds.length < 1 || input.hostIds.length > 32) {
    throw new Error("Jump Route must contain between 1 and 32 Hosts");
  }
  if (new Set(input.hostIds).size !== input.hostIds.length) {
    throw new Error("Jump Route cannot contain duplicate Hosts");
  }
  for (const hostId of input.hostIds) {
    if (!browserHosts.some((host) => host.id === hostId)) {
      throw new Error("Jump Route Host was not found");
    }
  }
}

function ensureBrowserRouteGraphHasNoCycles() {
  const routes = new Map(
    browserRoutes.map((route) => [route.id, route.hostIds]),
  );
  const graph = new Map<string, string[]>();
  for (const host of browserHosts) {
    if (host.effectiveJumpRouteId) {
      graph.set(host.id, routes.get(host.effectiveJumpRouteId) ?? []);
    }
  }

  const visiting = new Set<string>();
  const visited = new Set<string>();
  const visit = (hostId: string): boolean => {
    if (visited.has(hostId)) return false;
    if (visiting.has(hostId)) return true;
    visiting.add(hostId);
    for (const nextHost of graph.get(hostId) ?? []) {
      if (visit(nextHost)) return true;
    }
    visiting.delete(hostId);
    visited.add(hostId);
    return false;
  };

  for (const hostId of graph.keys()) {
    if (visit(hostId)) throw new Error("Jump Route cycle detected");
  }
}

function snapshotBrowserRepository() {
  return {
    groups: cloneGroups(browserGroups),
    hosts: cloneHosts(browserHosts),
    routes: cloneRoutes(browserRoutes),
  };
}

function restoreBrowserRepository(
  snapshot: ReturnType<typeof snapshotBrowserRepository>,
) {
  browserGroups = snapshot.groups;
  browserHosts = snapshot.hosts;
  browserRoutes = snapshot.routes;
}

function cloneOverride(value: ReferenceOverride): ReferenceOverride {
  return value.kind === "set"
    ? { kind: "set", value: value.value }
    : { ...value };
}

function cloneGroup(group: GroupSummary): GroupSummary {
  return {
    ...group,
    credentialOverride: cloneOverride(group.credentialOverride),
    jumpRouteOverride: cloneOverride(group.jumpRouteOverride),
  };
}

function cloneGroups(groups: GroupSummary[]): GroupSummary[] {
  return groups.map(cloneGroup);
}

function cloneHost(host: HostSummary): HostSummary {
  return {
    ...host,
    credentialOverride: cloneOverride(host.credentialOverride),
    jumpRouteOverride: cloneOverride(host.jumpRouteOverride),
  };
}

function cloneHosts(hosts: HostSummary[]): HostSummary[] {
  return hosts.map(cloneHost);
}

function cloneRoutes(routes: JumpRouteSummary[]): JumpRouteSummary[] {
  return routes.map((route) => ({
    ...route,
    hostIds: [...route.hostIds],
  }));
}
