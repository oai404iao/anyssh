import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TerminalAuxiliaryKeyboard } from "./TerminalMobileControls";

describe("TerminalAuxiliaryKeyboard", () => {
  it("disables SSH key injection before the session is connected", () => {
    render(
      <TerminalAuxiliaryKeyboard
        connected={false}
        modifiers={{ alt: false, control: false }}
        onSend={vi.fn()}
        onToggleModifier={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Send Escape" })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Toggle Control modifier" }),
    ).toBeDisabled();
  });

  it("routes modifier and auxiliary key actions through typed callbacks", () => {
    const onSend = vi.fn();
    const onToggleModifier = vi.fn();
    render(
      <TerminalAuxiliaryKeyboard
        connected
        modifiers={{ alt: true, control: false }}
        onSend={onSend}
        onToggleModifier={onToggleModifier}
      />,
    );

    expect(
      screen.getByRole("button", { name: "Toggle Alt modifier" }),
    ).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(
      screen.getByRole("button", { name: "Toggle Control modifier" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Send Arrow Up" }));
    expect(onToggleModifier).toHaveBeenCalledWith("control");
    expect(onSend).toHaveBeenCalledWith("arrowUp");
  });
});
