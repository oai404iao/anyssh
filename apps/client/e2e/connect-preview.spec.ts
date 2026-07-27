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

test("manages Credentials, Hosts, and ordered Jump Routes", async ({
  page,
}) => {
  await page.goto("/");

  await page.getByRole("button", { name: /^Credentials \d+$/ }).click();
  await expect(
    page.getByRole("heading", {
      level: 2,
      name: "Credentials",
      exact: true,
    }),
  ).toBeVisible();

  await page.getByRole("button", { name: "New password" }).click();
  const passwordDialog = page.getByRole("dialog", {
    name: "New Password Credential",
  });
  await passwordDialog
    .getByLabel("Credential label")
    .fill("QA deployment password");
  await passwordDialog.getByLabel("Username").fill("qa-user");
  await passwordDialog.getByLabel("Password").fill("qa-secret-not-returned");
  await passwordDialog.getByRole("button", { name: "Save Credential" }).click();
  await expect(page.getByText("QA deployment password")).toBeVisible();
  await expect(page.getByText("qa-secret-not-returned")).toHaveCount(0);

  await page.getByRole("button", { name: "Import private key" }).click();
  const keyDialog = page.getByRole("dialog", {
    name: "Import Private Key",
  });
  await keyDialog.getByLabel("Credential label").fill("QA imported key");
  await keyDialog.getByLabel("Username").fill("qa-key-user");
  await expect(keyDialog.locator('input[type="file"]')).toHaveCount(0);
  await expect(keyDialog.getByLabel("Passphrase")).toHaveCount(0);
  await keyDialog.getByRole("button", { name: "Choose private key" }).click();
  await expect(page.getByText("QA imported key")).toBeVisible();

  await page.getByRole("button", { name: /^Hosts \d+$/ }).click();
  await page.getByRole("button", { name: "New host" }).click();
  const hostDialog = page.getByRole("dialog", { name: "New Host" });
  await hostDialog.getByLabel("Display name").fill("QA target");
  await hostDialog.getByLabel("Host").fill("qa.internal");
  await hostDialog.getByRole("spinbutton", { name: "Port" }).fill("2202");
  await hostDialog
    .getByLabel("Credential")
    .selectOption({ label: "QA deployment password · qa-user" });
  await hostDialog.getByRole("button", { name: "Save Host" }).click();
  await expect(
    page.locator(".resource-card").filter({ hasText: "QA target" }),
  ).toBeVisible();

  await page.getByRole("button", { name: /^Jump routes \d+$/ }).click();
  await page.getByRole("button", { name: "New route" }).click();
  const routeDialog = page.getByRole("dialog", { name: "New Jump Route" });
  await routeDialog.getByLabel("Route label").fill("QA ordered route");
  await routeDialog
    .getByLabel("Add Host")
    .selectOption({ label: "Local lab · 127.0.0.1:2222" });
  await routeDialog.getByRole("button", { name: "Add", exact: true }).click();
  await routeDialog
    .getByLabel("Add Host")
    .selectOption({ label: "Edge gateway · 10.0.0.8:22" });
  await routeDialog.getByRole("button", { name: "Add", exact: true }).click();
  await routeDialog
    .getByRole("button", { name: "Move Edge gateway up" })
    .click();
  await routeDialog.getByRole("button", { name: "Save Jump Route" }).click();
  const createdRoute = page
    .locator(".resource-card")
    .filter({ hasText: "QA ordered route" });
  await expect(createdRoute.locator("li").nth(0)).toHaveText("Edge gateway");
  await expect(createdRoute.locator("li").nth(1)).toHaveText("Local lab");

  const inUseRoute = page
    .locator(".resource-card")
    .filter({ hasText: "Through edge gateway" });
  await inUseRoute.getByRole("button", { name: "Delete" }).click();
  await inUseRoute.getByRole("button", { name: "Confirm delete" }).click();
  await expect(page.getByRole("alert")).toContainText("in use");

  await page.setViewportSize({ width: 1024, height: 768 });
  await expect(page.locator(".primary-nav .nav-item").first()).toHaveCSS(
    "font-size",
    "0px",
  );
});
