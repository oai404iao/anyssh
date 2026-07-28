import { spawn } from "node:child_process";
import { mkdir, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
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
const encryptedKeyPath = requiredEnvironment(
  "ANYSSH_WINDOWS_ENCRYPTED_KEY_PATH",
);
const keyPassphrase = requiredEnvironment("ANYSSH_WINDOWS_KEY_PASSPHRASE");
const wrongKeyPassphrase = requiredEnvironment(
  "ANYSSH_WINDOWS_WRONG_KEY_PASSPHRASE",
);
const privateKeyMarkerPath = requiredEnvironment(
  "ANYSSH_WINDOWS_PRIVATE_KEY_MARKER_PATH",
);
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
  } else if (stage === "changed") {
    await verifyChangedHostKeyAfterRestart(page);
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

  await importEncryptedPrivateKey(targetPage);
  await connectWithEncryptedPrivateKey(targetPage);

  await targetPage.locator(".primary-nav .nav-item").nth(3).click();
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
  await assert(
    targetPage.getByText("Interactive shell is active."),
  ).toBeVisible();
  await assert(hostKeyDialog).toHaveCount(0);
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
  await verifyKnownHostForgetAndRetrust(targetPage);

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
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA encrypted key" }),
  ).toContainText("Private Key");

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
      .filter({ hasText: "Windows QA encrypted key host" }),
  ).toContainText("Private Key");
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA target" }),
  ).toBeVisible();
  const privateKeyHost = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA encrypted key host" });
  await privateKeyHost.getByRole("button", { name: "Open" }).click();
  await targetPage.getByRole("button", { name: "Connect saved Host" }).click();
  await assert(
    targetPage.getByText("Interactive shell is active."),
  ).toBeVisible();
  await assert(
    targetPage.getByRole("dialog", { name: "Verify server identity" }),
  ).toHaveCount(0);
  await capture(
    targetPage,
    "07a-restart-trusted-connection.png",
    "07a-restart-trusted-connection.txt",
  );
  await targetPage.getByRole("button", { name: "Disconnect" }).click();
  await assert(
    targetPage.getByText("The SSH session has ended."),
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

async function verifyChangedHostKeyAfterRestart(targetPage) {
  await assert(
    targetPage.getByRole("heading", { name: "Unlock AnySSH" }),
  ).toBeVisible();
  await targetPage.getByLabel("PIN", { exact: true }).fill(pin);
  await targetPage.getByRole("button", { name: "Unlock" }).click();
  await assert(
    targetPage.getByRole("button", { name: "Lock Vault" }),
  ).toBeVisible();

  await targetPage.locator(".primary-nav .nav-item").nth(2).click();
  const privateKeyHost = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA encrypted key host" });
  await privateKeyHost.getByRole("button", { name: "Open" }).click();
  await targetPage.getByRole("button", { name: "Connect saved Host" }).click();
  const changedDialog = targetPage.getByRole("alertdialog", {
    name: "Host key changed",
  });
  await assert(changedDialog).toContainText(sshHost);
  await assert(changedDialog).toContainText("Trusted");
  await assert(changedDialog).toContainText("Received");
  await assert(
    changedDialog.getByRole("button", { name: /accept|replace/i }),
  ).toHaveCount(0);
  await capture(
    targetPage,
    "08-changed-host-key.png",
    "08-changed-host-key.txt",
  );
  await changedDialog.getByRole("button", { name: "Close" }).click();
  await assert(
    targetPage.getByRole("button", { name: "Connect saved Host" }),
  ).toBeVisible();
}

async function importEncryptedPrivateKey(targetPage) {
  await targetPage.getByRole("button", { name: "Import private key" }).click();
  const privateKeyDialog = targetPage.getByRole("dialog", {
    name: "Import Private Key",
  });
  await privateKeyDialog
    .getByLabel("Credential label")
    .fill("Windows QA encrypted key");
  await privateKeyDialog.getByLabel("Username").fill(sshUsername);

  const nativeDialogDriver = runNativeDialogDriver();
  await privateKeyDialog
    .getByRole("button", { name: "Choose private key" })
    .click();
  const privateKeyCredential = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA encrypted key" });
  await Promise.all([
    assert(privateKeyCredential).toContainText("Private Key", {
      timeout: 120_000,
    }),
    nativeDialogDriver,
  ]);
  await assert(privateKeyCredential).not.toContainText(keyPassphrase);
  await unlink(encryptedKeyPath);
  await capture(
    targetPage,
    "02a4-private-key-imported.png",
    "02a4-private-key-imported.txt",
  );
}

async function connectWithEncryptedPrivateKey(targetPage) {
  await targetPage.locator(".primary-nav .nav-item").nth(2).click();
  await targetPage.getByRole("button", { name: "New host" }).click();
  const privateKeyHostDialog = targetPage.getByRole("dialog", {
    name: "New Host",
  });
  await privateKeyHostDialog
    .getByLabel("Display name")
    .fill("Windows QA encrypted key host");
  await privateKeyHostDialog.getByLabel("Host", { exact: true }).fill(sshHost);
  await privateKeyHostDialog
    .getByRole("spinbutton", { name: "Port" })
    .fill(sshPort);
  await privateKeyHostDialog
    .getByLabel("Credential behavior")
    .selectOption("set");
  await privateKeyHostDialog.getByLabel("Credential reference").selectOption({
    label: `Windows QA encrypted key · ${sshUsername}`,
  });
  await privateKeyHostDialog.getByRole("button", { name: "Save Host" }).click();
  const privateKeyHost = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA encrypted key host" });
  await assert(privateKeyHost).toContainText("Private Key");
  await privateKeyHost.getByRole("button", { name: "Open" }).click();
  await assert(
    targetPage.getByRole("heading", {
      level: 1,
      name: "Windows QA encrypted key host",
    }),
  ).toBeVisible();
  await targetPage.getByRole("button", { name: "Connect saved Host" }).click();
  const hostKeyDialog = targetPage.getByRole("dialog", {
    name: "Verify server identity",
  });
  await assert(hostKeyDialog).toContainText(sshHost);
  await hostKeyDialog
    .getByRole("button", { name: "Trust and continue" })
    .click();
  await assert(
    targetPage.getByText("Interactive shell is active."),
  ).toBeVisible();
  const terminalInput = targetPage.getByRole("textbox", {
    name: "Terminal input",
  });
  await terminalInput.focus();
  await terminalInput.pressSequentially(
    `echo ANYSSH_WINDOWS_ENCRYPTED_KEY_OK > "${privateKeyMarkerPath}"`,
  );
  await terminalInput.press("Enter");
  await targetPage.waitForTimeout(1500);
  await capture(
    targetPage,
    "02a5-private-key-connected.png",
    "02a5-private-key-connected.txt",
  );
  await targetPage.getByRole("button", { name: "Disconnect" }).click();
  await assert(
    targetPage.getByText("The SSH session has ended."),
  ).toBeVisible();
}

async function verifyKnownHostForgetAndRetrust(targetPage) {
  await targetPage.locator(".primary-nav .nav-item").nth(5).click();
  const knownHost = targetPage
    .locator(".known-host-card")
    .filter({ hasText: `${sshHost}:${sshPort}` });
  await assert(knownHost).toContainText("ssh-ed25519");
  await assert(knownHost).toContainText("SHA256:");
  await assert(knownHost).not.toContainText("publicKey");
  await capture(targetPage, "02c-known-hosts.png", "02c-known-hosts.txt");

  const confirmation = runNativeDialogDriver("KnownHostForget");
  await knownHost.getByRole("button", { name: "Forget trust…" }).click();
  await Promise.all([
    assert(targetPage.getByText("No trusted endpoints yet.")).toBeVisible(),
    confirmation,
  ]);
  await capture(
    targetPage,
    "02c2-known-host-forgotten.png",
    "02c2-known-host-forgotten.txt",
  );

  await targetPage.locator(".primary-nav .nav-item").nth(2).click();
  const agentHost = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA agent host" });
  await agentHost.getByRole("button", { name: "Open" }).click();
  await targetPage.getByRole("button", { name: "Connect saved Host" }).click();
  const hostKeyDialog = targetPage.getByRole("dialog", {
    name: "Verify server identity",
  });
  await assert(hostKeyDialog).toContainText(sshHost);
  await capture(
    targetPage,
    "02c3-tofu-after-forget.png",
    "02c3-tofu-after-forget.txt",
  );
  await hostKeyDialog
    .getByRole("button", { name: "Trust and continue" })
    .click();
  await assert(
    targetPage.getByText("Interactive shell is active."),
  ).toBeVisible();
  await targetPage.getByRole("button", { name: "Disconnect" }).click();
  await assert(
    targetPage.getByText("The SSH session has ended."),
  ).toBeVisible();
}

function runNativeDialogDriver(mode = "PrivateKey") {
  const repositoryRoot = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "..",
    "..",
    "..",
  );
  const driverPath = path.join(
    repositoryRoot,
    "scripts",
    "qa",
    "windows-native-dialog-driver.ps1",
  );
  return new Promise((resolve, reject) => {
    const output = [];
    const child = spawn(
      "powershell.exe",
      [
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        driverPath,
        "-Mode",
        mode,
      ],
      {
        cwd: repositoryRoot,
        env: process.env,
        windowsHide: true,
      },
    );
    child.stdout.on("data", (chunk) => output.push(String(chunk)));
    child.stderr.on("data", (chunk) => output.push(String(chunk)));
    child.on("error", (error) => reject(error));
    child.on("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(
          new Error(
            `native Windows dialog automation failed (${code}): ${redact(
              output.join(""),
            )}`,
          ),
        );
      }
    });
  });
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
    .replaceAll(password, "[REDACTED]")
    .replaceAll(keyPassphrase, "[REDACTED]")
    .replaceAll(wrongKeyPassphrase, "[REDACTED]");
}

function requiredEnvironment(name) {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}
