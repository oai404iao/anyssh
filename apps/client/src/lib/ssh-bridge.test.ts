import { describe, expect, it, vi } from "vitest";
import {
  confirmHostKey,
  connectSsh,
  disconnectSsh,
  sendSshInput,
  type SshClientEvent,
} from "./ssh-bridge";

describe("browser preview SSH bridge", () => {
  it("requires host-key confirmation before connecting", async () => {
    vi.useFakeTimers();
    const events: SshClientEvent[] = [];
    const output: string[] = [];
    const decoder = new TextDecoder();

    const sessionId = await connectSsh(
      {
        host: "127.0.0.1",
        port: 2222,
        username: "anyssh",
        password: "fixture",
        columns: 80,
        rows: 24,
      },
      {
        onEvent: (event) => events.push(event),
        onData: (data) => output.push(decoder.decode(data)),
      },
    );

    await vi.runAllTimersAsync();
    expect(events.map((event) => event.type)).toEqual([
      "connecting",
      "hostKey",
    ]);

    await confirmHostKey(sessionId, true);
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
});
