import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MobileNavigation } from "./MobileNavigation";

const COUNTS = {
  credentials: 3,
  groups: 2,
  hosts: 4,
  knownHosts: 1,
  routes: 2,
  sessions: 2,
  snippets: 3,
};

describe("MobileNavigation", () => {
  it("keeps advanced management destinations in the More sheet", () => {
    const onNavigate = vi.fn();
    render(
      <MobileNavigation
        counts={COUNTS}
        nativeRuntime
        onLockVault={vi.fn()}
        onNavigate={onNavigate}
        workspaceView="hosts"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "More" }));
    expect(
      screen.getByRole("region", { name: "More workspace navigation" }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Jump routes/ }));
    expect(onNavigate).toHaveBeenCalledWith("routes");
  });
});
