import { useState } from "react";
import {
  forgetKnownHost,
  type KnownHostSummary,
} from "../../lib/known-host-bridge";
import {
  ManagerEmpty,
  ManagerError,
  ManagerShell,
  type ManagerProps,
} from "../configuration/ManagerPrimitives";

interface KnownHostManagerProps extends ManagerProps {
  knownHosts: KnownHostSummary[];
}

export function KnownHostWorkspace({
  knownHosts,
  loading,
  onChanged,
}: KnownHostManagerProps) {
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function forget(knownHostId: string) {
    setBusyId(knownHostId);
    setError(null);
    try {
      await forgetKnownHost(knownHostId);
      await onChanged();
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusyId(null);
    }
  }

  return (
    <ManagerShell
      action={null}
      description="Trusted endpoint keys are stored in SQLCipher. Forgetting trust requires native confirmation and makes the next connection use TOFU again."
      eyebrow="Server identity"
      title="Known Hosts"
    >
      {error && <ManagerError message={error} />}
      {loading ? (
        <ManagerEmpty>Loading Known Hosts…</ManagerEmpty>
      ) : knownHosts.length === 0 ? (
        <ManagerEmpty>
          No trusted endpoints yet. Trust a Host Key during connection to add
          one.
        </ManagerEmpty>
      ) : (
        <div className="resource-list known-host-list">
          {knownHosts.map((knownHost) => (
            <article
              className="resource-card known-host-card"
              key={knownHost.id}
            >
              <div className="resource-icon known-host-resource-icon">KH</div>
              <div className="resource-main">
                <strong>
                  {knownHost.host}:{knownHost.port}
                </strong>
                <span>
                  {knownHost.keys.length} trusted key
                  {knownHost.keys.length === 1 ? "" : "s"}
                </span>
                <div className="known-host-keys">
                  {knownHost.keys.map((key) => (
                    <div className="known-host-key" key={key.fingerprintSha256}>
                      <span>{key.algorithm}</span>
                      <code>{key.fingerprintSha256}</code>
                    </div>
                  ))}
                </div>
              </div>
              <div className="resource-actions">
                <button
                  className="danger-action"
                  disabled={busyId !== null}
                  onClick={() => void forget(knownHost.id)}
                  type="button"
                >
                  {busyId === knownHost.id
                    ? "Waiting for confirmation…"
                    : "Forget trust…"}
                </button>
              </div>
            </article>
          ))}
        </div>
      )}
    </ManagerShell>
  );
}
