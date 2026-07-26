import { invoke, isTauri } from "@tauri-apps/api/core";

export type VaultState = "uninitialized" | "locked" | "unlocked" | "damaged";

export interface VaultStatus {
  state: VaultState;
  vaultId: string | null;
  cipherVersion: string | null;
}

const BROWSER_STATUS: VaultStatus = {
  state: "unlocked",
  vaultId: "browser-qa",
  cipherVersion: null,
};

export async function getVaultStatus(): Promise<VaultStatus> {
  if (!isTauri()) return BROWSER_STATUS;
  return invoke<VaultStatus>("vault_status");
}

export async function createVault(pin: string): Promise<VaultStatus> {
  if (!isTauri()) return BROWSER_STATUS;
  return invoke<VaultStatus>("vault_create", { request: { pin } });
}

export async function unlockVault(pin: string): Promise<VaultStatus> {
  if (!isTauri()) return BROWSER_STATUS;
  return invoke<VaultStatus>("vault_unlock", { request: { pin } });
}

export async function lockVault(): Promise<VaultStatus> {
  if (!isTauri()) return BROWSER_STATUS;
  return invoke<VaultStatus>("vault_lock");
}
