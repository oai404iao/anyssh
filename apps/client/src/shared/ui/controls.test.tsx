import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CheckboxField } from "./CheckboxField";
import { NumberField } from "./NumberField";
import { SelectField } from "./SelectField";
import { SwitchField } from "./SwitchField";

describe("shared UI controls", () => {
  it("routes a Base UI Select choice through the typed wrapper", () => {
    const onValueChange = vi.fn();
    render(
      <SelectField
        label="Theme"
        onValueChange={onValueChange}
        options={[
          { label: "Follow system", value: "system" },
          { label: "Dark", value: "dark" },
        ]}
        value="system"
      />,
    );

    fireEvent.click(screen.getByRole("combobox", { name: "Theme" }));
    const option = screen.getByRole("option", { name: "Dark" });
    fireEvent.pointerDown(option);
    fireEvent.pointerUp(option);
    fireEvent.click(option);
    expect(onValueChange).toHaveBeenCalledWith("dark");
  });

  it("exposes Switch and Checkbox state with accessible labels", () => {
    const onSwitch = vi.fn();
    const onCheckbox = vi.fn();
    render(
      <>
        <SwitchField
          checked
          label="Programming ligatures"
          onCheckedChange={onSwitch}
        />
        <CheckboxField
          checked={false}
          label="Confirm command"
          onCheckedChange={onCheckbox}
        />
      </>,
    );

    expect(
      screen.getByRole("switch", { name: "Programming ligatures" }),
    ).toBeChecked();
    fireEvent.click(
      screen.getByRole("switch", { name: "Programming ligatures" }),
    );
    fireEvent.click(screen.getByRole("checkbox", { name: "Confirm command" }));
    expect(onSwitch).toHaveBeenCalledWith(false);
    expect(onCheckbox).toHaveBeenCalledWith(true);
  });

  it("replaces native number spinners with bounded steppers", () => {
    const onValueChange = vi.fn();
    render(
      <NumberField
        ariaLabel="Terminal font size"
        label="Font size"
        max={32}
        min={10}
        onValueChange={onValueChange}
        value={13}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", {
        name: "Increase Terminal font size",
      }),
    );

    expect(onValueChange).toHaveBeenCalledWith(14);
    expect(
      screen.getByRole("textbox", { name: "Terminal font size" }),
    ).toHaveValue("13");
  });
});
