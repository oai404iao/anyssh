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

export interface PrivateKeyCredentialImport {
  label: string;
  username: string;
}

const BROWSER_CREDENTIAL_FIXTURES: CredentialSummary[] = [
  {
    id: "browser-credential-local",
    label: "Local lab password",
    username: "anyssh",
    kind: "password",
  },
  {
    id: "browser-credential-edge",
    label: "Edge gateway password",
    username: "ops",
    kind: "password",
  },
  {
    id: "browser-credential-database",
    label: "Database key",
    username: "database",
    kind: "privateKey",
  },
];

let browserCredentials = cloneCredentials(BROWSER_CREDENTIAL_FIXTURES);
let nextBrowserCredentialId = browserCredentials.length + 1;

export async function listCredentials(): Promise<CredentialSummary[]> {
  if (!isTauri()) return cloneCredentials(browserCredentials);
  return invoke<CredentialSummary[]>("credential_list");
}

export async function createPasswordCredential(
  input: PasswordCredentialInput,
): Promise<CredentialSummary> {
  if (!isTauri()) {
    const summary = {
      id: `browser-credential-${nextBrowserCredentialId++}`,
      label: input.label,
      username: input.username,
      kind: "password" as const,
    };
    browserCredentials.push(summary);
    return { ...summary };
  }
  return invoke<CredentialSummary>("credential_create_password", {
    request: input,
  });
}

export async function updatePasswordCredential(
  input: PasswordCredentialUpdate,
): Promise<CredentialSummary> {
  if (!isTauri()) {
    const index = browserCredentials.findIndex(
      (credential) => credential.id === input.credentialId,
    );
    if (index < 0 || browserCredentials[index]?.kind !== "password") {
      throw new Error("Password Credential was not found");
    }
    const summary = {
      id: input.credentialId,
      label: input.label,
      username: input.username,
      kind: "password" as const,
    };
    browserCredentials[index] = summary;
    return { ...summary };
  }
  return invoke<CredentialSummary>("credential_update_password", {
    request: input,
  });
}

export async function importPrivateKeyCredential(
  input: PrivateKeyCredentialImport,
): Promise<CredentialSummary | null> {
  if (!isTauri()) {
    const summary = {
      id: `browser-credential-${nextBrowserCredentialId++}`,
      label: input.label,
      username: input.username,
      kind: "privateKey" as const,
    };
    browserCredentials.push(summary);
    return { ...summary };
  }
  return invoke<CredentialSummary | null>("credential_import_private_key", {
    request: input,
  });
}

export async function deleteCredential(credentialId: string): Promise<boolean> {
  if (!isTauri()) {
    const previousLength = browserCredentials.length;
    browserCredentials = browserCredentials.filter(
      (credential) => credential.id !== credentialId,
    );
    return browserCredentials.length !== previousLength;
  }
  return invoke<boolean>("credential_delete", { credentialId });
}

export function resetBrowserCredentialsForTests(seed = false) {
  browserCredentials = seed
    ? cloneCredentials(BROWSER_CREDENTIAL_FIXTURES)
    : [];
  nextBrowserCredentialId = browserCredentials.length + 1;
}

function cloneCredentials(
  credentials: CredentialSummary[],
): CredentialSummary[] {
  return credentials.map((credential) => ({ ...credential }));
}
