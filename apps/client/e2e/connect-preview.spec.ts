import { expect, test, type Page } from "@playwright/test";

async function selectUiOption(page: Page, label: string, value: string) {
  await page.getByRole("combobox", { name: label }).click();
  await page
    .locator(`[role="option"][data-value=${JSON.stringify(value)}]`)
    .click();
}

async function setUiSwitch(page: Page, label: string, checked: boolean) {
  const control = page.getByRole("switch", { name: label });
  if ((await control.getAttribute("aria-checked")) !== String(checked)) {
    await control.click();
  }
}

test("keeps the Android product shell in landscape by platform identity", async ({
  browser,
}) => {
  const context = await browser.newContext({
    userAgent:
      "Mozilla/5.0 (Linux; Android 16; Pixel 9) AppleWebKit/537.36 Chrome/140 Mobile Safari/537.36",
    viewport: { width: 844, height: 390 },
  });
  const page = await context.newPage();
  await page.goto("/");

  await expect(page.locator(".app-shell")).toHaveClass(/compact-product-shell/);
  await expect(
    page.getByRole("navigation", { name: "Terminal actions" }),
  ).toBeVisible();
  await expect(page.locator(".sidebar")).toBeHidden();

  await context.close();
});

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

test("manages Local, Dynamic, and Remote forwards as session metadata", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByLabel("Password", { exact: true }).fill("fixture");
  await page.getByRole("button", { name: "Connect" }).click();
  await page
    .getByRole("dialog", { name: "Verify server identity" })
    .getByRole("button", { name: "Trust and continue" })
    .click();

  const forwarding = page.getByRole("region", { name: "Port forwarding" });
  await forwarding.getByRole("button", { name: "Start forward" }).click();
  await expect(
    forwarding.getByRole("button", {
      name: /^Stop local forward on port \d+$/,
    }),
  ).toBeVisible();
  await expect(forwarding).toContainText("1/16");

  await forwarding
    .getByRole("combobox", { name: "Port forward type" })
    .selectOption("dynamic");
  await expect(
    forwarding.getByRole("textbox", {
      name: "Port forward destination host",
    }),
  ).toHaveCount(0);
  await forwarding.getByRole("button", { name: "Start forward" }).click();
  const stopDynamic = forwarding.getByRole("button", {
    name: /^Stop dynamic forward on port \d+$/,
  });
  await expect(stopDynamic).toBeVisible();

  await forwarding
    .getByRole("combobox", { name: "Port forward type" })
    .selectOption("remote");
  await expect(
    forwarding.getByRole("textbox", {
      name: "Port forward destination host",
    }),
  ).toBeVisible();
  await forwarding.getByRole("button", { name: "Start forward" }).click();
  await expect(
    forwarding.getByRole("button", {
      name: /^Stop remote forward on port \d+$/,
    }),
  ).toBeVisible();
  await expect(forwarding).toContainText("3/16");
  await expect(forwarding).not.toContainText("fixture");

  await stopDynamic.click();
  await expect(stopDynamic).toHaveCount(0);
  await expect(forwarding).toContainText("2/16");

  await page.getByRole("button", { name: "Disconnect" }).click();
  await expect(
    forwarding.getByRole("button", { name: /^Stop .* forward on port \d+$/ }),
  ).toHaveCount(0);
  await expect(
    forwarding.getByRole("button", { name: "Session required" }),
  ).toBeDisabled();
});

test("keeps active forwards isolated between session tabs", async ({
  page,
}) => {
  await page.goto("/");

  async function connectActiveTab(
    displayName: string,
    host: string,
  ): Promise<void> {
    await page.getByLabel("Display name").fill(displayName);
    await page.getByRole("textbox", { name: "Host", exact: true }).fill(host);
    await page.getByLabel("Password", { exact: true }).fill("fixture");
    await page.getByRole("button", { name: "Connect" }).click();
    await page
      .getByRole("dialog", { name: "Verify server identity" })
      .getByRole("button", { name: "Trust and continue" })
      .click();
    await expect(page.getByText("Interactive shell is active.")).toBeVisible();
  }

  await connectActiveTab("Forward one", "forward-one.example");
  let forwarding = page.getByRole("region", { name: "Port forwarding" });
  await forwarding.getByRole("button", { name: "Start forward" }).click();
  await expect(
    forwarding.getByRole("button", {
      name: /^Stop local forward on port \d+$/,
    }),
  ).toBeVisible();

  await page.getByRole("button", { name: "New session tab" }).click();
  await connectActiveTab("Forward two", "forward-two.example");
  forwarding = page.getByRole("region", { name: "Port forwarding" });
  await expect(
    forwarding.getByRole("button", { name: /^Stop .* forward on port \d+$/ }),
  ).toHaveCount(0);
  await forwarding
    .getByRole("combobox", { name: "Port forward type" })
    .selectOption("dynamic");
  await forwarding.getByRole("button", { name: "Start forward" }).click();
  await expect(
    forwarding.getByRole("button", {
      name: /^Stop dynamic forward on port \d+$/,
    }),
  ).toBeVisible();

  await page.getByRole("tab", { name: "Forward one" }).click();
  forwarding = page.getByRole("region", { name: "Port forwarding" });
  await expect(
    forwarding.getByRole("button", {
      name: /^Stop local forward on port \d+$/,
    }),
  ).toBeVisible();
  await expect(
    forwarding.getByRole("button", {
      name: /^Stop dynamic forward on port \d+$/,
    }),
  ).toHaveCount(0);

  await page
    .getByRole("button", { name: "Close Forward one session tab" })
    .click();
  await expect(page.getByRole("tab", { name: "Forward two" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(
    page.getByRole("region", { name: "Port forwarding" }).getByRole("button", {
      name: /^Stop dynamic forward on port \d+$/,
    }),
  ).toBeVisible();
});

test("completes a session-bound keyboard-interactive challenge", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("textbox", { name: "Host", exact: true })
    .fill("multi-otp.example");
  await page.getByRole("spinbutton", { name: "Port" }).fill("22");
  await page.getByLabel("Username").fill("anyssh");
  await page.getByLabel("Authentication").selectOption("keyboardInteractive");
  await expect(page.getByLabel("Password", { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Connect" }).click();

  const hostKey = page.getByRole("dialog", {
    name: "Verify server identity",
  });
  await hostKey.getByRole("button", { name: "Trust and continue" }).click();

  const challenge = page.getByRole("dialog", {
    name: "Multi-factor authentication",
  });
  await expect(challenge).toContainText("Target host");
  await expect(challenge).toContainText("multi-otp.example:22");
  const response = challenge.getByLabel("Verification code:");
  const device = challenge.getByLabel("Device name:");
  await expect(response).toHaveAttribute("type", "password");
  await expect(device).toHaveAttribute("type", "text");
  await response.fill("654321");
  await device.fill("qa-laptop");
  await challenge.getByRole("button", { name: "Continue" }).click();

  await expect(challenge).toHaveCount(0);
  await expect(page.getByText("Interactive shell is active.")).toBeVisible();
  await expect(page.getByText("654321", { exact: true })).toHaveCount(0);
  await expect(page.getByText("qa-laptop", { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Disconnect" }).click();
});

test("cancels and clears a pending keyboard-interactive challenge", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("textbox", { name: "Host", exact: true })
    .fill("otp.example");
  await page.getByLabel("Authentication").selectOption("keyboardInteractive");
  await page.getByRole("button", { name: "Connect" }).click();
  await page
    .getByRole("dialog", { name: "Verify server identity" })
    .getByRole("button", { name: "Trust and continue" })
    .click();

  const challenge = page.getByRole("dialog", {
    name: "Multi-factor authentication",
  });
  await challenge.getByLabel("Verification code:").fill("cancelled-response");
  await challenge
    .getByRole("button", { name: "Cancel authentication" })
    .click();

  await expect(challenge).toHaveCount(0);
  await expect(
    page.getByText("Additional authentication was cancelled."),
  ).toBeVisible();
  await expect(
    page.getByText("cancelled-response", { exact: true }),
  ).toHaveCount(0);
});

test("routes simultaneous authentication challenges to their owning tabs", async ({
  page,
}) => {
  await page.goto("/");

  await page.getByLabel("Display name").fill("Trust setup");
  await page
    .getByRole("textbox", { name: "Host", exact: true })
    .fill("otp.example");
  await page.getByLabel("Authentication").selectOption("keyboardInteractive");
  await page.getByRole("button", { name: "Connect" }).click();
  await page
    .getByRole("dialog", { name: "Verify server identity" })
    .getByRole("button", { name: "Trust and continue" })
    .click();
  await page
    .getByRole("dialog", { name: "Multi-factor authentication" })
    .getByRole("button", { name: "Cancel authentication" })
    .click();
  await page
    .getByRole("button", { name: "Close Trust setup session tab" })
    .click();

  async function configureInteractiveTab(displayName: string): Promise<void> {
    await page.getByLabel("Display name").fill(displayName);
    await page
      .getByRole("textbox", { name: "Host", exact: true })
      .fill("otp.example");
    await page.getByLabel("Authentication").selectOption("keyboardInteractive");
  }

  await configureInteractiveTab("OTP one");
  await page.getByRole("button", { name: "New session tab" }).click();
  await configureInteractiveTab("OTP two");

  await page.getByRole("tab", { name: "OTP one" }).click();
  await page.getByRole("button", { name: "Connect" }).click();
  await page.getByRole("tab", { name: "OTP two" }).click({ force: true });
  await page.getByRole("button", { name: "Connect" }).click();

  await expect(page.locator(".session-tab-pending")).toHaveCount(2);
  await expect(page.getByRole("tab", { name: /OTP one/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  let challenge = page.getByRole("dialog", {
    name: "Multi-factor authentication",
  });
  await challenge.getByLabel("Verification code:").fill("111111");
  await challenge.getByRole("button", { name: "Continue" }).click();
  await expect(page.getByText("Interactive shell is active.")).toBeVisible();

  await page.getByRole("tab", { name: /OTP two/ }).click();
  challenge = page.getByRole("dialog", {
    name: "Multi-factor authentication",
  });
  await expect(challenge).toBeVisible();
  await challenge.getByLabel("Verification code:").fill("222222");
  await challenge.getByRole("button", { name: "Continue" }).click();
  await expect(page.getByText("Interactive shell is active.")).toBeVisible();
  await expect(page.locator(".session-tab-pending")).toHaveCount(0);
  await expect(page.getByText("111111", { exact: true })).toHaveCount(0);
  await expect(page.getByText("222222", { exact: true })).toHaveCount(0);
});

test("keeps concurrent session tabs isolated when one closes", async ({
  page,
}) => {
  await page.goto("/");

  async function connectActiveTab(
    displayName: string,
    host: string,
  ): Promise<void> {
    await page.getByLabel("Display name").fill(displayName);
    await page.getByRole("textbox", { name: "Host", exact: true }).fill(host);
    await page.getByLabel("Password", { exact: true }).fill("fixture");
    await page.getByRole("button", { name: "Connect" }).click();
    await page
      .getByRole("dialog", { name: "Verify server identity" })
      .getByRole("button", { name: "Trust and continue" })
      .click();
    await expect(page.getByText("Interactive shell is active.")).toBeVisible();
  }

  await page.getByLabel("Display name").fill("Session one");
  await page
    .getByLabel("Password", { exact: true })
    .fill("draft-password-must-clear");
  await page.getByRole("button", { name: "New session tab" }).click();
  await page.getByRole("tab", { name: "Session one" }).click();
  await expect(page.getByLabel("Password", { exact: true })).toHaveValue("");

  await connectActiveTab("Session one", "one.example");
  await page.getByRole("tab", { name: "Local lab" }).click();
  await connectActiveTab("Session two", "two.example");

  await expect(page.getByRole("tab")).toHaveCount(2);
  const secondTab = page.getByRole("tab", { name: "Session two" });
  await secondTab.focus();
  await secondTab.press("ArrowLeft");
  await expect(page.getByRole("tab", { name: "Session one" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  const firstPanel = page.getByRole("tabpanel");
  await expect(firstPanel).toContainText("one.example:2222");
  const firstTerminal = page.getByRole("textbox", { name: "Terminal input" });
  await firstTerminal.focus();
  await firstTerminal.pressSequentially("unicode");
  await firstTerminal.press("Enter");
  await expect(firstPanel).toContainText("中文");

  await page.getByRole("tab", { name: "Session two" }).click();
  const secondPanel = page.getByRole("tabpanel");
  await expect(secondPanel).toContainText("two.example:2222");
  await expect(secondPanel).not.toContainText("中文");

  await page.getByRole("tab", { name: "Session one" }).click();
  await page
    .getByRole("button", { name: "Close Session one session tab" })
    .click();
  await expect(page.getByRole("tab", { name: "Session one" })).toHaveCount(0);
  await expect(page.getByRole("tab", { name: "Session two" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page.getByText("Interactive shell is active.")).toBeVisible();

  const remainingTerminal = page.getByRole("textbox", {
    name: "Terminal input",
  });
  await remainingTerminal.focus();
  await remainingTerminal.pressSequentially("help");
  await remainingTerminal.press("Enter");
  await expect(page.getByRole("tabpanel")).toContainText("Available commands");
});

test("closes a connecting tab without accepting late events", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByLabel("Display name").fill("Closing session");
  await page.getByLabel("Password", { exact: true }).fill("fixture");
  await page.getByRole("button", { name: "Connect" }).click();
  await page
    .getByRole("button", { name: "Close Closing session session tab" })
    .click({ force: true });

  await page.waitForTimeout(350);
  await expect(
    page.getByRole("dialog", { name: "Verify server identity" }),
  ).toHaveCount(0);
  await expect(page.getByRole("tab")).toHaveCount(1);
  await expect(page.getByRole("tab", { name: "Local lab" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page.locator(".status-pill")).toHaveText("Ready");
});

test("enforces the eight-tab session limit without evicting a tab", async ({
  page,
}) => {
  await page.goto("/");
  const newTab = page.getByRole("button", { name: "New session tab" });

  for (let index = 1; index < 8; index += 1) {
    await newTab.click();
  }

  await expect(page.getByRole("tab")).toHaveCount(8);
  await expect(newTab).toBeDisabled();

  await page
    .getByRole("button", { name: "Close Local lab session tab" })
    .last()
    .click();
  await expect(page.getByRole("tab")).toHaveCount(7);
  await expect(newTab).toBeEnabled();
  await newTab.click();
  await expect(page.getByRole("tab")).toHaveCount(8);
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
  await expect(
    page.locator(".known-host-card").filter({ hasText: "changed.example:22" }),
  ).toBeVisible();
});

test("updates mounted Terminal appearance and imports metadata-only resources", async ({
  page,
}) => {
  await page.goto("/");
  await page.locator(".terminal-mount").evaluate((element) => {
    (
      window as Window & { __anysshTerminalMount?: Element }
    ).__anysshTerminalMount = element;
  });

  await page.getByRole("button", { name: "Appearance Aa" }).click();
  await page.getByRole("button", { name: "Import Theme" }).click();
  await expect(
    page.getByText("Browser QA Midnight", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Import Font" }).click();
  await expect(
    page.getByText("Browser QA Mono", { exact: true }),
  ).toBeVisible();
  await expect(page.locator('input[type="file"]')).toHaveCount(0);

  await selectUiOption(page, "App theme", "light");
  await selectUiOption(page, "Terminal theme", "theme-browser-1");
  await selectUiOption(page, "Terminal font", "imported:font-browser-1");
  await page
    .getByRole("textbox", { name: "Terminal font size", exact: true })
    .fill("16");
  await selectUiOption(page, "Terminal line height", "1600");
  await setUiSwitch(page, "Programming ligatures", true);
  await selectUiOption(page, "East Asian ambiguous width", "wide");
  await page.getByRole("button", { name: "Apply appearance" }).click();

  await expect(page.locator("html")).toHaveAttribute("data-app-theme", "light");
  await page.getByRole("button", { name: /^Terminal \d+$/ }).click();
  const terminalStayedMounted = await page
    .locator(".terminal-mount")
    .evaluate((element) => {
      return (
        (window as Window & { __anysshTerminalMount?: Element })
          .__anysshTerminalMount === element
      );
    });
  expect(terminalStayedMounted).toBe(true);
  await expect(page.locator(".terminal-surface")).toHaveCSS(
    "--terminal-background",
    "#101426",
  );

  await page.getByRole("button", { name: "Appearance Aa" }).click();
  const importedFont = page
    .locator(".appearance-asset-list > div")
    .filter({ hasText: "Browser QA Mono" });
  await importedFont.getByRole("button", { name: "Delete" }).click();
  await expect(
    page.getByRole("combobox", { name: "Terminal font" }),
  ).toContainText("AnySSH Nerd Mono");
  await expect(page.getByRole("combobox", { name: "App theme" })).toContainText(
    "Light",
  );
});

test("creates and runs variable-aware Snippets with multi-line confirmation", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: /^Snippets \d+$/ }).click();
  await page.getByRole("button", { name: "New Snippet" }).click();
  const editor = page.getByRole("dialog", { name: "New Snippet" });
  await editor.getByLabel("Label").fill("QA multi-line");
  await editor
    .getByLabel("Snippet command template")
    .fill("echo {{target}}\nprintf qa-finished");
  await editor.getByRole("button", { name: "Save Snippet" }).click();
  const snippet = page
    .locator(".snippet-card")
    .filter({ hasText: "QA multi-line" });
  await expect(snippet).toContainText("2 lines");
  await expect(snippet).toContainText("{{target}}");
  await expect(page.getByText("printf qa-finished")).toHaveCount(0);
  await expect(snippet.getByRole("button", { name: "Run" })).toBeDisabled();

  await page.getByRole("button", { name: /^Terminal \d+$/ }).click();
  await page.getByLabel("Password", { exact: true }).fill("fixture");
  await page.getByRole("button", { name: "Connect" }).click();
  await page
    .getByRole("dialog", { name: "Verify server identity" })
    .getByRole("button", { name: "Trust and continue" })
    .click();
  await expect(page.getByText("Interactive shell is active.")).toBeVisible();

  await page.getByRole("button", { name: /^Snippets \d+$/ }).click();
  await snippet.getByRole("button", { name: "Run" }).click();
  const runner = page.getByRole("dialog", { name: "QA multi-line" });
  await runner.getByLabel("target").fill("qa-marker");
  await expect(runner.getByLabel("Rendered Snippet preview")).toHaveValue(
    "echo qa-marker\nprintf qa-finished",
  );
  await expect(
    runner.getByRole("button", { name: "Run in Session" }),
  ).toBeDisabled();
  await runner
    .getByRole("checkbox", {
      name: "I reviewed every line and want to send this multi-line command.",
    })
    .click();
  await runner.getByRole("button", { name: "Run in Session" }).click();

  await page.getByRole("button", { name: /^Terminal \d+$/ }).click();
  await expect(page.locator(".xterm-rows")).toContainText("echo qa-marker");
  await expect(page.locator(".xterm-rows")).toContainText("printf qa-finished");
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

  await page.getByRole("button", { name: "Generate key" }).click();
  const generatedKeyDialog = page.getByRole("dialog", {
    name: "Generate Private Key",
  });
  await generatedKeyDialog
    .getByLabel("Credential label")
    .fill("QA generated key");
  await generatedKeyDialog.getByLabel("Username").fill("qa-generated-user");
  await generatedKeyDialog.getByLabel("Algorithm").selectOption("rsa4096");
  await expect(generatedKeyDialog.getByLabel("Passphrase")).toHaveCount(0);
  await expect(generatedKeyDialog.getByLabel("PIN")).toHaveCount(0);
  await expect(generatedKeyDialog.locator('input[type="file"]')).toHaveCount(0);
  await generatedKeyDialog
    .getByRole("button", { name: "Generate key" })
    .click();
  const generatedKey = page
    .locator(".resource-card")
    .filter({ hasText: "QA generated key" });
  await expect(generatedKey).toContainText("Private Key");
  await expect(generatedKey).toContainText("Secret hidden");
  await generatedKey.getByRole("button", { name: "Public key" }).click();
  const publicKeyDialog = page.getByRole("dialog", { name: "Public Key" });
  await expect(publicKeyDialog).toContainText("ssh-rsa");
  await expect(publicKeyDialog).toContainText("SHA256:");
  await expect(publicKeyDialog.getByLabel("OpenSSH Public Key")).toHaveValue(
    /^ssh-rsa /u,
  );
  await expect(publicKeyDialog.getByLabel("Passphrase")).toHaveCount(0);
  await expect(
    publicKeyDialog.getByLabel("Private Key", { exact: true }),
  ).toHaveCount(0);
  await publicKeyDialog.getByRole("button", { name: "Close" }).click();
  await generatedKey.getByRole("button", { name: "Export encrypted…" }).click();
  await expect(
    page.getByText(
      "Encrypted Private Key export is available in the native AnySSH runtime. Browser QA writes no file.",
    ),
  ).toBeVisible();
  await expect(page.getByLabel("PIN", { exact: true })).toHaveCount(0);
  await expect(page.getByLabel("Passphrase", { exact: true })).toHaveCount(0);
  await expect(page.locator('input[type="file"]')).toHaveCount(0);

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

  await page.getByRole("button", { name: "New interactive" }).click();
  const interactiveDialog = page.getByRole("dialog", {
    name: "New Interactive Credential",
  });
  await interactiveDialog
    .getByLabel("Credential label")
    .fill("QA production OTP");
  await interactiveDialog.getByLabel("Username").fill("qa-otp-user");
  await expect(interactiveDialog.getByLabel("Password")).toHaveCount(0);
  await expect(
    interactiveDialog.getByText("Session-only responses"),
  ).toBeVisible();
  await interactiveDialog
    .getByRole("button", { name: "Save Interactive Credential" })
    .click();
  const interactiveCredential = page
    .locator(".resource-card")
    .filter({ hasText: "QA production OTP" });
  await expect(interactiveCredential).toContainText("Keyboard-interactive");
  await expect(interactiveCredential).toContainText(
    "Responses are session-only",
  );
  await interactiveCredential
    .getByRole("button", { name: "Edit metadata" })
    .click();
  const editInteractiveDialog = page.getByRole("dialog", {
    name: "Edit Interactive Credential",
  });
  await editInteractiveDialog
    .getByLabel("Credential label")
    .fill("QA updated OTP");
  await editInteractiveDialog
    .getByRole("button", { name: "Save Interactive Credential" })
    .click();
  await expect(page.getByText("QA updated OTP")).toBeVisible();

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
