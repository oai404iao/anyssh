import { describe, expect, it } from "vitest";
import {
  createPasswordCredential,
  deleteCredential,
  listCredentials,
  updatePasswordCredential,
} from "./credential-bridge";

describe("browser preview credential bridge", () => {
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
    await expect(listCredentials()).resolves.toEqual([]);
    await expect(deleteCredential(created.id)).resolves.toBe(false);
  });
});
