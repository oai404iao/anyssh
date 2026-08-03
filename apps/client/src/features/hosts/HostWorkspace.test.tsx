import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { HostWorkspace } from "./HostWorkspace";

const GROUP = {
  id: "group-production",
  label: "Production",
  parentGroupId: null,
  credentialOverride: { kind: "inherit" as const },
  jumpRouteOverride: { kind: "inherit" as const },
  effectiveCredentialId: null,
  effectiveJumpRouteId: null,
};

const CREDENTIAL = {
  id: "credential-deploy",
  label: "Deploy key",
  username: "deploy",
  kind: "privateKey" as const,
};

const HOSTS = [
  {
    id: "host-production",
    displayName: "Production server",
    host: "prod.example.com",
    port: 22,
    groupId: GROUP.id,
    credentialOverride: {
      kind: "set" as const,
      value: CREDENTIAL.id,
    },
    jumpRouteOverride: { kind: "inherit" as const },
    effectiveCredentialId: CREDENTIAL.id,
    effectiveJumpRouteId: null,
  },
  {
    id: "host-home",
    displayName: "Home NAS",
    host: "192.168.1.20",
    port: 22,
    groupId: null,
    credentialOverride: { kind: "clear" as const },
    jumpRouteOverride: { kind: "inherit" as const },
    effectiveCredentialId: null,
    effectiveJumpRouteId: null,
  },
];

describe("HostWorkspace", () => {
  it("filters Hosts and opens a product detail view", () => {
    const onConnectHost = vi.fn();
    const onOpenHost = vi.fn();
    render(
      <HostWorkspace
        credentials={[CREDENTIAL]}
        groups={[GROUP]}
        hosts={HOSTS}
        loading={false}
        nativeRuntime
        onChanged={vi.fn()}
        onConnectHost={onConnectHost}
        onOpenHost={onOpenHost}
        routes={[]}
      />,
    );

    fireEvent.change(screen.getByLabelText("Search Hosts"), {
      target: { value: "prod.example.com" },
    });

    expect(screen.getByText("Production server")).toBeVisible();
    expect(screen.queryByText("Home NAS")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    const detail = screen.getByRole("region", {
      name: "Production server",
    });
    expect(within(detail).getByText("Deploy key · Private Key")).toBeVisible();

    fireEvent.click(within(detail).getByRole("button", { name: "Connect" }));
    expect(onConnectHost).toHaveBeenCalledWith(HOSTS[0]);
    expect(onOpenHost).not.toHaveBeenCalled();
  });

  it("retains the existing Open accessible action for automation", () => {
    const onOpenHost = vi.fn();
    render(
      <HostWorkspace
        credentials={[CREDENTIAL]}
        groups={[GROUP]}
        hosts={HOSTS}
        loading={false}
        nativeRuntime={false}
        onChanged={vi.fn()}
        onConnectHost={vi.fn()}
        onOpenHost={onOpenHost}
        routes={[]}
      />,
    );

    const productionCard = screen
      .getByText("Production server")
      .closest("article");
    expect(productionCard).not.toBeNull();
    expect(
      within(productionCard as HTMLElement).getByRole("button", {
        name: "Open",
      }),
    ).toHaveTextContent("Open session");

    fireEvent.click(
      within(productionCard as HTMLElement).getByRole("button", {
        name: "Details",
      }),
    );
    const detail = screen.getByRole("region", {
      name: "Production server",
    });
    fireEvent.click(
      within(detail).getByRole("button", { name: "Open session" }),
    );
    expect(onOpenHost).toHaveBeenCalledWith(HOSTS[0]);
  });
});
