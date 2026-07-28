import { invoke, isTauri } from "@tauri-apps/api/core";

export interface KnownHostKeySummary {
  algorithm: string;
  fingerprintSha256: string;
}

export interface KnownHostSummary {
  id: string;
  host: string;
  port: number;
  keys: KnownHostKeySummary[];
}

const BROWSER_KNOWN_HOST_FIXTURES: KnownHostSummary[] = [
  {
    id: "browser-known-host-changed",
    host: "changed.example",
    port: 22,
    keys: [
      {
        algorithm: "ssh-ed25519",
        fingerprintSha256: "SHA256:trusted-browser-changed-key",
      },
    ],
  },
];

let browserKnownHosts: KnownHostSummary[] = [];
let nextBrowserKnownHostId = 1;

export async function listKnownHosts(): Promise<KnownHostSummary[]> {
  if (!isTauri()) return cloneKnownHosts(browserKnownHosts);
  return invoke<KnownHostSummary[]>("known_host_list");
}

export async function forgetKnownHost(knownHostId: string): Promise<boolean> {
  if (!isTauri()) {
    const previousLength = browserKnownHosts.length;
    browserKnownHosts = browserKnownHosts.filter(
      (knownHost) => knownHost.id !== knownHostId,
    );
    return browserKnownHosts.length !== previousLength;
  }
  return invoke<boolean>("known_host_forget", {
    request: { knownHostId },
  });
}

export function browserKnownHostForEndpoint(
  host: string,
  port: number,
): KnownHostSummary | null {
  if (isTauri()) return null;
  const canonicalHost = canonicalBrowserHost(host);
  const knownHost = browserKnownHosts.find(
    (candidate) => candidate.host === canonicalHost && candidate.port === port,
  );
  return knownHost ? cloneKnownHost(knownHost) : null;
}

export function trustBrowserKnownHost(
  host: string,
  port: number,
  algorithm: string,
  fingerprintSha256: string,
): void {
  if (isTauri()) return;
  const canonicalHost = canonicalBrowserHost(host);
  const existing = browserKnownHosts.find(
    (knownHost) => knownHost.host === canonicalHost && knownHost.port === port,
  );
  if (existing) {
    if (
      existing.keys.some(
        (key) =>
          key.algorithm === algorithm &&
          key.fingerprintSha256 === fingerprintSha256,
      )
    ) {
      return;
    }
    throw new Error("Known Host already trusts a different key");
  }
  browserKnownHosts.push({
    id: `browser-known-host-${nextBrowserKnownHostId++}`,
    host: canonicalHost,
    port,
    keys: [{ algorithm, fingerprintSha256 }],
  });
}

export function resetBrowserKnownHostsForTests(seed = false) {
  browserKnownHosts = seed ? cloneKnownHosts(BROWSER_KNOWN_HOST_FIXTURES) : [];
  nextBrowserKnownHostId = browserKnownHosts.length + 1;
}

function canonicalBrowserHost(host: string): string {
  return host.trim().replace(/\.$/, "").toLowerCase();
}

function cloneKnownHosts(knownHosts: KnownHostSummary[]): KnownHostSummary[] {
  return knownHosts.map(cloneKnownHost);
}

function cloneKnownHost(knownHost: KnownHostSummary): KnownHostSummary {
  return {
    ...knownHost,
    keys: knownHost.keys.map((key) => ({ ...key })),
  };
}
