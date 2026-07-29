import { invoke, isTauri } from "@tauri-apps/api/core";
import { browserCredentialIsReferenced } from "./host-bridge";

export type CredentialKind =
  "password" | "privateKey" | "systemAgent" | "keyboardInteractive";

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

export type PrivateKeyGenerationAlgorithm = "ed25519" | "rsa4096";

export interface PrivateKeyCredentialGeneration {
  label: string;
  username: string;
  algorithm: PrivateKeyGenerationAlgorithm;
}

export interface PrivateKeyPublicSummary {
  credentialId: string;
  algorithm: string;
  fingerprintSha256: string;
  opensshPublicKey: string;
}

export interface PrivateKeyExportSummary {
  fileName: string;
  algorithm: string;
  fingerprintSha256: string;
  encrypted: boolean;
}

export interface SystemAgentIdentitySummary {
  algorithm: string;
  fingerprintSha256: string;
  comment: string;
}

export interface SystemAgentCredentialInput {
  label: string;
  username: string;
  identityFingerprintSha256: string;
}

export interface KeyboardInteractiveCredentialInput {
  label: string;
  username: string;
}

export interface KeyboardInteractiveCredentialUpdate extends KeyboardInteractiveCredentialInput {
  credentialId: string;
}

const BROWSER_SYSTEM_AGENT_IDENTITIES: SystemAgentIdentitySummary[] = [
  {
    algorithm: "ssh-ed25519",
    fingerprintSha256: "SHA256:browser-agent-ed25519",
    comment: "Browser QA workstation key",
  },
  {
    algorithm: "rsa-sha2-512",
    fingerprintSha256: "SHA256:browser-agent-rsa",
    comment: "Browser QA hardware-backed key",
  },
];

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
let browserPrivateKeyPublicSummaries = new Map<string, PrivateKeyPublicSummary>(
  [
    [
      "browser-credential-database",
      browserPrivateKeyPublicSummary(
        "browser-credential-database",
        "Database key",
        "ssh-ed25519",
      ),
    ],
  ],
);

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
    browserPrivateKeyPublicSummaries.set(
      summary.id,
      browserPrivateKeyPublicSummary(summary.id, summary.label, "ssh-ed25519"),
    );
    return { ...summary };
  }
  return invoke<CredentialSummary | null>("credential_import_private_key", {
    request: input,
  });
}

export async function generatePrivateKeyCredential(
  input: PrivateKeyCredentialGeneration,
): Promise<CredentialSummary> {
  if (!isTauri()) {
    const summary = {
      id: `browser-credential-${nextBrowserCredentialId++}`,
      label: input.label,
      username: input.username,
      kind: "privateKey" as const,
    };
    browserCredentials.push(summary);
    browserPrivateKeyPublicSummaries.set(
      summary.id,
      browserPrivateKeyPublicSummary(
        summary.id,
        summary.label,
        input.algorithm === "rsa4096" ? "ssh-rsa" : "ssh-ed25519",
      ),
    );
    return { ...summary };
  }
  return invoke<CredentialSummary>("credential_generate_private_key", {
    request: input,
  });
}

export async function getPrivateKeyPublicSummary(
  credentialId: string,
): Promise<PrivateKeyPublicSummary> {
  if (!isTauri()) {
    const summary = browserPrivateKeyPublicSummaries.get(credentialId);
    if (!summary) {
      throw new Error("Private Key Credential was not found");
    }
    return { ...summary };
  }
  return invoke<PrivateKeyPublicSummary>("credential_get_private_key_public", {
    request: { credentialId },
  });
}

export async function listSystemAgentIdentities(): Promise<
  SystemAgentIdentitySummary[]
> {
  if (!isTauri()) {
    return BROWSER_SYSTEM_AGENT_IDENTITIES.map((identity) => ({
      ...identity,
    }));
  }
  return invoke<SystemAgentIdentitySummary[]>(
    "credential_list_system_agent_identities",
  );
}

export function credentialOperationsUseNativeRuntime(): boolean {
  return isTauri();
}

export async function exportPrivateKeyCredential(
  credentialId: string,
): Promise<PrivateKeyExportSummary | null> {
  if (!isTauri()) return null;
  return invoke<PrivateKeyExportSummary | null>(
    "credential_export_private_key",
    {
      request: { credentialId },
    },
  );
}

export async function createSystemAgentCredential(
  input: SystemAgentCredentialInput,
): Promise<CredentialSummary> {
  if (!isTauri()) {
    if (
      !BROWSER_SYSTEM_AGENT_IDENTITIES.some(
        (identity) =>
          identity.fingerprintSha256 === input.identityFingerprintSha256,
      )
    ) {
      throw new Error("Selected SSH Agent identity is no longer available");
    }
    const summary = {
      id: `browser-credential-${nextBrowserCredentialId++}`,
      label: input.label,
      username: input.username,
      kind: "systemAgent" as const,
    };
    browserCredentials.push(summary);
    return { ...summary };
  }
  return invoke<CredentialSummary>("credential_create_system_agent", {
    request: input,
  });
}

export async function createKeyboardInteractiveCredential(
  input: KeyboardInteractiveCredentialInput,
): Promise<CredentialSummary> {
  if (!isTauri()) {
    const summary = {
      id: `browser-credential-${nextBrowserCredentialId++}`,
      label: input.label,
      username: input.username,
      kind: "keyboardInteractive" as const,
    };
    browserCredentials.push(summary);
    return { ...summary };
  }
  return invoke<CredentialSummary>("credential_create_keyboard_interactive", {
    request: input,
  });
}

export async function updateKeyboardInteractiveCredential(
  input: KeyboardInteractiveCredentialUpdate,
): Promise<CredentialSummary> {
  if (!isTauri()) {
    const index = browserCredentials.findIndex(
      (credential) => credential.id === input.credentialId,
    );
    if (
      index < 0 ||
      browserCredentials[index]?.kind !== "keyboardInteractive"
    ) {
      throw new Error("Keyboard-interactive Credential was not found");
    }
    const summary = {
      id: input.credentialId,
      label: input.label,
      username: input.username,
      kind: "keyboardInteractive" as const,
    };
    browserCredentials[index] = summary;
    return { ...summary };
  }
  return invoke<CredentialSummary>("credential_update_keyboard_interactive", {
    request: input,
  });
}

export async function deleteCredential(credentialId: string): Promise<boolean> {
  if (!isTauri()) {
    if (browserCredentialIsReferenced(credentialId)) {
      throw new Error("Credential is in use by a Host or Group");
    }
    const previousLength = browserCredentials.length;
    browserCredentials = browserCredentials.filter(
      (credential) => credential.id !== credentialId,
    );
    browserPrivateKeyPublicSummaries.delete(credentialId);
    return browserCredentials.length !== previousLength;
  }
  return invoke<boolean>("credential_delete", { credentialId });
}

export function resetBrowserCredentialsForTests(seed = false) {
  browserCredentials = seed
    ? cloneCredentials(BROWSER_CREDENTIAL_FIXTURES)
    : [];
  nextBrowserCredentialId = browserCredentials.length + 1;
  browserPrivateKeyPublicSummaries = new Map();
  if (seed) {
    browserPrivateKeyPublicSummaries.set(
      "browser-credential-database",
      browserPrivateKeyPublicSummary(
        "browser-credential-database",
        "Database key",
        "ssh-ed25519",
      ),
    );
  }
}

function cloneCredentials(
  credentials: CredentialSummary[],
): CredentialSummary[] {
  return credentials.map((credential) => ({ ...credential }));
}

function browserPrivateKeyPublicSummary(
  credentialId: string,
  label: string,
  algorithm: "ssh-ed25519" | "ssh-rsa",
): PrivateKeyPublicSummary {
  const encoded = window
    .btoa(`${credentialId}:${algorithm}`)
    .replace(/=+$/u, "");
  return {
    credentialId,
    algorithm,
    fingerprintSha256: `SHA256:${credentialId}`,
    opensshPublicKey: `${algorithm} ${encoded} ${label}`,
  };
}
