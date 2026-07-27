import { beforeEach, describe, expect, it } from "vitest";
import {
  createHost,
  createJumpRoute,
  deleteHost,
  deleteJumpRoute,
  listHosts,
  listJumpRoutes,
  resetBrowserHostsAndRoutesForTests,
  updateHost,
  updateJumpRoute,
} from "./host-bridge";

describe("browser preview Host and Jump Route bridge", () => {
  beforeEach(() => {
    resetBrowserHostsAndRoutesForTests();
  });

  it("keeps ordered opaque references without embedding credentials", async () => {
    const jump = await createHost({
      displayName: "Jump",
      host: "jump.internal",
      port: 22,
      credentialId: "cred-jump",
    });
    const route = await createJumpRoute({
      label: "Production",
      hostIds: [jump.id],
    });
    const target = await createHost({
      displayName: "Target",
      host: "target.internal",
      port: 2222,
      credentialId: "cred-target",
      jumpRouteId: route.id,
    });
    const secondJump = await createHost({
      displayName: "Jump two",
      host: "jump-two.internal",
      port: 22,
      credentialId: "cred-jump-two",
    });
    const updatedTarget = await updateHost({
      hostId: target.id,
      displayName: "Updated target",
      host: "target.internal",
      port: 2222,
      credentialId: "cred-target",
      jumpRouteId: route.id,
    });
    const updatedRoute = await updateJumpRoute({
      jumpRouteId: route.id,
      label: "Updated production",
      hostIds: [jump.id, secondJump.id],
    });

    expect(updatedTarget).toMatchObject({
      id: target.id,
      credentialId: "cred-target",
      jumpRouteId: route.id,
    });
    expect(updatedRoute.hostIds).toEqual([jump.id, secondJump.id]);
    const serialized = JSON.stringify([
      jump,
      route,
      target,
      updatedTarget,
      updatedRoute,
    ]);
    expect(serialized).not.toContain("password");
    expect(serialized).not.toContain("privateKey");
    expect(serialized).not.toContain("passphrase");
    await expect(listHosts()).resolves.toHaveLength(3);
    await expect(listJumpRoutes()).resolves.toEqual([updatedRoute]);
    await expect(deleteHost(jump.id)).rejects.toThrow("in use");
    await expect(deleteHost(target.id)).resolves.toBe(true);
    await expect(deleteJumpRoute(route.id)).resolves.toBe(true);
  });

  it("rejects browser preview Route cycles", async () => {
    const host = await createHost({
      displayName: "Cycle target",
      host: "cycle.internal",
      port: 22,
      credentialId: "cred-cycle",
    });
    const route = await createJumpRoute({
      label: "Cycle route",
      hostIds: [host.id],
    });

    await expect(
      updateHost({
        hostId: host.id,
        displayName: host.displayName,
        host: host.host,
        port: host.port,
        credentialId: host.credentialId,
        jumpRouteId: route.id,
      }),
    ).rejects.toThrow("cycle");
  });
});
