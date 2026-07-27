import { beforeEach, describe, expect, it } from "vitest";
import {
  createPasswordCredential,
  deleteCredential,
  importPrivateKeyCredential,
  listCredentials,
  resetBrowserCredentialsForTests,
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
});
