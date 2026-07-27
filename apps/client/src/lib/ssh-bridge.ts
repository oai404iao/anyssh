import { Channel, invoke, isTauri } from "@tauri-apps/api/core";

export const isNativeRuntime = isTauri();

export interface ConnectRequest {
  host: string;
  port: number;
  authentication: SshAuthenticationRequest;
  columns: number;
  rows: number;
  jumpHost?: JumpHostRequest;
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

export type SshClientEvent =
  | { type: "connecting" }
  | HostKeyEvent
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
  connected: boolean;
}

const previewSessions = new Map<string, PreviewSession>();
let nextPreviewId = 1;
let nextHostKeyRequestId = 1;
const encoder = new TextEncoder();

export async function connectSsh(
  request: ConnectRequest,
  callbacks: SessionCallbacks,
): Promise<string> {
  if (!isNativeRuntime) {
    return connectPreview(request, callbacks);
  }

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
    const sessionId = await invoke<string>("ssh_connect", {
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
  const hostKeys = createPreviewHostKeys(request);
  previewSessions.set(sessionId, {
    callbacks,
    commandBuffer: "",
    hostKeys,
    hostKeyIndex: 0,
    connected: false,
  });

  queueMicrotask(() => callbacks.onEvent({ type: "connecting" }));
  window.setTimeout(() => emitPreviewHostKey(sessionId), 180);

  callbacks.onData(
    encoder.encode(
      `\r\n\x1b[33mBrowser QA mode\x1b[0m — ${request.host}:${request.port}\r\n`,
    ),
  );

  return Promise.resolve(sessionId);
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

  session.hostKeyIndex += 1;
  if (session.hostKeyIndex < session.hostKeys.length) {
    window.setTimeout(() => emitPreviewHostKey(sessionId), 120);
    return;
  }

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

function createPreviewHostKeys(request: ConnectRequest): HostKeyEvent[] {
  const hostKeys: HostKeyEvent[] = [];

  if (request.jumpHost) {
    hostKeys.push({
      type: "hostKey",
      requestId: nextHostKeyRequestId++,
      hop: { kind: "jumpHost", index: 1 },
      host: request.jumpHost.host,
      port: request.jumpHost.port,
      algorithm: "ssh-ed25519",
      fingerprintSha256: "SHA256:7Jv2eL8mQ4sA9wR1yT6pK3nH5cD0xBzGfUoNqPiMVaE",
    });
  }

  hostKeys.push({
    type: "hostKey",
    requestId: nextHostKeyRequestId++,
    hop: { kind: "target" },
    host: request.host,
    port: request.port,
    algorithm: "ssh-ed25519",
    fingerprintSha256: "SHA256:4G6Yp8sJ0B7x1uN3zR9Qm2cK5dL8vT6aW0fH3eP7nXs",
  });

  return hostKeys;
}

function emitPreviewHostKey(sessionId: string) {
  const session = previewSessions.get(sessionId);
  const hostKey = session?.hostKeys[session.hostKeyIndex];
  if (session && hostKey) {
    session.callbacks.onEvent(hostKey);
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
