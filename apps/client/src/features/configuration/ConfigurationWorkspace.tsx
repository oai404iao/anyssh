import type { CredentialSummary } from "../../lib/credential-bridge";
import type {
  GroupSummary,
  HostSummary,
  JumpRouteSummary,
} from "../../lib/host-bridge";
import type { KnownHostSummary } from "../../lib/known-host-bridge";
import { CredentialWorkspace } from "../credentials/CredentialWorkspace";
import { GroupWorkspace } from "../groups/GroupWorkspace";
import { HostWorkspace } from "../hosts/HostWorkspace";
import { KnownHostWorkspace } from "../known-hosts/KnownHostWorkspace";
import { RouteWorkspace } from "../routes/RouteWorkspace";

export type ConfigurationSection =
  "groups" | "hosts" | "credentials" | "routes" | "knownHosts";

interface ConfigurationWorkspaceProps {
  section: ConfigurationSection;
  groups: GroupSummary[];
  hosts: HostSummary[];
  credentials: CredentialSummary[];
  routes: JumpRouteSummary[];
  knownHosts: KnownHostSummary[];
  loading: boolean;
  loadError: string | null;
  nativeRuntime: boolean;
  onChanged(): Promise<void>;
  onConnectHost(host: HostSummary): void;
  onOpenHost(host: HostSummary): void;
}

export function ConfigurationWorkspace({
  section,
  groups,
  hosts,
  credentials,
  routes,
  knownHosts,
  loading,
  loadError,
  nativeRuntime,
  onChanged,
  onConnectHost,
  onOpenHost,
}: ConfigurationWorkspaceProps) {
  return (
    <div className="configuration-body">
      {loadError && (
        <div className="manager-error" role="alert">
          {loadError}
        </div>
      )}
      {section === "hosts" && (
        <HostWorkspace
          credentials={credentials}
          groups={groups}
          hosts={hosts}
          loading={loading}
          nativeRuntime={nativeRuntime}
          onChanged={onChanged}
          onConnectHost={onConnectHost}
          onOpenHost={onOpenHost}
          routes={routes}
        />
      )}
      {section === "groups" && (
        <GroupWorkspace
          credentials={credentials}
          groups={groups}
          loading={loading}
          onChanged={onChanged}
          routes={routes}
        />
      )}
      {section === "credentials" && (
        <CredentialWorkspace
          credentials={credentials}
          loading={loading}
          onChanged={onChanged}
        />
      )}
      {section === "routes" && (
        <RouteWorkspace
          hosts={hosts}
          loading={loading}
          onChanged={onChanged}
          routes={routes}
        />
      )}
      {section === "knownHosts" && (
        <KnownHostWorkspace
          knownHosts={knownHosts}
          loading={loading}
          onChanged={onChanged}
        />
      )}
    </div>
  );
}
