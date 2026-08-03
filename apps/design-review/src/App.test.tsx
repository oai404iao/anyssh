import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";

describe("design review app", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("opens the clickable prototype and follows the primary action", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: /交互流程/ }));
    expect(
      screen.getByRole("heading", { level: 1, name: "欢迎与产品价值" }),
    ).toBeVisible();

    fireEvent.click(screen.getAllByRole("button", { name: /开始使用/ })[0]);
    expect(
      screen.getAllByRole("heading", {
        level: 1,
        name: "创建本地保险库",
      })[0],
    ).toBeVisible();
  });

  it("persists review status and notes locally", async () => {
    const first = render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "通过" }));
    fireEvent.change(screen.getByLabelText("评审备注"), {
      target: { value: "主按钮层级清楚" },
    });

    await waitFor(() => {
      expect(window.localStorage.getItem("anyssh-design-review-v1")).toContain(
        "主按钮层级清楚",
      );
    });

    first.unmount();
    render(<App />);

    expect(screen.getByLabelText("评审备注")).toHaveValue("主按钮层级清楚");
    expect(screen.getByRole("button", { name: "通过" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });
});
