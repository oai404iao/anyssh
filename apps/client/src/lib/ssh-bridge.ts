import { Channel, invoke, isTauri } from "@tauri-apps/api/core";

export const isNativeRuntime = isTauri();

export interface ConnectRequest {
  host: string;
  port: number;
  username: string;
  password: string;
  columns: number;
  rows: number;
}

export interface HostKeyEvent {
  type: "hostKey";
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
  onData(data: Uint8Array): void;
}

interface PreviewSession {
  callbacks: SessionCallbacks;
  commandBuffer: string;
  awaitingHostKey: boolean;
  connected: boolean;
}

const previewSessions = new Map<string, PreviewSession>();
let nextPreviewId = 1;
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

  const data = new Channel<ArrayBuffer>();
  data.onmessage = (message) =>
    callbacks.onData(normalizeBinaryMessage(message));

  return invoke<string>("ssh_connect", { request, events, data });
}

export async function confirmHostKey(
  sessionId: string,
  accepted: boolean,
): Promise<void> {
  if (!isNativeRuntime) {
    confirmPreviewHostKey(sessionId, accepted);
    return;
  }

  await invoke("ssh_confirm_host_key", { sessionId, accepted });
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
  previewSessions.set(sessionId, {
    callbacks,
    commandBuffer: "",
    awaitingHostKey: true,
    connected: false,
  });

  queueMicrotask(() => callbacks.onEvent({ type: "connecting" }));
  window.setTimeout(() => {
    callbacks.onEvent({
      type: "hostKey",
      algorithm: "ssh-ed25519",
      fingerprintSha256: "SHA256:4G6Yp8sJ0B7x1uN3zR9Qm2cK5dL8vT6aW0fH3eP7nXs",
    });
  }, 180);

  callbacks.onData(
    encoder.encode(
      `\r\n\x1b[33mBrowser QA mode\x1b[0m — ${request.host}:${request.port}\r\n`,
    ),
  );

  return Promise.resolve(sessionId);
}

function confirmPreviewHostKey(sessionId: string, accepted: boolean) {
  const session = previewSessions.get(sessionId);
  if (!session || !session.awaitingHostKey) return;

  session.awaitingHostKey = false;
  if (!accepted) {
    session.callbacks.onData(
      encoder.encode("\r\n\x1b[31mHost key rejected.\x1b[0m\r\n"),
    );
    session.callbacks.onEvent({ type: "closed" });
    previewSessions.delete(sessionId);
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
