import { beforeEach, describe, expect, it } from "vitest";
import {
  createGroup,
  createHost,
  createJumpRoute,
  deleteGroup,
  deleteHost,
  deleteJumpRoute,
  listGroups,
  listHosts,
  listJumpRoutes,
  resetBrowserHostsAndRoutesForTests,
  updateGroup,
  updateHost,
  updateJumpRoute,
} from "./host-bridge";

describe("browser preview Group, Host, and Jump Route bridge", () => {
  beforeEach(() => {
    resetBrowserHostsAndRoutesForTests();
  });

  it("keeps ordered opaque references without embedding credentials", async () => {
    const jump = await createHost({
      displayName: "Jump",
      host: "jump.internal",
      port: 22,
      credentialOverride: { kind: "set", value: "cred-jump" },
      jumpRouteOverride: { kind: "inherit" },
    });
    const route = await createJumpRoute({
      label: "Production",
      hostIds: [jump.id],
    });
    const target = await createHost({
      displayName: "Target",
      host: "target.internal",
      port: 2222,
      credentialOverride: { kind: "set", value: "cred-target" },
      jumpRouteOverride: { kind: "set", value: route.id },
    });
    const secondJump = await createHost({
      displayName: "Jump two",
      host: "jump-two.internal",
      port: 22,
      credentialOverride: { kind: "set", value: "cred-jump-two" },
      jumpRouteOverride: { kind: "inherit" },
    });
    const updatedTarget = await updateHost({
      hostId: target.id,
      displayName: "Updated target",
      host: "target.internal",
      port: 2222,
      credentialOverride: { kind: "set", value: "cred-target" },
      jumpRouteOverride: { kind: "set", value: route.id },
    });
    const updatedRoute = await updateJumpRoute({
      jumpRouteId: route.id,
      label: "Updated production",
      hostIds: [jump.id, secondJump.id],
    });

    expect(updatedTarget).toMatchObject({
      id: target.id,
      effectiveCredentialId: "cred-target",
      effectiveJumpRouteId: route.id,
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

  it("resolves Inherit, Set, and Clear through parent Groups", async () => {
    const jump = await createHost({
      displayName: "Jump",
      host: "jump.internal",
      port: 22,
      credentialOverride: { kind: "set", value: "cred-jump" },
      jumpRouteOverride: { kind: "inherit" },
    });
    const route = await createJumpRoute({
      label: "Inherited route",
      hostIds: [jump.id],
    });
    const root = await createGroup({
      label: "Root",
      credentialOverride: { kind: "set", value: "cred-root" },
      jumpRouteOverride: { kind: "set", value: route.id },
    });
    const child = await createGroup({
      label: "Child",
      parentGroupId: root.id,
      credentialOverride: { kind: "inherit" },
      jumpRouteOverride: { kind: "clear" },
    });
    const target = await createHost({
      displayName: "Target",
      host: "target.internal",
      port: 22,
      groupId: child.id,
      credentialOverride: { kind: "inherit" },
      jumpRouteOverride: { kind: "inherit" },
    });

    expect(target.effectiveCredentialId).toBe("cred-root");
    expect(target.effectiveJumpRouteId).toBeNull();

    const inheritedChild = await updateGroup({
      groupId: child.id,
      label: child.label,
      parentGroupId: root.id,
      credentialOverride: { kind: "inherit" },
      jumpRouteOverride: { kind: "inherit" },
    });
    expect(inheritedChild.effectiveJumpRouteId).toBe(route.id);
    expect(
      (await listHosts()).find((host) => host.id === target.id)
        ?.effectiveJumpRouteId,
    ).toBe(route.id);

    await expect(
      updateGroup({
        groupId: root.id,
        label: root.label,
        parentGroupId: child.id,
        credentialOverride: root.credentialOverride,
        jumpRouteOverride: root.jumpRouteOverride,
      }),
    ).rejects.toThrow("cycle");
    await expect(deleteGroup(root.id)).rejects.toThrow("in use");
    await expect(deleteGroup(child.id)).rejects.toThrow("in use");
    await expect(deleteJumpRoute(route.id)).rejects.toThrow("in use");
    await expect(listGroups()).resolves.toHaveLength(2);
  });

  it("rejects browser preview Route cycles including inherited Routes", async () => {
    const host = await createHost({
      displayName: "Cycle target",
      host: "cycle.internal",
      port: 22,
      credentialOverride: { kind: "set", value: "cred-cycle" },
      jumpRouteOverride: { kind: "inherit" },
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
        credentialOverride: host.credentialOverride,
        jumpRouteOverride: { kind: "set", value: route.id },
      }),
    ).rejects.toThrow("cycle");

    const group = await createGroup({
      label: "Cycle Group",
      credentialOverride: { kind: "inherit" },
      jumpRouteOverride: { kind: "inherit" },
    });
    await updateHost({
      hostId: host.id,
      displayName: host.displayName,
      host: host.host,
      port: host.port,
      groupId: group.id,
      credentialOverride: host.credentialOverride,
      jumpRouteOverride: { kind: "inherit" },
    });
    await expect(
      updateGroup({
        groupId: group.id,
        label: group.label,
        credentialOverride: { kind: "inherit" },
        jumpRouteOverride: { kind: "set", value: route.id },
      }),
    ).rejects.toThrow("cycle");
  });
});
