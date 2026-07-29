import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import {
  browserKnownHostForEndpoint,
  trustBrowserKnownHost,
} from "./known-host-bridge";

export const isNativeRuntime = isTauri();

export interface ConnectRequest {
  host: string;
  port: number;
  authentication: SshAuthenticationRequest;
  columns: number;
  rows: number;
  jumpHost?: JumpHostRequest;
}

export interface ConnectSavedHostRequest {
  hostId: string;
  columns: number;
  rows: number;
}

export type SshPortForwardKind = "local" | "remote" | "dynamic";

export interface StartSshPortForwardRequest {
  kind: SshPortForwardKind;
  bindHost: string;
  bindPort: number;
  destinationHost?: string;
  destinationPort?: number;
}

export interface SshPortForwardSummary {
  id: string;
  kind: SshPortForwardKind;
  bindHost: string;
  boundPort: number;
  destinationHost?: string;
  destinationPort?: number;
}

export interface JumpHostRequest {
  host: string;
  port: number;
  authentication: SshAuthenticationRequest;
}

export type SshAuthenticationRequest =
  | {
      kind: "temporaryPassword";
      username: string;
      password: string;
    }
  | {
      kind: "keyboardInteractive";
      username: string;
    }
  | {
      kind: "credential";
      credentialId: string;
    };

export type SshSessionHop =
  { kind: "jumpHost"; index: number } | { kind: "target" };

export interface HostKeyEvent {
  type: "hostKey";
  requestId: number;
  hop: SshSessionHop;
  host: string;
  port: number;
  algorithm: string;
  fingerprintSha256: string;
}

export interface HostKeyChangedEvent {
  type: "hostKeyChanged";
  hop: SshSessionHop;
  host: string;
  port: number;
  algorithm: string;
  receivedFingerprintSha256: string;
  trustedFingerprintsSha256: string[];
}

export interface AuthenticationPrompt {
  text: string;
  echo: boolean;
}

export interface AuthenticationChallengeEvent {
  type: "authenticationChallenge";
  requestId: number;
  hop: SshSessionHop;
  host: string;
  port: number;
  name: string;
  instructions: string;
  prompts: AuthenticationPrompt[];
}

export type SshClientEvent =
  | { type: "connecting" }
  | HostKeyEvent
  | HostKeyChangedEvent
  | AuthenticationChallengeEvent
  | { type: "authenticated" }
  | { type: "connected" }
  | { type: "exitStatus"; code: number }
  | { type: "error"; message: string }
  | { type: "closed" };

interface SessionCallbacks {
  onEvent(event: SshClientEvent): void;
  onData(data: Uint8Array): void | Promise<void>;
}

interface PreviewSession {
  callbacks: SessionCallbacks;
  commandBuffer: string;
  hostKeys: HostKeyEvent[];
  hostKeyIndex: number;
  changedHostKey: HostKeyChangedEvent | null;
  authenticationChallenge: AuthenticationChallengeEvent | null;
  connected: boolean;
  portForwards: Map<string, SshPortForwardSummary>;
}

const previewSessions = new Map<string, PreviewSession>();
let nextPreviewId = 1;
let nextPreviewForwardId = 1;
let nextHostKeyRequestId = 1;
let nextAuthenticationRequestId = 1;
const encoder = new TextEncoder();

export async function connectSsh(
  request: ConnectRequest,
  callbacks: SessionCallbacks,
): Promise<string> {
  if (!isNativeRuntime) {
    return connectPreview(request, callbacks);
  }

  return connectNative("ssh_connect", request, callbacks);
}

export async function connectSavedHost(
  request: ConnectSavedHostRequest,
  callbacks: SessionCallbacks,
): Promise<string> {
  if (!isNativeRuntime) {
    throw new Error("Saved Host connections require the native runtime");
  }

  return connectNative("ssh_connect_saved_host", request, callbacks);
}

async function connectNative(
  command: "ssh_connect" | "ssh_connect_saved_host",
  request: ConnectRequest | ConnectSavedHostRequest,
  callbacks: SessionCallbacks,
): Promise<string> {
  const events = new Channel<SshClientEvent>();
  events.onmessage = callbacks.onEvent;

  let resolveSessionId!: (sessionId: string | null) => void;
  const sessionIdReady = new Promise<string | null>((resolve) => {
    resolveSessionId = resolve;
  });

  const data = new Channel<ArrayBuffer>();
  data.onmessage = (message) => {
    const bytes = normalizeBinaryMessage(message);
    void Promise.resolve()
      .then(() => callbacks.onData(bytes))
      .catch((error) => {
        console.error("Terminal output consumer failed", error);
      })
      .then(async () => {
        try {
          const sessionId = await sessionIdReady;
          if (sessionId) {
            await acknowledgeSshOutput(sessionId);
          }
        } catch {
          // The session can close while an xterm write callback is still queued.
        }
      });
  };

  try {
    const sessionId = await invoke<string>(command, {
      request,
      events,
      data,
    });
    resolveSessionId(sessionId);
    return sessionId;
  } catch (error) {
    resolveSessionId(null);
    throw error;
  }
}

export async function confirmHostKey(
  sessionId: string,
  requestId: number,
  accepted: boolean,
): Promise<void> {
  if (!isNativeRuntime) {
    confirmPreviewHostKey(sessionId, requestId, accepted);
    return;
  }

  await invoke("ssh_confirm_host_key", { sessionId, requestId, accepted });
}

export async function respondAuthentication(
  sessionId: string,
  requestId: number,
  responses: string[] | null,
): Promise<void> {
  if (!isNativeRuntime) {
    respondPreviewAuthentication(sessionId, requestId, responses);
    return;
  }

  await invoke("ssh_respond_authentication", {
    request: { sessionId, requestId, responses },
  });
}

export async function sendSshInput(
  sessionId: string,
  input: string,
): Promise<void> {
  if (!isNativeRuntime) {
    sendPreviewInput(sessionId, input);
    return;
  }

  await invoke("ssh_send", { sessionId, input });
}

async function acknowledgeSshOutput(sessionId: string): Promise<void> {
  await invoke("ssh_ack_output", { sessionId });
}

export async function resizeSsh(
  sessionId: string,
  columns: number,
  rows: number,
): Promise<void> {
  if (!isNativeRuntime) return;
  await invoke("ssh_resize", { sessionId, columns, rows });
}

export async function startSshPortForward(
  sessionId: string,
  request: StartSshPortForwardRequest,
): Promise<SshPortForwardSummary> {
  if (!isNativeRuntime) {
    return startPreviewPortForward(sessionId, request);
  }

  return invoke<SshPortForwardSummary>("ssh_forward_start", {
    request: { sessionId, ...request },
  });
}

export async function stopSshPortForward(
  sessionId: string,
  forwardId: string,
): Promise<void> {
  if (!isNativeRuntime) {
    const session = previewSessions.get(sessionId);
    if (!session) {
      throw new Error("SSH session is already closed.");
    }
    session.portForwards.delete(forwardId);
    return;
  }

  await invoke("ssh_forward_stop", {
    request: { sessionId, forwardId },
  });
}

export async function disconnectSsh(sessionId: string): Promise<void> {
  if (!isNativeRuntime) {
    const session = previewSessions.get(sessionId);
    if (session) {
      session.callbacks.onData(
        encoder.encode("\r\n\x1b[33mPreview session closed.\x1b[0m\r\n"),
      );
      session.callbacks.onEvent({ type: "closed" });
      previewSessions.delete(sessionId);
    }
    return;
  }

  await invoke("ssh_disconnect", { sessionId });
}

function connectPreview(
  request: ConnectRequest,
  callbacks: SessionCallbacks,
): Promise<string> {
  const sessionId = `preview-${nextPreviewId++}`;
  const { hostKeys, changedHostKey } = createPreviewHostKeys(request);
  const authenticationChallenge = createPreviewAuthenticationChallenge(request);
  previewSessions.set(sessionId, {
    callbacks,
    commandBuffer: "",
    hostKeys,
    hostKeyIndex: 0,
    changedHostKey,
    authenticationChallenge,
    connected: false,
    portForwards: new Map(),
  });

  queueMicrotask(() => callbacks.onEvent({ type: "connecting" }));
  window.setTimeout(() => advancePreviewHandshake(sessionId), 180);

  callbacks.onData(
    encoder.encode(
      `\r\n\x1b[33mBrowser QA mode\x1b[0m — ${request.host}:${request.port}\r\n`,
    ),
  );

  return Promise.resolve(sessionId);
}

function startPreviewPortForward(
  sessionId: string,
  request: StartSshPortForwardRequest,
): SshPortForwardSummary {
  const session = previewSessions.get(sessionId);
  if (!session?.connected) {
    throw new Error("SSH session is already closed.");
  }
  if (!["local", "remote", "dynamic"].includes(request.kind)) {
    throw new Error("Port forward request is invalid.");
  }
  const bindHost = normalizePreviewLoopback(request.bindHost);
  if (
    !Number.isInteger(request.bindPort) ||
    request.bindPort < 0 ||
    request.bindPort > 65_535
  ) {
    throw new Error("Port forward request is invalid.");
  }
  const dynamic = request.kind === "dynamic";
  const destinationHost = request.destinationHost?.trim();
  const destinationPort = request.destinationPort;
  if (
    dynamic
      ? destinationHost !== undefined || destinationPort !== undefined
      : !destinationHost ||
        destinationHost.length > 255 ||
        /\s/u.test(destinationHost) ||
        Array.from(destinationHost).some((character) => {
          const codePoint = character.codePointAt(0) ?? 0;
          return codePoint <= 0x1f || codePoint === 0x7f;
        }) ||
        !Number.isInteger(destinationPort) ||
        (destinationPort ?? 0) < 1 ||
        (destinationPort ?? 0) > 65_535
  ) {
    throw new Error("Port forward destination is invalid.");
  }
  if (session.portForwards.size >= 16) {
    throw new Error("Maximum active port forwards reached.");
  }

  const sequence = nextPreviewForwardId++;
  const summary: SshPortForwardSummary = {
    id: `preview-forward-${sequence}`,
    kind: request.kind,
    bindHost,
    boundPort:
      request.bindPort === 0 ? 40_000 + (sequence % 20_000) : request.bindPort,
    ...(dynamic
      ? {}
      : {
          destinationHost,
          destinationPort,
        }),
  };
  session.portForwards.set(summary.id, summary);
  return summary;
}

function normalizePreviewLoopback(value: string): string {
  const normalized = value.trim();
  if (normalized === "127.0.0.1" || normalized === "::1") {
    return normalized;
  }
  throw new Error("Port forward bind address must be loopback.");
}

function respondPreviewAuthentication(
  sessionId: string,
  requestId: number,
  responses: string[] | null,
) {
  const session = previewSessions.get(sessionId);
  const challenge = session?.authenticationChallenge;
  if (!session || challenge?.requestId !== requestId) return;

  session.authenticationChallenge = null;
  if (responses === null) {
    session.callbacks.onEvent({ type: "closed" });
    previewSessions.delete(sessionId);
    return;
  }
  if (responses.length !== challenge.prompts.length) {
    session.callbacks.onEvent({
      type: "error",
      message: "Authentication response count does not match.",
    });
    session.callbacks.onEvent({ type: "closed" });
    previewSessions.delete(sessionId);
    return;
  }

  window.setTimeout(() => finishPreviewAuthentication(sessionId), 120);
}

function confirmPreviewHostKey(
  sessionId: string,
  requestId: number,
  accepted: boolean,
) {
  const session = previewSessions.get(sessionId);
  const activeHostKey = session?.hostKeys[session.hostKeyIndex];
  if (!session || activeHostKey?.requestId !== requestId) return;

  if (!accepted) {
    session.callbacks.onData(
      encoder.encode("\r\n\x1b[31mHost key rejected.\x1b[0m\r\n"),
    );
    session.callbacks.onEvent({ type: "closed" });
    previewSessions.delete(sessionId);
    return;
  }

  try {
    trustBrowserKnownHost(
      activeHostKey.host,
      activeHostKey.port,
      activeHostKey.algorithm,
      activeHostKey.fingerprintSha256,
    );
  } catch (error) {
    session.callbacks.onEvent({ type: "error", message: String(error) });
    session.callbacks.onEvent({ type: "closed" });
    previewSessions.delete(sessionId);
    return;
  }

  session.hostKeyIndex += 1;
  if (session.hostKeyIndex < session.hostKeys.length) {
    window.setTimeout(() => advancePreviewHandshake(sessionId), 120);
    return;
  }

  window.setTimeout(() => advancePreviewHandshake(sessionId), 120);
}

function finishPreviewAuthentication(sessionId: string) {
  const session = previewSessions.get(sessionId);
  if (!session) return;
  session.callbacks.onEvent({ type: "authenticated" });
  window.setTimeout(() => {
    const activeSession = previewSessions.get(sessionId);
    if (!activeSession) return;

    activeSession.connected = true;
    activeSession.callbacks.onEvent({ type: "connected" });
    activeSession.callbacks.onData(
      encoder.encode(
        "\r\nWelcome to the AnySSH browser QA shell.\r\n" +
          "Type \x1b[36mhelp\x1b[0m to see available preview commands.\r\n\r\n" +
          "\x1b[32manyssh@preview\x1b[0m:\x1b[34m~\x1b[0m$ ",
      ),
    );
  }, 160);
}

function createPreviewHostKeys(request: ConnectRequest): {
  hostKeys: HostKeyEvent[];
  changedHostKey: HostKeyChangedEvent | null;
} {
  const observedHostKeys: HostKeyEvent[] = [];
  const hostKeys: HostKeyEvent[] = [];

  if (request.jumpHost) {
    observedHostKeys.push({
      type: "hostKey",
      requestId: nextHostKeyRequestId++,
      hop: { kind: "jumpHost", index: 1 },
      host: request.jumpHost.host,
      port: request.jumpHost.port,
      algorithm: "ssh-ed25519",
      fingerprintSha256: "SHA256:7Jv2eL8mQ4sA9wR1yT6pK3nH5cD0xBzGfUoNqPiMVaE",
    });
  }

  observedHostKeys.push({
    type: "hostKey",
    requestId: nextHostKeyRequestId++,
    hop: { kind: "target" },
    host: request.host,
    port: request.port,
    algorithm: "ssh-ed25519",
    fingerprintSha256: "SHA256:4G6Yp8sJ0B7x1uN3zR9Qm2cK5dL8vT6aW0fH3eP7nXs",
  });

  for (const observed of observedHostKeys) {
    let knownHost = browserKnownHostForEndpoint(observed.host, observed.port);
    if (
      !knownHost &&
      observed.host.trim().replace(/\.$/, "").toLowerCase() ===
        "changed.example"
    ) {
      trustBrowserKnownHost(
        observed.host,
        observed.port,
        observed.algorithm,
        "SHA256:trusted-browser-changed-key",
      );
      knownHost = browserKnownHostForEndpoint(observed.host, observed.port);
    }
    if (!knownHost) {
      hostKeys.push(observed);
      continue;
    }
    if (
      knownHost.keys.some(
        (key) => key.fingerprintSha256 === observed.fingerprintSha256,
      )
    ) {
      continue;
    }
    return {
      hostKeys,
      changedHostKey: {
        type: "hostKeyChanged",
        hop: observed.hop,
        host: observed.host,
        port: observed.port,
        algorithm: observed.algorithm,
        receivedFingerprintSha256: observed.fingerprintSha256,
        trustedFingerprintsSha256: knownHost.keys.map(
          (key) => key.fingerprintSha256,
        ),
      },
    };
  }

  return { hostKeys, changedHostKey: null };
}

function createPreviewAuthenticationChallenge(
  request: ConnectRequest,
): AuthenticationChallengeEvent | null {
  const host = request.host.trim().replace(/\.$/, "").toLowerCase();
  if (host !== "otp.example" && host !== "multi-otp.example") {
    return null;
  }

  if (host === "multi-otp.example") {
    return {
      type: "authenticationChallenge",
      requestId: nextAuthenticationRequestId++,
      hop: { kind: "target" },
      host: request.host,
      port: request.port,
      name: "Multi-factor authentication",
      instructions: "Enter the verification code and device name.",
      prompts: [
        { text: "Verification code:", echo: false },
        { text: "Device name:", echo: true },
      ],
    };
  }

  return {
    type: "authenticationChallenge",
    requestId: nextAuthenticationRequestId++,
    hop: { kind: "target" },
    host: request.host,
    port: request.port,
    name: "Multi-factor authentication",
    instructions: "Enter the current verification code.",
    prompts: [{ text: "Verification code:", echo: false }],
  };
}

function advancePreviewHandshake(sessionId: string) {
  const session = previewSessions.get(sessionId);
  const hostKey = session?.hostKeys[session.hostKeyIndex];
  if (session && hostKey) {
    session.callbacks.onEvent(hostKey);
    return;
  }
  if (session?.changedHostKey) {
    session.callbacks.onEvent(session.changedHostKey);
    session.callbacks.onEvent({ type: "closed" });
    previewSessions.delete(sessionId);
    return;
  }
  if (session?.authenticationChallenge) {
    session.callbacks.onEvent(session.authenticationChallenge);
    return;
  }
  if (session) {
    finishPreviewAuthentication(sessionId);
  }
}

function sendPreviewInput(sessionId: string, input: string) {
  const session = previewSessions.get(sessionId);
  if (!session?.connected) return;

  for (const character of input) {
    if (character === "\r" || character === "\n") {
      const command = session.commandBuffer.trim();
      session.callbacks.onData(encoder.encode("\r\n"));
      runPreviewCommand(sessionId, command);
      session.commandBuffer = "";
      continue;
    }

    if (character === "\u007f") {
      if (session.commandBuffer.length > 0) {
        session.commandBuffer = session.commandBuffer.slice(0, -1);
        session.callbacks.onData(encoder.encode("\b \b"));
      }
      continue;
    }

    if (character >= " ") {
      session.commandBuffer += character;
      session.callbacks.onData(encoder.encode(character));
    }
  }
}

function runPreviewCommand(sessionId: string, command: string) {
  const session = previewSessions.get(sessionId);
  if (!session) return;

  const write = (value: string) =>
    session.callbacks.onData(encoder.encode(value));

  switch (command) {
    case "":
      break;
    case "help":
      write(
        "Available commands: help, unicode, status, clear, exit\r\n" +
          "This preview validates terminal input and UI flows without opening a network socket.\r\n",
      );
      break;
    case "unicode":
      write("Unicode: 中文 日本語 한글 ⚡  → ✓ 👩‍💻\r\n");
      break;
    case "status":
      write("transport=preview encryption=simulated host_key=confirmed\r\n");
      break;
    case "clear":
      write("\x1b[2J\x1b[H");
      break;
    case "exit":
      write("logout\r\n");
      session.callbacks.onEvent({ type: "exitStatus", code: 0 });
      session.callbacks.onEvent({ type: "closed" });
      previewSessions.delete(sessionId);
      return;
    default:
      write(`anyssh-preview: command not found: ${command}\r\n`);
  }

  write("\x1b[32manyssh@preview\x1b[0m:\x1b[34m~\x1b[0m$ ");
}

function normalizeBinaryMessage(message: ArrayBuffer): Uint8Array {
  return new Uint8Array(message);
}
