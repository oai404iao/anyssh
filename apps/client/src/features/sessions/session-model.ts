import type {
  AuthenticationChallengeEvent,
  HostKeyChangedEvent,
  HostKeyEvent,
  SshPortForwardKind,
  SshPortForwardSummary,
} from "../../lib/ssh-bridge";
import { isNativeRuntime } from "../../lib/ssh-bridge";
import type { HostSummary } from "../../lib/host-bridge";

export type ConnectionStatus =
  | "idle"
  | "connecting"
  | "verifying"
  | "authenticating"
  | "authenticated"
  | "connected"
  | "error"
  | "closed";

export interface ConnectionForm {
  name: string;
  host: string;
  port: string;
  username: string;
  authenticationKind: "password" | "keyboardInteractive";
  password: string;
}

export interface PortForwardForm {
  kind: SshPortForwardKind;
  bindHost: string;
  bindPort: string;
  destinationHost: string;
  destinationPort: string;
}

export interface SessionTab {
  id: string;
  generation: number;
  title: string;
  form: ConnectionForm;
  status: ConnectionStatus;
  statusDetail: string;
  sessionId: string | null;
  pendingHostKey: HostKeyEvent | null;
  changedHostKey: HostKeyChangedEvent | null;
  pendingAuthentication: AuthenticationChallengeEvent | null;
  passwordVisible: boolean;
  error: string | null;
  selectedSavedHostId: string | null;
  terminalSize: { columns: number; rows: number };
  portForwardForm: PortForwardForm;
  portForwards: SshPortForwardSummary[];
  portForwardError: string | null;
  portForwardBusy: boolean;
}

export const INITIAL_CONNECTION_FORM: ConnectionForm = {
  name: "Local lab",
  host: "127.0.0.1",
  port: "2222",
  username: "anyssh",
  authenticationKind: "password",
  password: "",
};

const INITIAL_PORT_FORWARD_FORM: PortForwardForm = {
  kind: "local",
  bindHost: "127.0.0.1",
  bindPort: "0",
  destinationHost: "127.0.0.1",
  destinationPort: "8080",
};

export const MAX_SESSION_TABS = 8;

export const STATUS_LABEL: Record<ConnectionStatus, string> = {
  idle: "Ready",
  connecting: "Connecting",
  verifying: "Verify host",
  authenticating: "Authentication required",
  authenticated: "Authenticated",
  connected: "Connected",
  error: "Connection failed",
  closed: "Disconnected",
};

let nextSessionTabId = 1;

export function formatForwardEndpoint(host: string, port: number): string {
  return `${host.includes(":") ? `[${host}]` : host}:${port}`;
}

export function createSessionTab(
  source:
    | { kind: "quick" }
    | {
        kind: "savedHost";
        host: HostSummary;
      } = { kind: "quick" },
): SessionTab {
  const id = `session-tab-${nextSessionTabId++}`;
  if (source.kind === "savedHost") {
    return {
      id,
      generation: 0,
      title: source.host.displayName,
      form: {
        ...INITIAL_CONNECTION_FORM,
        name: source.host.displayName,
        host: source.host.host,
        port: String(source.host.port),
      },
      status: "idle",
      statusDetail: "Ready to connect the saved Host.",
      sessionId: null,
      pendingHostKey: null,
      changedHostKey: null,
      pendingAuthentication: null,
      passwordVisible: false,
      error: null,
      selectedSavedHostId: source.host.id,
      terminalSize: { columns: 120, rows: 32 },
      portForwardForm: { ...INITIAL_PORT_FORWARD_FORM },
      portForwards: [],
      portForwardError: null,
      portForwardBusy: false,
    };
  }

  return {
    id,
    generation: 0,
    title: INITIAL_CONNECTION_FORM.name,
    form: { ...INITIAL_CONNECTION_FORM },
    status: "idle",
    statusDetail: isNativeRuntime
      ? "Native SSH runtime is available."
      : "Browser QA mode uses a local terminal simulation.",
    sessionId: null,
    pendingHostKey: null,
    changedHostKey: null,
    pendingAuthentication: null,
    passwordVisible: false,
    error: null,
    selectedSavedHostId: null,
    terminalSize: { columns: 120, rows: 32 },
    portForwardForm: { ...INITIAL_PORT_FORWARD_FORM },
    portForwards: [],
    portForwardError: null,
    portForwardBusy: false,
  };
}
