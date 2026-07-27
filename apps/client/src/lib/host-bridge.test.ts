import { describe, expect, it } from "vitest";
import {
  createHost,
  createJumpRoute,
  deleteHost,
  deleteJumpRoute,
  listHosts,
  listJumpRoutes,
  updateHost,
  updateJumpRoute,
} from "./host-bridge";

describe("browser preview Host and Jump Route bridge", () => {
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
      hostIds: [jump.id, target.id],
    });

    expect(updatedTarget).toMatchObject({
      id: target.id,
      credentialId: "cred-target",
      jumpRouteId: route.id,
    });
    expect(updatedRoute.hostIds).toEqual([jump.id, target.id]);
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
    await expect(listHosts()).resolves.toEqual([]);
    await expect(listJumpRoutes()).resolves.toEqual([]);
    await expect(deleteHost(target.id)).resolves.toBe(false);
    await expect(deleteJumpRoute(route.id)).resolves.toBe(false);
  });
});
