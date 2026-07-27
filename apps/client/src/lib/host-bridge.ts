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

let nextBrowserHostId = 1;
let nextBrowserRouteId = 1;

export async function listHosts(): Promise<HostSummary[]> {
  if (!isTauri()) return [];
  return invoke<HostSummary[]>("host_list");
}

export async function createHost(input: HostInput): Promise<HostSummary> {
  if (!isTauri()) {
    return {
      id: `browser-host-${nextBrowserHostId++}`,
      displayName: input.displayName,
      host: input.host,
      port: input.port,
      credentialId: input.credentialId ?? null,
      jumpRouteId: input.jumpRouteId ?? null,
    };
  }
  return invoke<HostSummary>("host_create", { request: input });
}

export async function updateHost(input: HostUpdate): Promise<HostSummary> {
  if (!isTauri()) {
    return {
      id: input.hostId,
      displayName: input.displayName,
      host: input.host,
      port: input.port,
      credentialId: input.credentialId ?? null,
      jumpRouteId: input.jumpRouteId ?? null,
    };
  }
  return invoke<HostSummary>("host_update", { request: input });
}

export async function deleteHost(hostId: string): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>("host_delete", { hostId });
}

export async function listJumpRoutes(): Promise<JumpRouteSummary[]> {
  if (!isTauri()) return [];
  return invoke<JumpRouteSummary[]>("jump_route_list");
}

export async function createJumpRoute(
  input: JumpRouteInput,
): Promise<JumpRouteSummary> {
  if (!isTauri()) {
    return {
      id: `browser-route-${nextBrowserRouteId++}`,
      label: input.label,
      hostIds: [...input.hostIds],
    };
  }
  return invoke<JumpRouteSummary>("jump_route_create", { request: input });
}

export async function updateJumpRoute(
  input: JumpRouteUpdate,
): Promise<JumpRouteSummary> {
  if (!isTauri()) {
    return {
      id: input.jumpRouteId,
      label: input.label,
      hostIds: [...input.hostIds],
    };
  }
  return invoke<JumpRouteSummary>("jump_route_update", { request: input });
}

export async function deleteJumpRoute(jumpRouteId: string): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>("jump_route_delete", { jumpRouteId });
}
