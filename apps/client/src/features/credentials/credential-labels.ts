import type { CredentialSummary } from "../../lib/credential-bridge";

export function credentialKindLabel(kind: CredentialSummary["kind"]) {
  switch (kind) {
    case "privateKey":
      return "Private Key";
    case "systemAgent":
      return "System Agent";
    case "keyboardInteractive":
      return "Keyboard-interactive";
    case "password":
      return "Password";
  }
}
