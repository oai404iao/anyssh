import { invoke, isTauri } from "@tauri-apps/api/core";

export interface HostSummary {
  id: string;
  displayName: string;
  host: string;
  port: number;
  credentialId: string | null;
  jumpRouteId: string | null;
}

export interface HostInput {
  displayName: string;
  host: string;
  port: number;
  credentialId?: string | null;
  jumpRouteId?: string | null;
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

const BROWSER_HOST_FIXTURES: HostSummary[] = [
  {
    id: "browser-host-local",
    displayName: "Local lab",
    host: "127.0.0.1",
    port: 2222,
    credentialId: "browser-credential-local",
    jumpRouteId: null,
  },
  {
    id: "browser-host-edge",
    displayName: "Edge gateway",
    host: "10.0.0.8",
    port: 22,
    credentialId: "browser-credential-edge",
    jumpRouteId: null,
  },
  {
    id: "browser-host-database",
    displayName: "Database",
    host: "db.internal",
    port: 22,
    credentialId: "browser-credential-database",
    jumpRouteId: "browser-route-edge",
  },
];

const BROWSER_ROUTE_FIXTURES: JumpRouteSummary[] = [
  {
    id: "browser-route-edge",
    label: "Through edge gateway",
    hostIds: ["browser-host-edge"],
  },
];

let browserHosts = cloneHosts(BROWSER_HOST_FIXTURES);
let browserRoutes = cloneRoutes(BROWSER_ROUTE_FIXTURES);
let nextBrowserHostId = browserHosts.length + 1;
let nextBrowserRouteId = browserRoutes.length + 1;

export async function listHosts(): Promise<HostSummary[]> {
  if (!isTauri()) return cloneHosts(browserHosts);
  return invoke<HostSummary[]>("host_list");
}

export async function createHost(input: HostInput): Promise<HostSummary> {
  if (!isTauri()) {
    validateBrowserHostReferences(input);
    const summary = {
      id: `browser-host-${nextBrowserHostId++}`,
      displayName: input.displayName,
      host: input.host,
      port: input.port,
      credentialId: input.credentialId ?? null,
      jumpRouteId: input.jumpRouteId ?? null,
    };
    browserHosts.push(summary);
    try {
      ensureBrowserGraphHasNoCycles();
    } catch (error) {
      browserHosts.pop();
      throw error;
    }
    return { ...summary };
  }
  return invoke<HostSummary>("host_create", { request: input });
}

export async function updateHost(input: HostUpdate): Promise<HostSummary> {
  if (!isTauri()) {
    validateBrowserHostReferences(input);
    const index = browserHosts.findIndex((host) => host.id === input.hostId);
    if (index < 0) throw new Error("Host was not found");
    const previous = browserHosts[index];
    const summary = {
      id: input.hostId,
      displayName: input.displayName,
      host: input.host,
      port: input.port,
      credentialId: input.credentialId ?? null,
      jumpRouteId: input.jumpRouteId ?? null,
    };
    browserHosts[index] = summary;
    try {
      ensureBrowserGraphHasNoCycles();
    } catch (error) {
      if (previous) browserHosts[index] = previous;
      throw error;
    }
    return { ...summary };
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
    const summary = {
      id: `browser-route-${nextBrowserRouteId++}`,
      label: input.label,
      hostIds: [...input.hostIds],
    };
    browserRoutes.push(summary);
    try {
      ensureBrowserGraphHasNoCycles();
    } catch (error) {
      browserRoutes.pop();
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
    const previous = browserRoutes[index];
    const summary = {
      id: input.jumpRouteId,
      label: input.label,
      hostIds: [...input.hostIds],
    };
    browserRoutes[index] = summary;
    try {
      ensureBrowserGraphHasNoCycles();
    } catch (error) {
      if (previous) browserRoutes[index] = previous;
      throw error;
    }
    return { ...summary, hostIds: [...summary.hostIds] };
  }
  return invoke<JumpRouteSummary>("jump_route_update", { request: input });
}

export async function deleteJumpRoute(jumpRouteId: string): Promise<boolean> {
  if (!isTauri()) {
    if (browserHosts.some((host) => host.jumpRouteId === jumpRouteId)) {
      throw new Error("Jump Route is in use by a Host");
    }
    const previousLength = browserRoutes.length;
    browserRoutes = browserRoutes.filter((route) => route.id !== jumpRouteId);
    return browserRoutes.length !== previousLength;
  }
  return invoke<boolean>("jump_route_delete", { jumpRouteId });
}

export function resetBrowserHostsAndRoutesForTests(seed = false) {
  browserHosts = seed ? cloneHosts(BROWSER_HOST_FIXTURES) : [];
  browserRoutes = seed ? cloneRoutes(BROWSER_ROUTE_FIXTURES) : [];
  nextBrowserHostId = browserHosts.length + 1;
  nextBrowserRouteId = browserRoutes.length + 1;
}

function validateBrowserHostReferences(input: HostInput) {
  if (
    input.jumpRouteId &&
    !browserRoutes.some((route) => route.id === input.jumpRouteId)
  ) {
    throw new Error("Jump Route was not found");
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

function ensureBrowserGraphHasNoCycles() {
  const routes = new Map(
    browserRoutes.map((route) => [route.id, route.hostIds]),
  );
  const graph = new Map<string, string[]>();
  for (const host of browserHosts) {
    if (host.jumpRouteId) {
      graph.set(host.id, routes.get(host.jumpRouteId) ?? []);
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

function cloneHosts(hosts: HostSummary[]): HostSummary[] {
  return hosts.map((host) => ({ ...host }));
}

function cloneRoutes(routes: JumpRouteSummary[]): JumpRouteSummary[] {
  return routes.map((route) => ({
    ...route,
    hostIds: [...route.hostIds],
  }));
}
