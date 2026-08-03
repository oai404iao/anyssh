import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { WindowTitlebar } from "./WindowTitlebar";

describe("WindowTitlebar", () => {
  it("exposes standard window controls without invoking native APIs in browser QA", () => {
    render(<WindowTitlebar workspaceTitle="Local lab" />);

    expect(screen.getByText("AnySSH")).toBeInTheDocument();
    expect(screen.getByText("Local lab")).toBeInTheDocument();

    for (const name of [
      "Minimize window",
      "Maximize or restore window",
      "Close window",
    ]) {
      const control = screen.getByRole("button", { name });
      expect(() => fireEvent.click(control)).not.toThrow();
    }
  });
});
