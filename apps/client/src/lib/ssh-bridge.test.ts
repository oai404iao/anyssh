import { describe, expect, it, vi } from "vitest";
import {
  confirmHostKey,
  connectSavedHost,
  connectSsh,
  disconnectSsh,
  sendSshInput,
  type SshClientEvent,
} from "./ssh-bridge";

describe("browser preview SSH bridge", () => {
  it("keeps Saved Host connection resolution native-only", async () => {
    await expect(
      connectSavedHost(
        {
          hostId: "host-target",
          columns: 80,
          rows: 24,
        },
        {
          onEvent: () => {},
          onData: () => {},
        },
      ),
    ).rejects.toThrow("native runtime");
  });

  it("requires host-key confirmation before connecting", async () => {
    vi.useFakeTimers();
    const events: SshClientEvent[] = [];
    const output: string[] = [];
    const decoder = new TextDecoder();

    const sessionId = await connectSsh(
      {
        host: "127.0.0.1",
        port: 2222,
        authentication: {
          kind: "temporaryPassword",
          username: "anyssh",
          password: "fixture",
        },
        columns: 80,
        rows: 24,
      },
      {
        onEvent: (event) => events.push(event),
        onData: (data) => {
          output.push(decoder.decode(data));
        },
      },
    );

    await vi.runAllTimersAsync();
    expect(events.map((event) => event.type)).toEqual([
      "connecting",
      "hostKey",
    ]);

    const hostKeyEvent = events.find((event) => event.type === "hostKey");
    expect(hostKeyEvent?.type).toBe("hostKey");
    if (hostKeyEvent?.type !== "hostKey") {
      throw new Error("host-key event was not emitted");
    }

    await confirmHostKey(sessionId, hostKeyEvent.requestId, true);
    await vi.runAllTimersAsync();
    expect(events.map((event) => event.type)).toEqual([
      "connecting",
      "hostKey",
      "authenticated",
      "connected",
    ]);

    await sendSshInput(sessionId, "unicode\r");
    expect(output.join("")).toContain("中文");

    await disconnectSsh(sessionId);
    expect(events.at(-1)).toEqual({ type: "closed" });
    vi.useRealTimers();
  });

  it("requires separate Jump Host and target confirmations", async () => {
    vi.useFakeTimers();
    const events: SshClientEvent[] = [];

    const sessionId = await connectSsh(
      {
        host: "db.internal",
        port: 22,
        authentication: {
          kind: "temporaryPassword",
          username: "target-user",
          password: "target-password",
        },
        columns: 80,
        rows: 24,
        jumpHost: {
          host: "gateway.example",
          port: 22,
          authentication: {
            kind: "temporaryPassword",
            username: "jump-user",
            password: "jump-password",
          },
        },
      },
      {
        onEvent: (event) => events.push(event),
        onData: () => {},
      },
    );

    await vi.runAllTimersAsync();
    const jumpHostKey = events.find((event) => event.type === "hostKey");
    expect(jumpHostKey?.type).toBe("hostKey");
    if (jumpHostKey?.type !== "hostKey") {
      throw new Error("Jump Host key event was not emitted");
    }
    expect(jumpHostKey.hop).toEqual({ kind: "jumpHost", index: 1 });
    expect(jumpHostKey.host).toBe("gateway.example");

    await confirmHostKey(sessionId, jumpHostKey.requestId, true);
    await vi.runAllTimersAsync();
    const hostKeys = events.filter((event) => event.type === "hostKey");
    expect(hostKeys).toHaveLength(2);
    const targetHostKey = hostKeys[1];
    expect(targetHostKey?.type).toBe("hostKey");
    if (targetHostKey?.type !== "hostKey") {
      throw new Error("target host-key event was not emitted");
    }
    expect(targetHostKey.hop).toEqual({ kind: "target" });
    expect(targetHostKey.host).toBe("db.internal");
    expect(events.some((event) => event.type === "connected")).toBe(false);

    await confirmHostKey(sessionId, targetHostKey.requestId, true);
    await vi.runAllTimersAsync();
    expect(events.map((event) => event.type)).toEqual([
      "connecting",
      "hostKey",
      "hostKey",
      "authenticated",
      "connected",
    ]);

    await disconnectSsh(sessionId);
    vi.useRealTimers();
  });
});
