import { FormEvent, useMemo, useState } from "react";
import {
  BUILT_IN_TERMINAL_THEMES,
  deleteFontAsset,
  deleteTerminalTheme,
  importFontAsset,
  importTerminalTheme,
  importedFontCssFamily,
  terminalThemeById,
  type AppearanceSettings,
  type FontAssetSummary,
  type FontSourceKind,
  type SystemFontSummary,
  type TerminalThemeSummary,
} from "../lib/appearance-bridge";

interface AppearanceWorkspaceProps {
  settings: AppearanceSettings;
  themes: TerminalThemeSummary[];
  fonts: FontAssetSummary[];
  systemFonts: SystemFontSummary[];
  loading: boolean;
  loadError: string | null;
  onChanged(): Promise<void>;
  onUpdate(settings: AppearanceSettings): Promise<void>;
}

interface FontOption {
  value: string;
  label: string;
  sourceKind: FontSourceKind;
  fontId: string | null;
  family: string;
}

export function AppearanceWorkspace({
  settings,
  themes,
  fonts,
  systemFonts,
  loading,
  loadError,
  onChanged,
  onUpdate,
}: AppearanceWorkspaceProps) {
  const [draft, setDraft] = useState(settings);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const allThemes = useMemo(
    () => [...BUILT_IN_TERMINAL_THEMES, ...themes],
    [themes],
  );
  const selectedTheme = terminalThemeById(themes, draft.terminalThemeId);
  const fontOptions = useMemo<FontOption[]>(
    () => [
      {
        value: "bundled:anyssh-nerd-mono",
        label: "AnySSH Nerd Mono · bundled",
        sourceKind: "bundled",
        fontId: "builtin:anyssh-nerd-mono",
        family: "AnySSH Nerd Mono",
      },
      {
        value: "system:ui-monospace",
        label: "System UI Monospace",
        sourceKind: "system",
        fontId: null,
        family: "ui-monospace",
      },
      {
        value: "system:monospace",
        label: "System Monospace",
        sourceKind: "system",
        fontId: null,
        family: "monospace",
      },
      ...uniqueSystemFonts(systemFonts).map((font) => ({
        value: `system:${font.family}`,
        label: `${font.family}${font.monospaced ? " · monospace" : ""}`,
        sourceKind: "system" as const,
        fontId: null,
        family: font.family,
      })),
      ...fonts.map((font) => ({
        value: `imported:${font.id}`,
        label: `${font.family} ${font.style} · imported`,
        sourceKind: "imported" as const,
        fontId: font.id,
        family: font.family,
      })),
    ],
    [fonts, systemFonts],
  );
  const selectedFontOption =
    fontOptions.find(
      (option) =>
        option.sourceKind === draft.fontSourceKind &&
        option.fontId === draft.fontId &&
        option.family === draft.fontFamily,
    ) ?? fontOptions[0]!;

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await onUpdate(draft);
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function removeTheme(themeId: string) {
    setBusy(true);
    setError(null);
    try {
      await deleteTerminalTheme(themeId);
      await onChanged();
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function removeFont(fontId: string) {
    setBusy(true);
    setError(null);
    try {
      await deleteFontAsset(fontId);
      await onChanged();
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function importTheme() {
    setBusy(true);
    setError(null);
    try {
      const imported = await importTerminalTheme();
      if (imported) {
        const next = { ...draft, terminalThemeId: imported.id };
        setDraft(next);
        await onUpdate(next);
        await onChanged();
      }
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function importFont() {
    setBusy(true);
    setError(null);
    try {
      const imported = await importFontAsset();
      if (imported) {
        const next: AppearanceSettings = {
          ...draft,
          fontSourceKind: "imported",
          fontId: imported.id,
          fontFamily: imported.family,
        };
        setDraft(next);
        await onUpdate(next);
        await onChanged();
      }
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="configuration-body appearance-workspace">
      <section className="manager-shell">
        <div className="manager-header">
          <div>
            <p className="eyebrow">Personalization</p>
            <h2>Appearance</h2>
            <p>
              App and Terminal themes are versioned data. Fonts never grant
              themes script or remote-resource access.
            </p>
          </div>
          <div className="manager-actions appearance-import-actions">
            <button
              className="secondary-button compact-button"
              disabled={busy}
              onClick={() => void importTheme()}
              type="button"
            >
              Import Theme
            </button>
            <button
              className="connect-button compact-button"
              disabled={busy}
              onClick={() => void importFont()}
              type="button"
            >
              Import Font
            </button>
          </div>
        </div>

        {(loadError || error) && (
          <div className="manager-error" role="alert">
            {error ?? loadError}
          </div>
        )}

        <div className="appearance-layout">
          <form className="appearance-form" onSubmit={submit}>
            <div className="appearance-section">
              <div>
                <p className="eyebrow">Application</p>
                <h3>Interface theme</h3>
              </div>
              <label>
                App theme
                <select
                  aria-label="App theme"
                  disabled={loading || busy}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      appTheme: event.target
                        .value as AppearanceSettings["appTheme"],
                    }))
                  }
                  value={draft.appTheme}
                >
                  <option value="system">Follow system</option>
                  <option value="dark">Dark</option>
                  <option value="light">Light</option>
                </select>
              </label>
            </div>

            <div className="appearance-section">
              <div>
                <p className="eyebrow">Terminal</p>
                <h3>Palette and font</h3>
              </div>
              <label>
                Terminal theme
                <select
                  aria-label="Terminal theme"
                  disabled={loading || busy}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      terminalThemeId: event.target.value,
                    }))
                  }
                  value={draft.terminalThemeId}
                >
                  {allThemes.map((theme) => (
                    <option key={theme.id} value={theme.id}>
                      {theme.label}
                      {theme.builtIn ? " · built-in" : " · custom"}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Font
                <select
                  aria-label="Terminal font"
                  disabled={loading || busy}
                  onChange={(event) => {
                    const option =
                      fontOptions.find(
                        (candidate) => candidate.value === event.target.value,
                      ) ?? fontOptions[0]!;
                    setDraft((current) => ({
                      ...current,
                      fontSourceKind: option.sourceKind,
                      fontId: option.fontId,
                      fontFamily: option.family,
                    }));
                  }}
                  value={selectedFontOption.value}
                >
                  {fontOptions.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
              <div className="field-grid">
                <label>
                  Font size
                  <input
                    aria-label="Terminal font size"
                    disabled={loading || busy}
                    max="32"
                    min="10"
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        fontSize: Number(event.target.value),
                      }))
                    }
                    type="number"
                    value={draft.fontSize}
                  />
                </label>
                <label>
                  Line height
                  <select
                    aria-label="Terminal line height"
                    disabled={loading || busy}
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        lineHeightMillis: Number(event.target.value),
                      }))
                    }
                    value={draft.lineHeightMillis}
                  >
                    <option value="1200">Compact · 1.20</option>
                    <option value="1420">Balanced · 1.42</option>
                    <option value="1600">Relaxed · 1.60</option>
                    <option value="1800">Spacious · 1.80</option>
                  </select>
                </label>
              </div>
              <label className="toggle-field">
                <input
                  checked={draft.ligaturesEnabled}
                  disabled={loading || busy}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      ligaturesEnabled: event.target.checked,
                    }))
                  }
                  type="checkbox"
                />
                Programming ligatures
              </label>
              <label>
                East Asian ambiguous width
                <select
                  aria-label="East Asian ambiguous width"
                  disabled={loading || busy}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      ambiguousWidth: event.target
                        .value as AppearanceSettings["ambiguousWidth"],
                    }))
                  }
                  value={draft.ambiguousWidth}
                >
                  <option value="narrow">Narrow</option>
                  <option value="wide">Wide</option>
                </select>
              </label>
            </div>

            <button
              className="connect-button"
              disabled={loading || busy}
              type="submit"
            >
              {busy ? "Applying…" : "Apply appearance"}
            </button>
          </form>

          <div className="appearance-preview-column">
            <div
              className="terminal-appearance-preview"
              style={{
                background: selectedTheme.palette.background,
                color: selectedTheme.palette.foreground,
                fontFamily: previewFontFamily(draft),
                fontSize: `${draft.fontSize}px`,
                lineHeight: String(draft.lineHeightMillis / 1000),
              }}
            >
              <span style={{ color: selectedTheme.palette.brightCyan }}>
                anyssh@preview
              </span>
              <span style={{ color: selectedTheme.palette.foreground }}>
                :~$ unicode
              </span>
              <strong>中文 😀 👩‍💻   → != =&gt;</strong>
              <small>
                {selectedTheme.label} · {draft.fontFamily} ·{" "}
                {draft.ambiguousWidth}
              </small>
            </div>

            <section className="appearance-assets">
              <div>
                <p className="eyebrow">Data-only assets</p>
                <h3>Custom resources</h3>
              </div>
              {themes.length === 0 && fonts.length === 0 ? (
                <p className="manager-empty compact-manager-empty">
                  No custom Theme or Font has been imported.
                </p>
              ) : (
                <div className="appearance-asset-list">
                  {themes.map((theme) => (
                    <div key={theme.id}>
                      <span>
                        <strong>{theme.label}</strong>
                        <small>Terminal Theme v{theme.schemaVersion}</small>
                      </span>
                      <button
                        className="danger-action"
                        disabled={busy}
                        onClick={() => void removeTheme(theme.id)}
                        type="button"
                      >
                        Delete
                      </button>
                    </div>
                  ))}
                  {fonts.map((font) => (
                    <div key={font.id}>
                      <span>
                        <strong>{font.family}</strong>
                        <small>
                          {font.style} · {font.format.toUpperCase()}
                        </small>
                      </span>
                      <button
                        className="danger-action"
                        disabled={busy}
                        onClick={() => void removeFont(font.id)}
                        type="button"
                      >
                        Delete
                      </button>
                    </div>
                  ))}
                </div>
              )}
              <p className="appearance-native-note">
                Native pickers keep the source Path and Font bytes in Rust.
                Browser QA simulates metadata only.
              </p>
            </section>
          </div>
        </div>
      </section>
    </div>
  );
}

function uniqueSystemFonts(fonts: SystemFontSummary[]): SystemFontSummary[] {
  const unique = new Map<string, SystemFontSummary>();
  for (const font of fonts) {
    if (font.family === "ui-monospace" || font.family === "monospace") {
      continue;
    }
    if (!unique.has(font.family)) unique.set(font.family, font);
  }
  return [...unique.values()];
}

function previewFontFamily(settings: AppearanceSettings): string {
  const primary =
    settings.fontSourceKind === "bundled"
      ? '"AnySSH Nerd Mono"'
      : settings.fontSourceKind === "imported" && settings.fontId
        ? `"${importedFontCssFamily(settings.fontId)}"`
        : settings.fontFamily;
  return `${primary}, "Noto Emoji Variable", "Noto Sans Mono CJK SC", monospace`;
}
