import { describe, expect, it } from "vitest";
import {
  createVault,
  getVaultStatus,
  lockVault,
  unlockVault,
} from "./vault-bridge";

describe("browser preview vault bridge", () => {
  it("keeps browser QA mode unlocked without persisting a PIN", async () => {
    await expect(getVaultStatus()).resolves.toMatchObject({
      state: "unlocked",
      vaultId: "browser-qa",
    });
    await expect(createVault("fixture-pin")).resolves.toMatchObject({
      state: "unlocked",
    });
    await expect(unlockVault("fixture-pin")).resolves.toMatchObject({
      state: "unlocked",
    });
    await expect(lockVault()).resolves.toMatchObject({
      state: "unlocked",
    });
  });
});
