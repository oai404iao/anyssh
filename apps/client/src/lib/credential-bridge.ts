import { invoke, isTauri } from "@tauri-apps/api/core";

export type CredentialKind = "password" | "privateKey";

export interface CredentialSummary {
  id: string;
  label: string;
  username: string;
  kind: CredentialKind;
}

export interface PasswordCredentialInput {
  label: string;
  username: string;
  password: string;
}

export interface PasswordCredentialUpdate extends PasswordCredentialInput {
  credentialId: string;
}

let nextBrowserCredentialId = 1;

export async function listCredentials(): Promise<CredentialSummary[]> {
  if (!isTauri()) return [];
  return invoke<CredentialSummary[]>("credential_list");
}

export async function createPasswordCredential(
  input: PasswordCredentialInput,
): Promise<CredentialSummary> {
  if (!isTauri()) {
    return {
      id: `browser-credential-${nextBrowserCredentialId++}`,
      label: input.label,
      username: input.username,
      kind: "password",
    };
  }
  return invoke<CredentialSummary>("credential_create_password", {
    request: input,
  });
}

export async function updatePasswordCredential(
  input: PasswordCredentialUpdate,
): Promise<CredentialSummary> {
  if (!isTauri()) {
    return {
      id: input.credentialId,
      label: input.label,
      username: input.username,
      kind: "password",
    };
  }
  return invoke<CredentialSummary>("credential_update_password", {
    request: input,
  });
}

export async function deleteCredential(credentialId: string): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>("credential_delete", { credentialId });
}
