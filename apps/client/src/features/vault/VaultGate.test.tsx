import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { VaultGate } from "./VaultGate";

describe("VaultGate", () => {
  it("introduces the product before local Vault setup", () => {
    render(
      <VaultGate
        error={null}
        onClearError={() => undefined}
        onSubmit={vi.fn()}
        status={{
          state: "uninitialized",
          vaultId: null,
          cipherVersion: null,
        }}
      />,
    );

    expect(
      screen.getByRole("heading", {
        name: "Your servers, securely within reach.",
      }),
    ).toBeVisible();
    expect(screen.queryByText(/Argon2id|SQLCipher|XChaCha/i)).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Get started" }));

    expect(
      screen.getByRole("heading", {
        name: "Create your encrypted Vault",
      }),
    ).toBeVisible();
    expect(screen.getByLabelText("PIN", { exact: true })).toBeVisible();
    expect(screen.getByLabelText("Confirm PIN")).toBeVisible();
  });

  it("submits matching PIN values and clears the local fields", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <VaultGate
        error={null}
        onClearError={() => undefined}
        onSubmit={onSubmit}
        status={{
          state: "uninitialized",
          vaultId: null,
          cipherVersion: null,
        }}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Get started" }));
    fireEvent.change(screen.getByLabelText("PIN", { exact: true }), {
      target: { value: "246810" },
    });
    fireEvent.change(screen.getByLabelText("Confirm PIN"), {
      target: { value: "246810" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "Create encrypted Vault" }),
    );

    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith("246810"));
    await waitFor(() =>
      expect(screen.getByLabelText("PIN", { exact: true })).toHaveValue(""),
    );
    expect(screen.getByLabelText("Confirm PIN")).toHaveValue("");
  });
});
