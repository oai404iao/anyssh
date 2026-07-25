import { expect, test } from "@playwright/test";

test("connects through the host-key preview flow", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Local lab" })).toBeVisible();
  await expect(
    page.getByText("Browser QA mode", { exact: true }),
  ).toBeVisible();

  const password = page.getByLabel("Password", { exact: true });
  await password.fill("fixture-password");
  await page.getByRole("button", { name: "Show password" }).click();
  await expect(password).toHaveAttribute("type", "text");
  await page.getByRole("button", { name: "Hide password" }).click();
  await expect(password).toHaveAttribute("type", "password");

  await page.getByRole("button", { name: "Connect" }).click();
  const dialog = page.getByRole("dialog", { name: "Verify server identity" });
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText("SHA256:");
  await expect(password).toHaveValue("");

  await dialog.getByRole("button", { name: "Trust for this session" }).click();
  await expect(page.getByText("Interactive shell is active.")).toBeVisible();

  const terminalInput = page.getByRole("textbox", { name: "Terminal input" });
  await terminalInput.focus();
  await terminalInput.pressSequentially("unicode");
  await terminalInput.press("Enter");

  await page.getByRole("button", { name: "Disconnect" }).click();
  await expect(page.getByText("The SSH session has ended.")).toBeVisible();
  await expect(password).toHaveValue("");
});
