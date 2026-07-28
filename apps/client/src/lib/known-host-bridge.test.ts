import { beforeEach, describe, expect, it } from "vitest";
import {
  browserKnownHostForEndpoint,
  forgetKnownHost,
  listKnownHosts,
  resetBrowserKnownHostsForTests,
  trustBrowserKnownHost,
} from "./known-host-bridge";

describe("browser preview Known Host bridge", () => {
  beforeEach(() => {
    resetBrowserKnownHostsForTests();
  });

  it("normalizes endpoints and keeps summaries metadata-only", async () => {
    trustBrowserKnownHost(
      " EXAMPLE.COM. ",
      2222,
      "ssh-ed25519",
      "SHA256:browser-known-host",
    );
    const knownHost = browserKnownHostForEndpoint("example.com", 2222);
    expect(knownHost).toMatchObject({
      host: "example.com",
      port: 2222,
      keys: [
        {
          algorithm: "ssh-ed25519",
          fingerprintSha256: "SHA256:browser-known-host",
        },
      ],
    });
    expect(JSON.stringify(await listKnownHosts())).not.toContain("publicKey");
  });

  it("fails closed on a different key and forgets by opaque ID", async () => {
    trustBrowserKnownHost("host.example", 22, "ssh-ed25519", "SHA256:first");
    expect(() =>
      trustBrowserKnownHost("host.example", 22, "ssh-ed25519", "SHA256:second"),
    ).toThrow("different key");

    const knownHost = browserKnownHostForEndpoint("host.example", 22);
    expect(knownHost).not.toBeNull();
    await expect(forgetKnownHost(knownHost!.id)).resolves.toBe(true);
    expect(browserKnownHostForEndpoint("host.example", 22)).toBeNull();
  });
});
