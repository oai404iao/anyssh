import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { chromium, expect } from "@playwright/test";

const assert = expect.configure({ timeout: 30_000 });
const cdpUrl = requiredEnvironment("ANYSSH_WINDOWS_CDP_URL");
const runDirectory = requiredEnvironment("ANYSSH_WINDOWS_RUN_DIR");
const stage = requiredEnvironment("ANYSSH_WINDOWS_STAGE");
const pin = "246810";
const wrongPin = "000000";
const password = "windows-fixture-password";
const sshHost = requiredEnvironment("ANYSSH_WINDOWS_SSH_HOST");
const sshPort = requiredEnvironment("ANYSSH_WINDOWS_SSH_PORT");
const sshUsername = requiredEnvironment("ANYSSH_WINDOWS_SSH_USERNAME");
const agentFingerprint = requiredEnvironment(
  "ANYSSH_WINDOWS_AGENT_FINGERPRINT",
);
const agentMarkerPath = requiredEnvironment("ANYSSH_WINDOWS_AGENT_MARKER_PATH");
const consoleEntries = [];
const browserErrors = [];
let browser;
let page;

await mkdir(runDirectory, { recursive: true });

try {
  browser = await chromium.connectOverCDP(cdpUrl);
  page = await findAnySshPage(browser);
  page.setDefaultTimeout(30_000);
  observePage(page);

  if (stage === "create") {
    await createVaultAndRepository(page);
  } else if (stage === "restart") {
    await unlockRestartedVault(page);
  } else {
    throw new Error(`unsupported Windows native smoke stage: ${stage}`);
  }

  if (browserErrors.length > 0) {
    throw new Error(
      `WebView2 reported ${browserErrors.length} browser error(s)`,
    );
  }
} catch (error) {
  browserErrors.push(
    redact(
      error instanceof Error ? (error.stack ?? error.message) : String(error),
    ),
  );
  if (page) {
    await clearPasswordInputs(page);
    await page
      .screenshot({
        animations: "disabled",
        fullPage: true,
        path: path.join(runDirectory, `failed-${stage}.png`),
      })
      .catch(() => {});
  }
  process.exitCode = 1;
} finally {
  await writeEvidence();
  await browser?.close().catch(() => {});
}

async function createVaultAndRepository(targetPage) {
  await assert(
    targetPage.getByRole("heading", {
      name: "Create your encrypted Vault",
    }),
  ).toBeVisible();
  await capture(targetPage, "01-vault-create.png", "01-vault-create.txt");

  await targetPage.getByLabel("PIN", { exact: true }).fill(pin);
  await targetPage.getByLabel("Confirm PIN").fill(pin);
  await targetPage
    .getByRole("button", { name: "Create encrypted Vault" })
    .click();
  await assert(
    targetPage.getByRole("heading", { level: 1, name: "Local lab" }),
  ).toBeVisible();
  await capture(targetPage, "02-native-ready.png", "02-native-ready.txt");

  await targetPage.locator(".primary-nav .nav-item").nth(3).click();
  await targetPage.getByRole("button", { name: "New password" }).click();
  const credentialDialog = targetPage.getByRole("dialog", {
    name: "New Password Credential",
  });
  await credentialDialog
    .getByLabel("Credential label")
    .fill("Windows QA password");
  await credentialDialog.getByLabel("Username").fill("windows-user");
  await credentialDialog.getByLabel("Password").fill(password);
  await credentialDialog
    .getByRole("button", { name: "Save Credential" })
    .click();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA password" }),
  ).toBeVisible();

  await targetPage.getByRole("button", { name: "New system agent" }).click();
  const agentDialog = targetPage.getByRole("dialog", {
    name: "New System Agent Credential",
  });
  await agentDialog
    .getByLabel("Credential label")
    .fill("Windows QA system agent");
  await agentDialog.getByLabel("Username").fill(sshUsername);
  const identitySelect = agentDialog.getByLabel("SSH Agent identity");
  await assert(identitySelect).toContainText(agentFingerprint);
  await identitySelect.selectOption(agentFingerprint);
  await agentDialog
    .getByRole("button", { name: "Save Agent Credential" })
    .click();
  const agentCredential = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA system agent" });
  await assert(agentCredential).toContainText("System Agent");
  await assert(agentCredential).not.toContainText(agentFingerprint);

  await targetPage.locator(".primary-nav .nav-item").nth(2).click();
  await targetPage.getByRole("button", { name: "New host" }).click();
  const agentHostDialog = targetPage.getByRole("dialog", { name: "New Host" });
  await agentHostDialog
    .getByLabel("Display name")
    .fill("Windows QA agent host");
  await agentHostDialog.getByLabel("Host", { exact: true }).fill(sshHost);
  await agentHostDialog.getByRole("spinbutton", { name: "Port" }).fill(sshPort);
  await agentHostDialog.getByLabel("Credential behavior").selectOption("set");
  await agentHostDialog.getByLabel("Credential reference").selectOption({
    label: `Windows QA system agent · ${sshUsername}`,
  });
  await agentHostDialog.getByRole("button", { name: "Save Host" }).click();
  const agentHost = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA agent host" });
  await assert(agentHost).toContainText("System Agent");
  await agentHost.getByRole("button", { name: "Open" }).click();
  await assert(
    targetPage.getByRole("heading", {
      level: 1,
      name: "Windows QA agent host",
    }),
  ).toBeVisible();
  await targetPage.getByRole("button", { name: "Connect saved Host" }).click();
  const hostKeyDialog = targetPage.getByRole("dialog", {
    name: "Verify server identity",
  });
  await assert(hostKeyDialog).toContainText(sshHost);
  await hostKeyDialog
    .getByRole("button", { name: "Trust for this session" })
    .click();
  await assert(
    targetPage.getByText("Interactive shell is active."),
  ).toBeVisible();
  const terminalInput = targetPage.getByRole("textbox", {
    name: "Terminal input",
  });
  await terminalInput.focus();
  await terminalInput.pressSequentially(
    `echo ANYSSH_WINDOWS_AGENT_OK > "${agentMarkerPath}"`,
  );
  await terminalInput.press("Enter");
  await targetPage.waitForTimeout(1500);
  await capture(
    targetPage,
    "02b-system-agent-connected.png",
    "02b-system-agent-connected.txt",
  );
  await targetPage.getByRole("button", { name: "Disconnect" }).click();
  await assert(
    targetPage.getByText("The SSH session has ended."),
  ).toBeVisible();

  await targetPage.locator(".primary-nav .nav-item").nth(2).click();
  await targetPage.getByRole("button", { name: "New host" }).click();
  const hostDialog = targetPage.getByRole("dialog", { name: "New Host" });
  await hostDialog.getByLabel("Display name").fill("Windows QA jump");
  await hostDialog.getByLabel("Host", { exact: true }).fill("127.0.0.1");
  await hostDialog.getByRole("spinbutton", { name: "Port" }).fill("2222");
  await hostDialog.getByLabel("Credential behavior").selectOption("set");
  await hostDialog
    .getByLabel("Credential reference")
    .selectOption({ label: "Windows QA password · windows-user" });
  await hostDialog.getByRole("button", { name: "Save Host" }).click();
  await assert(
    targetPage.locator(".resource-card").filter({ hasText: "Windows QA jump" }),
  ).toBeVisible();

  await targetPage.locator(".primary-nav .nav-item").nth(4).click();
  await targetPage.getByRole("button", { name: "New route" }).click();
  const routeDialog = targetPage.getByRole("dialog", {
    name: "New Jump Route",
  });
  await routeDialog.getByLabel("Route label").fill("Windows QA route");
  await routeDialog
    .getByLabel("Add Host")
    .selectOption({ label: "Windows QA jump · 127.0.0.1:2222" });
  await routeDialog.getByRole("button", { name: "Add", exact: true }).click();
  await routeDialog.getByRole("button", { name: "Save Jump Route" }).click();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA route" }),
  ).toBeVisible();

  await targetPage.locator(".primary-nav .nav-item").nth(1).click();
  await targetPage.getByRole("button", { name: "New group" }).click();
  const groupDialog = targetPage.getByRole("dialog", { name: "New Group" });
  await groupDialog.getByLabel("Group label").fill("Windows QA group");
  await groupDialog.getByLabel("Credential behavior").selectOption("set");
  await groupDialog
    .getByLabel("Credential reference")
    .selectOption({ label: "Windows QA password · windows-user" });
  await groupDialog.getByLabel("Jump Route behavior").selectOption("set");
  await groupDialog
    .getByLabel("Jump Route reference")
    .selectOption({ label: "Windows QA route" });
  await groupDialog.getByRole("button", { name: "Save Group" }).click();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA group" }),
  ).toBeVisible();

  await targetPage.locator(".primary-nav .nav-item").nth(2).click();
  await targetPage.getByRole("button", { name: "New host" }).click();
  const targetDialog = targetPage.getByRole("dialog", { name: "New Host" });
  await targetDialog.getByLabel("Display name").fill("Windows QA target");
  await targetDialog
    .getByLabel("Host", { exact: true })
    .fill("target.internal");
  await targetDialog.getByRole("spinbutton", { name: "Port" }).fill("22");
  await targetDialog
    .getByLabel("Group")
    .selectOption({ label: "Windows QA group" });
  await targetDialog.getByRole("button", { name: "Save Host" }).click();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA target" }),
  ).toContainText("Inherited");
  await capture(
    targetPage,
    "03-repository-created.png",
    "03-repository-created.txt",
  );

  await targetPage.getByRole("button", { name: "Lock Vault" }).click();
  await assert(
    targetPage.getByRole("heading", { name: "Unlock AnySSH" }),
  ).toBeVisible();
  await targetPage.getByLabel("PIN", { exact: true }).fill(wrongPin);
  await targetPage.getByRole("button", { name: "Unlock" }).click();
  await assert(targetPage.getByRole("alert")).toBeVisible();
  await capture(targetPage, "04-vault-wrong-pin.png", "04-vault-wrong-pin.txt");

  await targetPage.getByLabel("PIN", { exact: true }).fill(pin);
  await targetPage.getByRole("button", { name: "Unlock" }).click();
  await assert(
    targetPage.getByRole("button", { name: "Lock Vault" }),
  ).toBeVisible();
  await targetPage.locator(".primary-nav .nav-item").nth(2).click();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA agent host" }),
  ).toContainText("System Agent");
  await capture(
    targetPage,
    "05-vault-reunlocked.png",
    "05-vault-reunlocked.txt",
  );
}

async function unlockRestartedVault(targetPage) {
  await assert(
    targetPage.getByRole("heading", { name: "Unlock AnySSH" }),
  ).toBeVisible();
  await capture(targetPage, "06-restart-locked.png", "06-restart-locked.txt");
  await targetPage.getByLabel("PIN", { exact: true }).fill(pin);
  await targetPage.getByRole("button", { name: "Unlock" }).click();
  await assert(
    targetPage.getByRole("button", { name: "Lock Vault" }),
  ).toBeVisible();

  await targetPage.locator(".primary-nav .nav-item").nth(3).click();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA password" }),
  ).toBeVisible();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA system agent" }),
  ).toContainText("System Agent");

  await targetPage.locator(".primary-nav .nav-item").nth(1).click();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA group" }),
  ).toBeVisible();

  await targetPage.locator(".primary-nav .nav-item").nth(2).click();
  await assert(
    targetPage.locator(".resource-card").filter({ hasText: "Windows QA jump" }),
  ).toBeVisible();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA agent host" }),
  ).toContainText("System Agent");
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA target" }),
  ).toBeVisible();

  await targetPage.locator(".primary-nav .nav-item").nth(4).click();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA route" }),
  ).toBeVisible();
  await capture(
    targetPage,
    "07-restart-recovered.png",
    "07-restart-recovered.txt",
  );
}

async function findAnySshPage(connectedBrowser) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    for (const context of connectedBrowser.contexts()) {
      for (const candidate of context.pages()) {
        const title = await candidate.title().catch(() => "");
        if (title.includes("AnySSH")) {
          return candidate;
        }
      }
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error("AnySSH WebView2 page was not exposed through CDP");
}

function observePage(targetPage) {
  targetPage.on("console", (message) => {
    const entry = `[${message.type()}] ${redact(message.text())}`;
    consoleEntries.push(entry);
    if (message.type() === "error") {
      browserErrors.push(entry);
    }
  });
  targetPage.on("pageerror", (error) => {
    browserErrors.push(`[pageerror] ${redact(error.message)}`);
  });
}

async function capture(targetPage, screenshotName, snapshotName) {
  await targetPage.screenshot({
    animations: "disabled",
    fullPage: true,
    path: path.join(runDirectory, screenshotName),
  });
  const text = await targetPage.locator("body").innerText();
  await writeFile(path.join(runDirectory, snapshotName), `${redact(text)}\n`);
}

async function clearPasswordInputs(targetPage) {
  for (const input of await targetPage
    .locator('input[type="password"]')
    .all()) {
    await input.fill("").catch(() => {});
  }
}

async function writeEvidence() {
  await writeFile(
    path.join(runDirectory, `console-${stage}.txt`),
    `${consoleEntries.join("\n")}\n`,
  );
  await writeFile(
    path.join(runDirectory, `errors-${stage}.txt`),
    `${browserErrors.join("\n")}\n`,
  );
}

function redact(value) {
  return value
    .replaceAll(pin, "[REDACTED]")
    .replaceAll(wrongPin, "[REDACTED]")
    .replaceAll(password, "[REDACTED]");
}

function requiredEnvironment(name) {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}
