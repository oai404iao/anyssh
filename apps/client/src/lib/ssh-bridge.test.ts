import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  resetBrowserKnownHostsForTests,
  trustBrowserKnownHost,
} from "./known-host-bridge";
import {
  confirmHostKey,
  connectSavedHost,
  connectSsh,
  disconnectSsh,
  sendSshInput,
  type SshClientEvent,
} from "./ssh-bridge";

describe("browser preview SSH bridge", () => {
  beforeEach(() => {
    resetBrowserKnownHostsForTests();
  });

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

  it("reuses durable browser trust without a second prompt", async () => {
    vi.useFakeTimers();
    const request = {
      host: "durable.example",
      port: 22,
      authentication: {
        kind: "temporaryPassword" as const,
        username: "anyssh",
        password: "fixture",
      },
      columns: 80,
      rows: 24,
    };
    const firstEvents: SshClientEvent[] = [];
    const firstSession = await connectSsh(request, {
      onEvent: (event) => firstEvents.push(event),
      onData: () => {},
    });
    await vi.runAllTimersAsync();
    const hostKey = firstEvents.find((event) => event.type === "hostKey");
    if (hostKey?.type !== "hostKey") {
      throw new Error("first connection did not prompt");
    }
    await confirmHostKey(firstSession, hostKey.requestId, true);
    await vi.runAllTimersAsync();
    await disconnectSsh(firstSession);

    const secondEvents: SshClientEvent[] = [];
    const secondSession = await connectSsh(request, {
      onEvent: (event) => secondEvents.push(event),
      onData: () => {},
    });
    await vi.runAllTimersAsync();
    expect(secondEvents.map((event) => event.type)).toEqual([
      "connecting",
      "authenticated",
      "connected",
    ]);
    await disconnectSsh(secondSession);
    vi.useRealTimers();
  });

  it("emits a typed changed-key event without prompting", async () => {
    vi.useFakeTimers();
    trustBrowserKnownHost(
      "changed-runtime.example",
      22,
      "ssh-ed25519",
      "SHA256:old-browser-host-key",
    );
    const events: SshClientEvent[] = [];
    await connectSsh(
      {
        host: "changed-runtime.example",
        port: 22,
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
        onData: () => {},
      },
    );
    await vi.runAllTimersAsync();
    expect(events.some((event) => event.type === "hostKey")).toBe(false);
    const changed = events.find((event) => event.type === "hostKeyChanged");
    expect(changed).toMatchObject({
      type: "hostKeyChanged",
      host: "changed-runtime.example",
      port: 22,
      trustedFingerprintsSha256: ["SHA256:old-browser-host-key"],
    });
    expect(events.at(-1)).toEqual({ type: "closed" });
    vi.useRealTimers();
  });
});
