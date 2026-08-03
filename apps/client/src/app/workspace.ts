import type { ConfigurationSection } from "../features/configuration/ConfigurationWorkspace";

export type WorkspaceView =
  "terminal" | "appearance" | "snippets" | ConfigurationSection;

export function isConfigurationSection(
  view: WorkspaceView,
): view is ConfigurationSection {
  return ["groups", "hosts", "credentials", "routes", "knownHosts"].includes(
    view,
  );
}
