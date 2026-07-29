import { beforeEach, describe, expect, it } from "vitest";
import {
  createKeyboardInteractiveCredential,
  createPasswordCredential,
  createSystemAgentCredential,
  credentialOperationsUseNativeRuntime,
  deleteCredential,
  exportPrivateKeyCredential,
  generatePrivateKeyCredential,
  getPrivateKeyPublicSummary,
  importPrivateKeyCredential,
  listCredentials,
  listSystemAgentIdentities,
  resetBrowserCredentialsForTests,
  updateKeyboardInteractiveCredential,
  updatePasswordCredential,
} from "./credential-bridge";
import { resetBrowserHostsAndRoutesForTests } from "./host-bridge";

describe("browser preview credential bridge", () => {
  beforeEach(() => {
    resetBrowserCredentialsForTests();
    resetBrowserHostsAndRoutesForTests();
  });

  it("returns metadata without retaining or echoing the password", async () => {
    const created = await createPasswordCredential({
      label: "Fixture password",
      username: "fixture-user",
      password: "password-must-not-return",
    });
    const updated = await updatePasswordCredential({
      credentialId: created.id,
      label: "Updated fixture password",
      username: "updated-user",
      password: "updated-password-must-not-return",
    });

    expect(created).toMatchObject({
      label: "Fixture password",
      username: "fixture-user",
      kind: "password",
    });
    expect(updated).toMatchObject({
      id: created.id,
      label: "Updated fixture password",
      username: "updated-user",
      kind: "password",
    });
    expect(JSON.stringify([created, updated])).not.toContain(
      "password-must-not-return",
    );
    await expect(listCredentials()).resolves.toEqual([updated]);
    await expect(deleteCredential(created.id)).resolves.toBe(true);
    await expect(listCredentials()).resolves.toEqual([]);
  });

  it("simulates a metadata-only native Private Key import", async () => {
    const imported = await importPrivateKeyCredential({
      label: "Imported key",
      username: "key-user",
    });

    expect(imported).toMatchObject({
      label: "Imported key",
      username: "key-user",
      kind: "privateKey",
    });
    expect(JSON.stringify(imported)).not.toContain("privateKeyMaterial");
    expect(JSON.stringify(imported)).not.toContain("passphrase");
  });

  it("generates metadata and reveals only the Public Key projection", async () => {
    const generated = await generatePrivateKeyCredential({
      label: "Generated RSA",
      username: "key-user",
      algorithm: "rsa4096",
    });
    expect(generated).toMatchObject({
      label: "Generated RSA",
      username: "key-user",
      kind: "privateKey",
    });

    const publicKey = await getPrivateKeyPublicSummary(generated.id);
    expect(publicKey).toMatchObject({
      credentialId: generated.id,
      algorithm: "ssh-rsa",
    });
    expect(publicKey.fingerprintSha256).toMatch(/^SHA256:/u);
    expect(publicKey.opensshPublicKey).toMatch(/^ssh-rsa /u);
    const serialized = JSON.stringify({ generated, publicKey });
    expect(serialized).not.toContain("PRIVATE KEY");
    expect(serialized).not.toContain("passphrase");
    expect(serialized).not.toContain("pin");
    expect(serialized).not.toContain("path");
    expect(credentialOperationsUseNativeRuntime()).toBe(false);
    await expect(exportPrivateKeyCredential(generated.id)).resolves.toBeNull();

    await expect(deleteCredential(generated.id)).resolves.toBe(true);
    await expect(getPrivateKeyPublicSummary(generated.id)).rejects.toThrow(
      "not found",
    );
  });

  it("selects a metadata-only System Agent identity by fingerprint", async () => {
    const identities = await listSystemAgentIdentities();
    expect(identities).toHaveLength(2);
    expect(identities[0]).toMatchObject({
      algorithm: "ssh-ed25519",
      fingerprintSha256: "SHA256:browser-agent-ed25519",
    });
    expect(JSON.stringify(identities)).not.toContain("privateKey");
    expect(JSON.stringify(identities)).not.toContain("signature");
    expect(JSON.stringify(identities)).not.toContain("socketPath");

    const created = await createSystemAgentCredential({
      label: "Workstation agent",
      username: "agent-user",
      identityFingerprintSha256: identities[0]!.fingerprintSha256,
    });
    expect(created).toMatchObject({
      label: "Workstation agent",
      username: "agent-user",
      kind: "systemAgent",
    });
    expect(JSON.stringify(created)).not.toContain(
      identities[0]!.fingerprintSha256,
    );
    await expect(
      createSystemAgentCredential({
        label: "Missing agent",
        username: "agent-user",
        identityFingerprintSha256: "SHA256:missing",
      }),
    ).rejects.toThrow("no longer available");
  });

  it("stores only metadata for Keyboard-interactive Credentials", async () => {
    const created = await createKeyboardInteractiveCredential({
      label: "Production OTP",
      username: "interactive-user",
    });
    const updated = await updateKeyboardInteractiveCredential({
      credentialId: created.id,
      label: "Updated OTP",
      username: "updated-interactive-user",
    });

    expect(created).toMatchObject({
      label: "Production OTP",
      username: "interactive-user",
      kind: "keyboardInteractive",
    });
    expect(updated).toMatchObject({
      id: created.id,
      label: "Updated OTP",
      username: "updated-interactive-user",
      kind: "keyboardInteractive",
    });
    const serialized = JSON.stringify([created, updated]);
    expect(serialized).not.toContain("otpSeed");
    expect(serialized).not.toContain("response");
    expect(serialized).not.toContain("promptRule");
    await expect(listCredentials()).resolves.toEqual([updated]);
  });
});
