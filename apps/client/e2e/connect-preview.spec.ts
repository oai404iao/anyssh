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

  await dialog.getByRole("button", { name: "Trust and continue" }).click();
  await expect(page.getByText("Interactive shell is active.")).toBeVisible();

  const terminalInput = page.getByRole("textbox", { name: "Terminal input" });
  await terminalInput.focus();
  await terminalInput.pressSequentially("unicode");
  await terminalInput.press("Enter");

  await page.getByRole("button", { name: "Disconnect" }).click();
  await expect(page.getByText("The SSH session has ended.")).toBeVisible();
  await expect(password).toHaveValue("");

  await page.getByRole("button", { name: "Connect" }).click();
  await expect(page.getByText("Interactive shell is active.")).toBeVisible();
  await expect(
    page.getByRole("dialog", { name: "Verify server identity" }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Disconnect" }).click();

  await page.getByRole("button", { name: /^Known hosts \d+$/ }).click();
  const localTrust = page
    .locator(".known-host-card")
    .filter({ hasText: "127.0.0.1:2222" });
  await expect(localTrust).toContainText("SHA256:");
  await localTrust.getByRole("button", { name: "Forget trust…" }).click();
  await expect(localTrust).toHaveCount(0);

  await page.getByRole("button", { name: /^Terminal \d+$/ }).click();
  await page.getByRole("button", { name: "Connect" }).click();
  await expect(
    page.getByRole("dialog", { name: "Verify server identity" }),
  ).toBeVisible();
});

test("blocks a changed Known Host without an accept action", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("textbox", { name: "Host", exact: true })
    .fill("changed.example");
  await page.getByRole("spinbutton", { name: "Port" }).fill("22");
  await page.getByLabel("Username").fill("anyssh");
  await page.getByLabel("Password", { exact: true }).fill("fixture");
  await page.getByRole("button", { name: "Connect" }).click();

  const changed = page.getByRole("alertdialog", { name: "Host key changed" });
  await expect(changed).toBeVisible();
  await expect(changed).toContainText("SHA256:trusted-browser-changed-key");
  await expect(changed).toContainText("Received");
  await expect(changed.getByRole("button", { name: /accept/i })).toHaveCount(0);
  await changed.getByRole("button", { name: "Open Known Hosts" }).click();
  await expect(
    page.getByRole("heading", { level: 2, name: "Known Hosts" }),
  ).toBeVisible();
  await expect(page.getByText("changed.example:22")).toBeVisible();
});

test("manages Groups, Credentials, Hosts, and ordered Jump Routes", async ({
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

  await page.getByRole("button", { name: "New system agent" }).click();
  const agentDialog = page.getByRole("dialog", {
    name: "New System Agent Credential",
  });
  await agentDialog.getByLabel("Credential label").fill("QA workstation agent");
  await agentDialog.getByLabel("Username").fill("qa-agent-user");
  const agentIdentity = agentDialog.getByLabel("SSH Agent identity");
  await expect(agentIdentity).toContainText("ssh-ed25519");
  await expect(agentIdentity).toContainText("SHA256:");
  await expect(agentDialog.locator('input[type="file"]')).toHaveCount(0);
  await expect(agentDialog.getByText(/Private Key/)).toHaveCount(1);
  await agentDialog
    .getByRole("button", { name: "Save Agent Credential" })
    .click();
  const agentCredential = page
    .locator(".resource-card")
    .filter({ hasText: "QA workstation agent" });
  await expect(agentCredential).toContainText("System Agent");
  await expect(agentCredential).toContainText("External signer");
  await expect(agentCredential).not.toContainText("SHA256:");

  await page.getByRole("button", { name: /^Groups \d+$/ }).click();
  await page.getByRole("button", { name: "New group" }).click();
  const rootGroupDialog = page.getByRole("dialog", { name: "New Group" });
  await rootGroupDialog.getByLabel("Group label").fill("QA root");
  await rootGroupDialog.getByLabel("Credential behavior").selectOption("set");
  await rootGroupDialog
    .getByLabel("Credential reference")
    .selectOption({ label: "QA deployment password · qa-user" });
  await rootGroupDialog.getByLabel("Jump Route behavior").selectOption("set");
  await rootGroupDialog
    .getByLabel("Jump Route reference")
    .selectOption({ label: "Through edge gateway" });
  await rootGroupDialog.getByRole("button", { name: "Save Group" }).click();
  await expect(
    page.locator(".resource-card").filter({ hasText: "QA root" }),
  ).toBeVisible();

  await page.getByRole("button", { name: "New group" }).click();
  const childGroupDialog = page.getByRole("dialog", { name: "New Group" });
  await childGroupDialog.getByLabel("Group label").fill("QA child");
  await childGroupDialog
    .getByLabel("Parent Group")
    .selectOption({ label: "QA root" });
  await childGroupDialog
    .getByLabel("Jump Route behavior")
    .selectOption("clear");
  await childGroupDialog.getByRole("button", { name: "Save Group" }).click();
  const childGroup = page
    .locator(".resource-card")
    .filter({ hasText: "QA child" });
  await expect(childGroup).toContainText("Inherited");
  await expect(childGroup).toContainText("Cleared");

  await page.getByRole("button", { name: /^Hosts \d+$/ }).click();
  await page.getByRole("button", { name: "New host" }).click();
  const hostDialog = page.getByRole("dialog", { name: "New Host" });
  await hostDialog.getByLabel("Display name").fill("QA target");
  await hostDialog.getByLabel("Host").fill("qa.internal");
  await hostDialog.getByRole("spinbutton", { name: "Port" }).fill("2202");
  await hostDialog.getByLabel("Group").selectOption({ label: "QA child" });
  await hostDialog.getByRole("button", { name: "Save Host" }).click();
  const target = page
    .locator(".resource-card")
    .filter({ hasText: "QA target" });
  await expect(target).toContainText("qa-user");
  await expect(target).toContainText("Direct · Inherited");

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
