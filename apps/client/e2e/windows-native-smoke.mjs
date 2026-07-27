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

  await targetPage.getByRole("button", { name: /^Credentials \d+$/ }).click();
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

  await targetPage.getByRole("button", { name: /^Hosts \d+$/ }).click();
  await targetPage.getByRole("button", { name: "New host" }).click();
  const hostDialog = targetPage.getByRole("dialog", { name: "New Host" });
  await hostDialog.getByLabel("Display name").fill("Windows QA host");
  await hostDialog.getByLabel("Host", { exact: true }).fill("127.0.0.1");
  await hostDialog.getByRole("spinbutton", { name: "Port" }).fill("2222");
  await hostDialog
    .getByLabel("Credential")
    .selectOption({ label: "Windows QA password · windows-user" });
  await hostDialog.getByRole("button", { name: "Save Host" }).click();
  await assert(
    targetPage.locator(".resource-card").filter({ hasText: "Windows QA host" }),
  ).toBeVisible();

  await targetPage.getByRole("button", { name: /^Jump routes \d+$/ }).click();
  await targetPage.getByRole("button", { name: "New route" }).click();
  const routeDialog = targetPage.getByRole("dialog", {
    name: "New Jump Route",
  });
  await routeDialog.getByLabel("Route label").fill("Windows QA route");
  await routeDialog
    .getByLabel("Add Host")
    .selectOption({ label: "Windows QA host · 127.0.0.1:2222" });
  await routeDialog.getByRole("button", { name: "Add", exact: true }).click();
  await routeDialog.getByRole("button", { name: "Save Jump Route" }).click();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA route" }),
  ).toBeVisible();
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
    targetPage.getByRole("heading", { level: 1, name: "Local lab" }),
  ).toBeVisible();
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
    targetPage.getByRole("heading", { level: 1, name: "Local lab" }),
  ).toBeVisible();

  await targetPage.getByRole("button", { name: /^Credentials \d+$/ }).click();
  await assert(
    targetPage
      .locator(".resource-card")
      .filter({ hasText: "Windows QA password" }),
  ).toBeVisible();

  await targetPage.getByRole("button", { name: /^Hosts \d+$/ }).click();
  await assert(
    targetPage.locator(".resource-card").filter({ hasText: "Windows QA host" }),
  ).toBeVisible();

  await targetPage.getByRole("button", { name: /^Jump routes \d+$/ }).click();
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
