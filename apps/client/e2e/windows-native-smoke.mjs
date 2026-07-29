import { spawn } from "node:child_process";
import { once } from "node:events";
import {
  access,
  appendFile,
  mkdir,
  readFile,
  unlink,
  writeFile,
} from "node:fs/promises";
import { createConnection, createServer } from "node:net";
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
const generatedExportPath = requiredEnvironment(
  "ANYSSH_WINDOWS_GENERATED_EXPORT_PATH",
);
const exportPassphrase = requiredEnvironment(
  "ANYSSH_WINDOWS_EXPORT_PASSPHRASE",
);
const wrongExportPassphrase = requiredEnvironment(
  "ANYSSH_WINDOWS_WRONG_EXPORT_PASSPHRASE",
);
const authorizedKeysPath = requiredEnvironment(
  "ANYSSH_WINDOWS_AUTHORIZED_KEYS_PATH",
);
const generatedKeyMarkerPath = requiredEnvironment(
  "ANYSSH_WINDOWS_GENERATED_KEY_MARKER_PATH",
);
const reimportedKeyMarkerPath = requiredEnvironment(
  "ANYSSH_WINDOWS_REIMPORTED_KEY_MARKER_PATH",
);
const interactiveHost = requiredEnvironment("ANYSSH_WINDOWS_INTERACTIVE_HOST");
const interactivePort = requiredEnvironment("ANYSSH_WINDOWS_INTERACTIVE_PORT");
const interactiveUsername = requiredEnvironment(
  "ANYSSH_WINDOWS_INTERACTIVE_USERNAME",
);
const interactiveResponse = requiredEnvironment(
  "ANYSSH_WINDOWS_INTERACTIVE_RESPONSE",
);
const interactiveMarkerPath = requiredEnvironment(
  "ANYSSH_WINDOWS_INTERACTIVE_MARKER_PATH",
);
const localForwardMarker = requiredEnvironment(
  "ANYSSH_WINDOWS_LOCAL_FORWARD_MARKER",
);
const dynamicForwardMarker = requiredEnvironment(
  "ANYSSH_WINDOWS_DYNAMIC_FORWARD_MARKER",
);
const remoteForwardMarker = requiredEnvironment(
  "ANYSSH_WINDOWS_REMOTE_FORWARD_MARKER",
);
const themeFixturePath = requiredEnvironment("ANYSSH_WINDOWS_THEME_PATH");
const fontFixturePath = requiredEnvironment("ANYSSH_WINDOWS_FONT_PATH");
const snippetMarkerPath = requiredEnvironment(
  "ANYSSH_WINDOWS_SNIPPET_MARKER_PATH",
);
const snippetBodyMarker = requiredEnvironment(
  "ANYSSH_WINDOWS_SNIPPET_BODY_MARKER",
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
  await targetPage.getByRole("button", { name: "New interactive" }).click();
  const interactiveCredentialDialog = targetPage.getByRole("dialog", {
    name: "New Interactive Credential",
  });
  await interactiveCredentialDialog
    .getByLabel("Credential label")
    .fill("Windows QA interactive");
  await interactiveCredentialDialog
    .getByLabel("Username")
    .fill(interactiveUsername);
  await assert(interactiveCredentialDialog.getByLabel("Password")).toHaveCount(
    0,
  );
  await interactiveCredentialDialog
    .getByRole("button", { name: "Save Interactive Credential" })
    .click();
  const interactiveCredential = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA interactive" });
  await assert(interactiveCredential).toContainText("Keyboard-interactive");
  await assert(interactiveCredential).toContainText(
    "Responses are session-only",
  );

  await importEncryptedPrivateKey(targetPage);
  await connectWithEncryptedPrivateKey(targetPage);
  await generateExportAndReimportPrivateKeys(targetPage);

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
  await verifyAppearanceAndSnippets(targetPage);
  const forwardPorts = await verifyPortForwarding(targetPage);
  await targetPage.getByRole("button", { name: "Disconnect" }).click();
  await assert(
    targetPage.getByText("The SSH session has ended."),
  ).toBeVisible();
  await assertPortClosed(forwardPorts.local);
  await assertPortClosed(forwardPorts.remote);
  await verifyKnownHostForgetAndRetrust(targetPage);
  await verifyKeyboardInteractive(targetPage);

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

  const vaultLockAgentHost = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA agent host" });
  await vaultLockAgentHost.getByRole("button", { name: "Open" }).click();
  await targetPage.getByRole("button", { name: "Connect saved Host" }).click();
  await assert(
    targetPage.getByText("Interactive shell is active."),
  ).toBeVisible();
  const vaultLockForwardPort = await startLocalForwardMetadata(
    targetPage,
    Number(sshPort),
  );
  await capture(
    targetPage,
    "03a-vault-lock-forwarding.png",
    "03a-vault-lock-forwarding.txt",
  );
  await targetPage.getByRole("button", { name: "Lock Vault" }).click();
  await assert(
    targetPage.getByRole("heading", { name: "Unlock AnySSH" }),
  ).toBeVisible();
  await assertPortClosed(vaultLockForwardPort);
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
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA interactive" }),
  ).toContainText("Keyboard-interactive");
  for (const label of [
    "Windows QA generated key",
    "Windows QA generated RSA",
    "Windows QA reimported key",
  ]) {
    await assert(
      targetPage.locator(".resource-card").filter({ hasText: label }),
    ).toContainText("Private Key");
  }

  await targetPage.locator(".primary-nav .nav-item").nth(7).click();
  await assert(targetPage.getByLabel("App theme")).toHaveValue("light");
  const restartedTheme = targetPage.getByLabel("Terminal theme");
  const restartedFont = targetPage.getByLabel("Font", { exact: true });
  await assert(restartedTheme.locator("option:checked")).toContainText(
    "Windows Aurora",
  );
  await assert(restartedFont.locator("option:checked")).toContainText(
    "imported",
  );
  const restartedFontId = await restartedFont.inputValue();
  await assert
    .poll(() =>
      targetPage.evaluate(
        (family) =>
          Array.from(globalThis.document.fonts).some(
            (face) => face.family === family && face.status === "loaded",
          ),
        `AnySSH Imported ${restartedFontId}`,
      ),
    )
    .toBe(true);
  await assert(targetPage.locator("body")).not.toContainText(themeFixturePath);
  await assert(targetPage.locator("body")).not.toContainText(fontFixturePath);
  await capture(
    targetPage,
    "07b-restart-appearance.png",
    "07b-restart-appearance.txt",
  );

  await targetPage.locator(".primary-nav .nav-item").nth(6).click();
  const restartedSnippet = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA snippet" });
  await assert(restartedSnippet).toContainText("2 lines");
  await assert(restartedSnippet).toContainText("{{marker}}");
  await assert(restartedSnippet).not.toContainText(snippetBodyMarker);
  await capture(
    targetPage,
    "07c-restart-snippet-summary.png",
    "07c-restart-snippet-summary.txt",
  );

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
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA generated key host" }),
  ).toContainText("Private Key");
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA reimported key host" }),
  ).toContainText("Private Key");
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

async function generateExportAndReimportPrivateKeys(targetPage) {
  await targetPage.locator(".primary-nav .nav-item").nth(3).click();
  await targetPage.getByRole("button", { name: "Generate key" }).click();
  const generatedDialog = targetPage.getByRole("dialog", {
    name: "Generate Private Key",
  });
  await generatedDialog
    .getByLabel("Credential label")
    .fill("Windows QA generated key");
  await generatedDialog.getByLabel("Username").fill(sshUsername);
  await generatedDialog.getByLabel("Algorithm").selectOption("ed25519");
  await assert(generatedDialog.getByLabel("PIN")).toHaveCount(0);
  await assert(generatedDialog.getByLabel("Passphrase")).toHaveCount(0);
  await generatedDialog.getByRole("button", { name: "Generate key" }).click();
  let generatedCredential = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA generated key" });
  await assert(generatedCredential).toContainText("Private Key");

  await generatedCredential.getByRole("button", { name: "Public key" }).click();
  const publicDialog = targetPage.getByRole("dialog", { name: "Public Key" });
  const generatedPublicKey = await publicDialog
    .getByLabel("OpenSSH Public Key")
    .inputValue();
  assert(generatedPublicKey.startsWith("ssh-ed25519 ")).toBe(true);
  await assert(publicDialog).toContainText("SHA256:");
  await appendFile(authorizedKeysPath, `\r\n${generatedPublicKey}\r\n`, "utf8");
  await capture(
    targetPage,
    "02g-generated-public-key.png",
    "02g-generated-public-key.txt",
  );
  await publicDialog.getByRole("button", { name: "Close" }).click();

  await createPrivateKeyHostAndConnect(
    targetPage,
    "Windows QA generated key host",
    "Windows QA generated key",
    generatedKeyMarkerPath,
    "ANYSSH_WINDOWS_GENERATED_KEY_OK",
    "02g2-generated-key-connected.png",
    "02g2-generated-key-connected.txt",
  );

  await targetPage.locator(".primary-nav .nav-item").nth(3).click();
  generatedCredential = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA generated key" });
  const exportDriver = runNativeDialogDriver("KeyExport");
  await generatedCredential
    .getByRole("button", { name: "Export encrypted…" })
    .click();
  await exportDriver;
  await assert(
    targetPage.getByText(/Encrypted ssh-ed25519 Private Key exported to/u),
  ).toBeVisible();
  await assert.poll(() => fileExists(generatedExportPath)).toBe(true);
  await verifyGeneratedExportAcl();
  await capture(
    targetPage,
    "02g3-generated-key-exported.png",
    "02g3-generated-key-exported.txt",
  );

  await targetPage.getByRole("button", { name: "Import private key" }).click();
  const reimportDialog = targetPage.getByRole("dialog", {
    name: "Import Private Key",
  });
  await reimportDialog
    .getByLabel("Credential label")
    .fill("Windows QA reimported key");
  await reimportDialog.getByLabel("Username").fill(sshUsername);
  const reimportDriver = runNativeDialogDriver("GeneratedReimport");
  await reimportDialog
    .getByRole("button", { name: "Choose private key" })
    .click();
  await reimportDriver;
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA reimported key" }),
  ).toContainText("Private Key");
  await unlink(generatedExportPath);

  await createPrivateKeyHostAndConnect(
    targetPage,
    "Windows QA reimported key host",
    "Windows QA reimported key",
    reimportedKeyMarkerPath,
    "ANYSSH_WINDOWS_REIMPORTED_KEY_OK",
    "02g4-reimported-key-connected.png",
    "02g4-reimported-key-connected.txt",
  );

  await targetPage.locator(".primary-nav .nav-item").nth(3).click();
  await targetPage.getByRole("button", { name: "Generate key" }).click();
  const rsaDialog = targetPage.getByRole("dialog", {
    name: "Generate Private Key",
  });
  await rsaDialog
    .getByLabel("Credential label")
    .fill("Windows QA generated RSA");
  await rsaDialog.getByLabel("Username").fill(sshUsername);
  await rsaDialog.getByLabel("Algorithm").selectOption("rsa4096");
  await rsaDialog.getByRole("button", { name: "Generate key" }).click();
  const rsaCredential = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA generated RSA" });
  await assert(rsaCredential).toContainText("Private Key", {
    timeout: 60_000,
  });
  await rsaCredential.getByRole("button", { name: "Public key" }).click();
  const rsaPublicDialog = targetPage.getByRole("dialog", {
    name: "Public Key",
  });
  await assert(rsaPublicDialog).toContainText("ssh-rsa");
  await assert(rsaPublicDialog.getByLabel("OpenSSH Public Key")).toHaveValue(
    /^ssh-rsa /u,
  );
  await rsaPublicDialog.getByRole("button", { name: "Close" }).click();
}

async function createPrivateKeyHostAndConnect(
  targetPage,
  hostLabel,
  credentialLabel,
  markerPath,
  marker,
  screenshotName,
  snapshotName,
) {
  await targetPage.locator(".primary-nav .nav-item").nth(2).click();
  await targetPage.getByRole("button", { name: "New host" }).click();
  const hostDialog = targetPage.getByRole("dialog", { name: "New Host" });
  await hostDialog.getByLabel("Display name").fill(hostLabel);
  await hostDialog.getByLabel("Host", { exact: true }).fill(sshHost);
  await hostDialog.getByRole("spinbutton", { name: "Port" }).fill(sshPort);
  await hostDialog.getByLabel("Credential behavior").selectOption("set");
  await hostDialog.getByLabel("Credential reference").selectOption({
    label: `${credentialLabel} · ${sshUsername}`,
  });
  await hostDialog.getByRole("button", { name: "Save Host" }).click();
  const host = targetPage
    .locator(".resource-card")
    .filter({ hasText: hostLabel });
  await host.getByRole("button", { name: "Open" }).click();
  await targetPage.getByRole("button", { name: "Connect saved Host" }).click();
  await assert(
    targetPage.getByText("Interactive shell is active."),
  ).toBeVisible();
  await assert(
    targetPage.getByRole("dialog", { name: "Verify server identity" }),
  ).toHaveCount(0);
  const terminalInput = targetPage.getByRole("textbox", {
    name: "Terminal input",
  });
  await terminalInput.focus();
  await terminalInput.pressSequentially(`echo ${marker} > "${markerPath}"`);
  await terminalInput.press("Enter");
  await assert.poll(() => fileExists(markerPath)).toBe(true);
  await capture(targetPage, screenshotName, snapshotName);
  await targetPage.getByRole("button", { name: "Disconnect" }).click();
  await assert(
    targetPage.getByText("The SSH session has ended."),
  ).toBeVisible();
}

async function verifyAppearanceAndSnippets(targetPage) {
  const terminalMount = targetPage
    .locator(".terminal-tab-panel:not([hidden]) .terminal-mount")
    .first();
  const mountIdentity = `windows-mounted-${Date.now()}`;
  await terminalMount.evaluate((element, identity) => {
    element.setAttribute("data-windows-mount-identity", identity);
  }, mountIdentity);

  await targetPage.locator(".primary-nav .nav-item").nth(7).click();
  const appearanceDriver = runNativeDialogDriver("AppearanceImport");
  await targetPage.getByRole("button", { name: "Import Theme" }).click();
  const terminalTheme = targetPage.getByLabel("Terminal theme");
  await assert(terminalTheme).toContainText("Windows Aurora");
  await targetPage.getByRole("button", { name: "Import Font" }).click();
  await appearanceDriver;

  const fontSelect = targetPage.getByLabel("Font", { exact: true });
  await assert(fontSelect).toContainText("imported");
  const themeOption = terminalTheme
    .locator("option")
    .filter({ hasText: "Windows Aurora" });
  const fontOption = fontSelect
    .locator("option")
    .filter({ hasText: "imported" })
    .first();
  const themeId = await themeOption.getAttribute("value");
  const fontId = await fontOption.getAttribute("value");
  if (!themeId || !fontId) {
    throw new Error("The imported Windows Appearance resources had no IDs.");
  }

  await targetPage.getByLabel("App theme").selectOption("light");
  await terminalTheme.selectOption(themeId);
  await fontSelect.selectOption(fontId);
  await targetPage.getByLabel("Font size").fill("15");
  await targetPage.getByLabel("Line height").selectOption("1600");
  await targetPage.getByLabel("Programming ligatures").check();
  await targetPage
    .getByLabel("East Asian ambiguous width")
    .selectOption("wide");
  await targetPage.getByRole("button", { name: "Apply appearance" }).click();
  await assert(targetPage.locator("html")).toHaveAttribute(
    "data-app-theme",
    "light",
  );
  await assert(targetPage.locator('input[type="file"]')).toHaveCount(0);
  await assert(targetPage.locator("body")).not.toContainText(themeFixturePath);
  await assert(targetPage.locator("body")).not.toContainText(fontFixturePath);

  const importedFontFamily = `AnySSH Imported ${fontId}`;
  await assert
    .poll(() =>
      targetPage.evaluate(
        (family) =>
          Array.from(globalThis.document.fonts).some(
            (face) => face.family === family && face.status === "loaded",
          ),
        importedFontFamily,
      ),
    )
    .toBe(true);
  await capture(
    targetPage,
    "02b5-appearance-imported.png",
    "02b5-appearance-imported.txt",
  );

  await targetPage.locator(".primary-nav .nav-item").nth(0).click();
  await assert(
    targetPage.locator(`[data-windows-mount-identity="${mountIdentity}"]`),
  ).toHaveCount(1);
  await capture(
    targetPage,
    "02b6-terminal-imported-font.png",
    "02b6-terminal-imported-font.txt",
  );

  await targetPage.locator(".primary-nav .nav-item").nth(6).click();
  await targetPage.getByRole("button", { name: "New Snippet" }).click();
  const editor = targetPage.getByRole("dialog", { name: "New Snippet" });
  await editor.getByLabel("Label").fill("Windows QA snippet");
  await editor
    .getByLabel("Snippet command template")
    .fill(
      `echo ${snippetBodyMarker}_ONE > "{{marker}}"\n` +
        `echo ${snippetBodyMarker}_TWO >> "{{marker}}"`,
    );
  await editor.getByRole("button", { name: "Save Snippet" }).click();
  const snippetCard = targetPage
    .locator(".resource-card")
    .filter({ hasText: "Windows QA snippet" });
  await assert(snippetCard).toContainText("2 lines");
  await assert(snippetCard).toContainText("{{marker}}");
  await assert(snippetCard).not.toContainText(snippetBodyMarker);
  await snippetCard.getByRole("button", { name: "Run" }).click();

  const runner = targetPage.getByRole("dialog", {
    name: "Windows QA snippet",
  });
  await runner.getByLabel("marker").fill(snippetMarkerPath);
  await assert(runner.getByLabel("Rendered Snippet preview")).toHaveValue(
    new RegExp(snippetBodyMarker, "u"),
  );
  await capture(
    targetPage,
    "02b7-snippet-confirmation.png",
    "02b7-snippet-confirmation.txt",
  );
  await runner
    .getByLabel(
      "I reviewed every line and want to send this multi-line command.",
    )
    .check();
  await runner.getByRole("button", { name: "Run in Session" }).click();
  await assert
    .poll(async () => {
      const first = await fileContains(
        snippetMarkerPath,
        `${snippetBodyMarker}_ONE`,
      );
      const second = await fileContains(
        snippetMarkerPath,
        `${snippetBodyMarker}_TWO`,
      );
      return first && second;
    })
    .toBe(true);

  await targetPage.locator(".primary-nav .nav-item").nth(0).click();
  await assert(
    targetPage.locator(`[data-windows-mount-identity="${mountIdentity}"]`),
  ).toHaveCount(1);
  await capture(
    targetPage,
    "02b8-snippet-terminal-output.png",
    "02b8-snippet-terminal-output.txt",
  );
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
}

async function verifyPortForwarding(targetPage) {
  const echoServer = await startEchoServer();
  const forwarding = targetPage.getByRole("region", {
    name: "Port forwarding",
  });
  try {
    await forwarding
      .getByRole("combobox", { name: "Port forward type" })
      .selectOption("local");
    await forwarding
      .getByRole("spinbutton", { name: "Forward bind number" })
      .fill("0");
    await forwarding
      .getByRole("textbox", { name: "Port forward destination host" })
      .fill("127.0.0.1");
    await forwarding
      .getByRole("spinbutton", { name: "Forward destination number" })
      .fill(String(echoServer.port));
    await forwarding.getByRole("button", { name: "Start forward" }).click();
    const stopLocal = forwarding.getByRole("button", {
      name: /^Stop local forward on port \d+$/,
    });
    await assert(stopLocal).toBeVisible();
    const localPort = await portFromStopButton(stopLocal);

    await forwarding
      .getByRole("combobox", { name: "Port forward type" })
      .selectOption("dynamic");
    await forwarding
      .getByRole("spinbutton", { name: "Forward bind number" })
      .fill("0");
    await forwarding.getByRole("button", { name: "Start forward" }).click();
    const stopDynamic = forwarding.getByRole("button", {
      name: /^Stop dynamic forward on port \d+$/,
    });
    await assert(stopDynamic).toBeVisible();
    const dynamicPort = await portFromStopButton(stopDynamic);

    await forwarding
      .getByRole("combobox", { name: "Port forward type" })
      .selectOption("remote");
    await forwarding
      .getByRole("spinbutton", { name: "Forward bind number" })
      .fill("0");
    await forwarding
      .getByRole("textbox", { name: "Port forward destination host" })
      .fill("127.0.0.1");
    await forwarding
      .getByRole("spinbutton", { name: "Forward destination number" })
      .fill(String(echoServer.port));
    await forwarding.getByRole("button", { name: "Start forward" }).click();
    const stopRemote = forwarding.getByRole("button", {
      name: /^Stop remote forward on port \d+$/,
    });
    await assert(stopRemote).toBeVisible();
    const remotePort = await portFromStopButton(stopRemote);

    await assert(forwarding).toContainText("3/16");
    await tcpRoundTrip(localPort, `${localForwardMarker}\n`);
    await socks5RoundTrip(
      dynamicPort,
      echoServer.port,
      `${dynamicForwardMarker}\n`,
    );
    await tcpRoundTrip(remotePort, `${remoteForwardMarker}\n`);
    await stopRemote.scrollIntoViewIfNeeded();
    await capture(
      targetPage,
      "02b2-port-forwarding.png",
      "02b2-port-forwarding.txt",
    );
    for (const marker of [
      localForwardMarker,
      dynamicForwardMarker,
      remoteForwardMarker,
    ]) {
      await assert(targetPage.getByText(marker, { exact: true })).toHaveCount(
        0,
      );
    }

    await stopDynamic.click();
    await assert(stopDynamic).toHaveCount(0);
    await assertPortClosed(dynamicPort);
    return { local: localPort, remote: remotePort };
  } finally {
    await echoServer.close();
  }
}

async function startLocalForwardMetadata(targetPage, destinationPort) {
  const forwarding = targetPage.getByRole("region", {
    name: "Port forwarding",
  });
  await forwarding
    .getByRole("combobox", { name: "Port forward type" })
    .selectOption("local");
  await forwarding
    .getByRole("spinbutton", { name: "Forward bind number" })
    .fill("0");
  await forwarding
    .getByRole("textbox", { name: "Port forward destination host" })
    .fill("127.0.0.1");
  await forwarding
    .getByRole("spinbutton", { name: "Forward destination number" })
    .fill(String(destinationPort));
  await forwarding.getByRole("button", { name: "Start forward" }).click();
  const stop = forwarding.getByRole("button", {
    name: /^Stop local forward on port \d+$/,
  });
  await assert(stop).toBeVisible();
  return portFromStopButton(stop);
}

async function verifyKeyboardInteractive(targetPage) {
  await targetPage.getByRole("button", { name: "New session tab" }).click();
  const connectionForm = targetPage.locator(".connection-panel form");
  await connectionForm
    .getByLabel("Display name")
    .fill("Windows QA interactive tab");
  await connectionForm
    .getByLabel("Host", { exact: true })
    .fill(interactiveHost);
  await connectionForm
    .getByRole("spinbutton", { name: "Port" })
    .fill(interactivePort);
  await connectionForm.getByLabel("Username").fill(interactiveUsername);
  await connectionForm
    .getByLabel("Authentication")
    .selectOption("keyboardInteractive");
  await assert(connectionForm.locator("#connection-password")).toHaveCount(0);
  await connectionForm.getByRole("button", { name: "Connect" }).click();

  const hostKeyDialog = targetPage.getByRole("dialog", {
    name: "Verify server identity",
  });
  await assert(hostKeyDialog).toContainText(
    `${interactiveHost}:${interactivePort}`,
  );
  await hostKeyDialog
    .getByRole("button", { name: "Trust and continue" })
    .click();

  const challengeDialog = targetPage.getByRole("dialog", {
    name: "AnySSH controlled challenge",
  });
  await assert(challengeDialog).toContainText("Verification response:");
  await assert(challengeDialog.locator('input[type="password"]')).toHaveCount(
    1,
  );
  await capture(
    targetPage,
    "02d-interactive-challenge.png",
    "02d-interactive-challenge.txt",
  );
  await challengeDialog
    .getByLabel("Verification response:")
    .fill(interactiveResponse);
  await challengeDialog.getByRole("button", { name: "Continue" }).click();
  await assert(
    targetPage.getByText("Interactive shell is active."),
  ).toBeVisible();

  const terminalInput = targetPage.getByRole("textbox", {
    name: "Terminal input",
  });
  await terminalInput.focus();
  await terminalInput.pressSequentially("windows-interactive-command");
  await terminalInput.press("Enter");
  await assert.poll(() => fileExists(interactiveMarkerPath)).toBe(true);
  const interactiveForwardPort = await startLocalForwardMetadata(
    targetPage,
    Number(sshPort),
  );
  await capture(
    targetPage,
    "02e-interactive-connected.png",
    "02e-interactive-connected.txt",
  );

  await targetPage
    .getByRole("button", { name: "Close Windows QA interactive tab" })
    .click();
  await assertPortClosed(interactiveForwardPort);
  await assert(
    targetPage.getByRole("heading", {
      level: 1,
      name: "Windows QA agent host",
    }),
  ).toBeVisible();
  await assert(
    targetPage.getByText("Interactive shell is active."),
  ).toBeVisible();

  const survivingTerminalInput = targetPage.getByRole("textbox", {
    name: "Terminal input",
  });
  await survivingTerminalInput.focus();
  await survivingTerminalInput.pressSequentially(
    `echo ANYSSH_WINDOWS_AGENT_TAB_SURVIVED >> "${agentMarkerPath}"`,
  );
  await survivingTerminalInput.press("Enter");
  await assert
    .poll(() =>
      fileContains(agentMarkerPath, "ANYSSH_WINDOWS_AGENT_TAB_SURVIVED"),
    )
    .toBe(true);
  await capture(
    targetPage,
    "02f-agent-tab-after-close.png",
    "02f-agent-tab-after-close.txt",
  );
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

async function verifyGeneratedExportAcl() {
  const output = await runPowerShell(`
$acl = Get-Acl -LiteralPath $env:ANYSSH_WINDOWS_GENERATED_EXPORT_PATH
$currentSid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$ownerSid = $acl.GetOwner(
  [System.Security.Principal.SecurityIdentifier]
).Value
if ($ownerSid -ne $currentSid) {
  throw "The generated export is not owned by the current Windows user."
}
if (-not $acl.AreAccessRulesProtected) {
  throw "The generated export DACL still inherits parent permissions."
}
$rules = @($acl.Access)
if ($rules.Count -ne 1) {
  throw "The generated export DACL contains unexpected access rules."
}
$ruleSid = $rules[0].IdentityReference.Translate(
  [System.Security.Principal.SecurityIdentifier]
).Value
if ($ruleSid -ne $currentSid) {
  throw "The generated export DACL grants access to a non-user principal."
}
if ($rules[0].AccessControlType -ne
  [System.Security.AccessControl.AccessControlType]::Allow) {
  throw "The generated export DACL rule is not an allow rule."
}
$fullControl = [System.Security.AccessControl.FileSystemRights]::FullControl
if (($rules[0].FileSystemRights -band $fullControl) -ne $fullControl) {
  throw "The generated export does not have the protected owner-only DACL."
}
"owner=current-user"
"dacl=protected-owner-only"
`);
  await writeFile(
    path.join(runDirectory, "generated-export-acl.txt"),
    `${output.trim()}\n`,
  );
}

function runPowerShell(command) {
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
        "-Command",
        command,
      ],
      {
        env: {
          ANYSSH_WINDOWS_GENERATED_EXPORT_PATH: generatedExportPath,
          PATH: process.env.PATH,
          PATHEXT: process.env.PATHEXT,
          SystemRoot: process.env.SystemRoot,
          TEMP: process.env.TEMP,
          TMP: process.env.TMP,
          WINDIR: process.env.WINDIR,
        },
        windowsHide: true,
      },
    );
    child.stdout.on("data", (chunk) => output.push(String(chunk)));
    child.stderr.on("data", (chunk) => output.push(String(chunk)));
    child.on("error", (error) => reject(error));
    child.on("exit", (code) => {
      if (code === 0) {
        resolve(output.join(""));
      } else {
        reject(
          new Error(
            `Windows ACL validation failed (${code}): ${redact(
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

async function startEchoServer() {
  const sockets = new Set();
  const server = createServer({ allowHalfOpen: true }, (socket) => {
    sockets.add(socket);
    socket.on("close", () => sockets.delete(socket));
    socket.setTimeout(10_000, () =>
      socket.destroy(new Error("Windows Forward echo socket timed out")),
    );
    socket.on("data", (chunk) => socket.write(chunk));
    socket.on("end", () => socket.end());
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("Windows Forward echo server did not expose a TCP port");
  }
  return {
    port: address.port,
    close: () =>
      new Promise((resolve, reject) => {
        for (const socket of sockets) {
          socket.destroy();
        }
        server.close((error) => (error ? reject(error) : resolve()));
      }),
  };
}

async function portFromStopButton(button) {
  const label = await button.getAttribute("aria-label");
  const match = label?.match(/port (\d+)$/);
  if (!match) {
    throw new Error("Active Forward did not expose its assigned port");
  }
  return Number(match[1]);
}

async function connectSocket(port) {
  const socket = createConnection({ host: "127.0.0.1", port });
  socket.setTimeout(10_000, () =>
    socket.destroy(new Error("Windows Forward socket timed out")),
  );
  await once(socket, "connect");
  return socket;
}

async function tcpRoundTrip(port, value) {
  const socket = await connectSocket(port);
  const payload = Buffer.from(value);
  const received = [];
  socket.on("data", (chunk) => received.push(chunk));
  socket.end(payload);
  await once(socket, "close");
  assert(Buffer.concat(received)).toEqual(payload);
}

async function socks5RoundTrip(proxyPort, destinationPort, value) {
  const socket = await connectSocket(proxyPort);
  socket.write(Buffer.from([5, 1, 0]));
  assert(await readExactly(socket, 2)).toEqual(Buffer.from([5, 0]));
  socket.write(
    Buffer.from([
      5,
      1,
      0,
      1,
      127,
      0,
      0,
      1,
      (destinationPort >> 8) & 0xff,
      destinationPort & 0xff,
    ]),
  );
  const reply = await readExactly(socket, 10);
  assert(reply.subarray(0, 2)).toEqual(Buffer.from([5, 0]));

  const payload = Buffer.from(value);
  const received = [];
  socket.on("data", (chunk) => received.push(chunk));
  socket.end(payload);
  await once(socket, "close");
  assert(Buffer.concat(received)).toEqual(payload);
}

async function readExactly(socket, length) {
  const chunks = [];
  let remaining = length;
  while (remaining > 0) {
    const chunk = socket.read(remaining);
    if (chunk) {
      chunks.push(chunk);
      remaining -= chunk.length;
      continue;
    }
    if (socket.readableEnded) {
      throw new Error(
        "Forward socket closed before the protocol reply completed",
      );
    }
    await Promise.race([
      once(socket, "readable"),
      once(socket, "end").then(() => {
        throw new Error(
          "Forward socket ended before the protocol reply completed",
        );
      }),
    ]);
  }
  return Buffer.concat(chunks);
}

async function assertPortClosed(port) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      const socket = await connectSocket(port);
      socket.destroy();
      await new Promise((resolve) => setTimeout(resolve, 100));
    } catch {
      return;
    }
  }
  throw new Error(`Forward listener on 127.0.0.1:${port} remained open`);
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
    .replaceAll(wrongKeyPassphrase, "[REDACTED]")
    .replaceAll(exportPassphrase, "[REDACTED]")
    .replaceAll(wrongExportPassphrase, "[REDACTED]")
    .replaceAll(encryptedKeyPath, "[REDACTED]")
    .replaceAll(generatedExportPath, "[REDACTED]")
    .replaceAll(agentFingerprint, "[REDACTED]")
    .replaceAll(interactiveResponse, "[REDACTED]")
    .replaceAll(localForwardMarker, "[REDACTED]")
    .replaceAll(dynamicForwardMarker, "[REDACTED]")
    .replaceAll(remoteForwardMarker, "[REDACTED]");
}

async function fileExists(filePath) {
  try {
    await access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function fileContains(filePath, marker) {
  try {
    return (await readFile(filePath, "utf8")).includes(marker);
  } catch {
    return false;
  }
}

function requiredEnvironment(name) {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}
