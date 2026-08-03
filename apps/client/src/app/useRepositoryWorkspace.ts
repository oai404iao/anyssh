import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { TerminalAppearance } from "../components/TerminalPane";
import { terminalFontFamily } from "../features/appearance/terminal-font";
import {
  DEFAULT_APPEARANCE_SETTINGS,
  fontAssetUrl,
  getAppearanceSettings,
  importedFontCssFamily,
  listFontAssets,
  listSystemFonts,
  listTerminalThemes,
  terminalThemeById,
  updateAppearanceSettings,
  type AppearanceSettings,
  type FontAssetSummary,
  type SystemFontSummary,
  type TerminalThemeSummary,
} from "../lib/appearance-bridge";
import {
  listCredentials,
  type CredentialSummary,
} from "../lib/credential-bridge";
import {
  listGroups,
  listHosts,
  listJumpRoutes,
  type GroupSummary,
  type HostSummary,
  type JumpRouteSummary,
} from "../lib/host-bridge";
import {
  listKnownHosts,
  type KnownHostSummary,
} from "../lib/known-host-bridge";
import { listSnippets, type SnippetSummary } from "../lib/snippet-bridge";
import type { VaultStatus } from "../lib/vault-bridge";

interface RepositoryWorkspaceOptions {
  nativeRuntime: boolean;
  onHostsChanged(hosts: HostSummary[]): void;
  vaultState: VaultStatus["state"] | null | undefined;
}

export function useRepositoryWorkspace({
  nativeRuntime,
  onHostsChanged,
  vaultState,
}: RepositoryWorkspaceOptions) {
  const [credentials, setCredentials] = useState<CredentialSummary[]>([]);
  const [groups, setGroups] = useState<GroupSummary[]>([]);
  const [hosts, setHosts] = useState<HostSummary[]>([]);
  const [routes, setRoutes] = useState<JumpRouteSummary[]>([]);
  const [knownHosts, setKnownHosts] = useState<KnownHostSummary[]>([]);
  const [appearance, setAppearance] = useState<AppearanceSettings>(() => ({
    ...DEFAULT_APPEARANCE_SETTINGS,
  }));
  const [terminalThemes, setTerminalThemes] = useState<TerminalThemeSummary[]>(
    [],
  );
  const [fontAssets, setFontAssets] = useState<FontAssetSummary[]>([]);
  const [systemFonts, setSystemFonts] = useState<SystemFontSummary[]>([]);
  const [fontFaceRevision, setFontFaceRevision] = useState(0);
  const [snippets, setSnippets] = useState<SnippetSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const refreshIdRef = useRef(0);

  const refresh = useCallback(async () => {
    const refreshId = ++refreshIdRef.current;
    setLoading(true);
    setLoadError(null);
    try {
      const [
        nextCredentials,
        nextGroups,
        nextHosts,
        nextRoutes,
        nextKnownHosts,
        nextAppearance,
        nextTerminalThemes,
        nextFontAssets,
        nextSystemFonts,
        nextSnippets,
      ] = await Promise.all([
        listCredentials(),
        listGroups(),
        listHosts(),
        listJumpRoutes(),
        listKnownHosts(),
        getAppearanceSettings(),
        listTerminalThemes(),
        listFontAssets(),
        listSystemFonts(),
        listSnippets(),
      ]);
      if (refreshId !== refreshIdRef.current) return;

      setCredentials(nextCredentials);
      setGroups(nextGroups);
      setHosts(nextHosts);
      setRoutes(nextRoutes);
      setKnownHosts(nextKnownHosts);
      setAppearance(nextAppearance);
      setTerminalThemes(nextTerminalThemes);
      setFontAssets(nextFontAssets);
      setSystemFonts(nextSystemFonts);
      setSnippets(nextSnippets);
      onHostsChanged(nextHosts);
    } catch (error) {
      if (refreshId === refreshIdRef.current) {
        setLoadError(String(error));
      }
    } finally {
      if (refreshId === refreshIdRef.current) {
        setLoading(false);
      }
    }
  }, [onHostsChanged]);

  useEffect(() => {
    if (nativeRuntime && vaultState !== "unlocked") return;
    const refreshTimer = window.setTimeout(() => {
      void refresh();
    }, 0);
    return () => window.clearTimeout(refreshTimer);
  }, [nativeRuntime, refresh, vaultState]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const applyTheme = () => {
      const resolved =
        appearance.appTheme === "system"
          ? media.matches
            ? "dark"
            : "light"
          : appearance.appTheme;
      document.documentElement.dataset.appTheme = resolved;
    };
    applyTheme();
    if (appearance.appTheme !== "system") return;
    media.addEventListener("change", applyTheme);
    return () => media.removeEventListener("change", applyTheme);
  }, [appearance.appTheme]);

  useEffect(() => {
    if (!nativeRuntime || typeof FontFace === "undefined") return;
    let active = true;
    const loaded: FontFace[] = [];
    void Promise.allSettled(
      fontAssets.map(async (font) => {
        const face = new FontFace(
          importedFontCssFamily(font.id),
          `url("${fontAssetUrl(font)}")`,
        );
        await face.load();
        if (active) {
          document.fonts.add(face);
          loaded.push(face);
        }
      }),
    ).then(() => {
      if (active) setFontFaceRevision((current) => current + 1);
    });
    return () => {
      active = false;
      for (const face of loaded) {
        document.fonts.delete(face);
      }
    };
  }, [fontAssets, nativeRuntime]);

  const applyAppearance = useCallback(async (settings: AppearanceSettings) => {
    const updated = await updateAppearanceSettings(settings);
    setAppearance(updated);
  }, []);

  const clear = useCallback(() => {
    refreshIdRef.current += 1;
    setLoading(false);
    setLoadError(null);
    setCredentials([]);
    setGroups([]);
    setHosts([]);
    setRoutes([]);
    setKnownHosts([]);
    setSnippets([]);
  }, []);

  const terminalAppearance = useMemo<TerminalAppearance>(() => {
    const theme = terminalThemeById(terminalThemes, appearance.terminalThemeId);
    return {
      fontFamily: terminalFontFamily(appearance),
      fontLoadRevision: fontFaceRevision,
      fontSize: appearance.fontSize,
      lineHeight: appearance.lineHeightMillis / 1000,
      ligaturesEnabled: appearance.ligaturesEnabled,
      ambiguousWidth: appearance.ambiguousWidth,
      palette: theme.palette,
    };
  }, [appearance, fontFaceRevision, terminalThemes]);

  return {
    appearance,
    applyAppearance,
    clear,
    credentials,
    fontAssets,
    groups,
    hosts,
    knownHosts,
    loadError,
    loading,
    refresh,
    routes,
    snippets,
    systemFonts,
    terminalAppearance,
    terminalThemes,
  };
}
