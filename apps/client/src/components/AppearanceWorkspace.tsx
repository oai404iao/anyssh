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
import {
  Button,
  NumberField,
  SelectField,
  SwitchField,
  type SelectOption,
} from "../shared/ui";

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

const APP_THEME_OPTIONS: readonly SelectOption<
  AppearanceSettings["appTheme"]
>[] = [
  { value: "system", label: "Follow system" },
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
];

const LINE_HEIGHT_OPTIONS: readonly SelectOption<string>[] = [
  { value: "1200", label: "Compact · 1.20" },
  { value: "1420", label: "Balanced · 1.42" },
  { value: "1600", label: "Relaxed · 1.60" },
  { value: "1800", label: "Spacious · 1.80" },
];

const AMBIGUOUS_WIDTH_OPTIONS: readonly SelectOption<
  AppearanceSettings["ambiguousWidth"]
>[] = [
  { value: "narrow", label: "Narrow" },
  { value: "wide", label: "Wide" },
];

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
            <Button
              className="compact-button"
              disabled={busy}
              onClick={() => void importTheme()}
              size="small"
              variant="outlined"
            >
              Import Theme
            </Button>
            <Button
              className="compact-button"
              disabled={busy}
              onClick={() => void importFont()}
              size="small"
            >
              Import Font
            </Button>
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
              <SelectField
                ariaLabel="App theme"
                disabled={loading || busy}
                label="App theme"
                onValueChange={(appTheme) =>
                  setDraft((current) => ({ ...current, appTheme }))
                }
                options={APP_THEME_OPTIONS}
                value={draft.appTheme}
              />
            </div>

            <div className="appearance-section">
              <div>
                <p className="eyebrow">Terminal</p>
                <h3>Palette and font</h3>
              </div>
              <SelectField
                ariaLabel="Terminal theme"
                disabled={loading || busy}
                label="Terminal theme"
                onValueChange={(terminalThemeId) =>
                  setDraft((current) => ({ ...current, terminalThemeId }))
                }
                options={allThemes.map((theme) => ({
                  value: theme.id,
                  label: `${theme.label}${
                    theme.builtIn ? " · built-in" : " · custom"
                  }`,
                }))}
                value={draft.terminalThemeId}
              />
              <SelectField
                ariaLabel="Terminal font"
                disabled={loading || busy}
                label="Font"
                onValueChange={(value) => {
                  const option =
                    fontOptions.find(
                      (candidate) => candidate.value === value,
                    ) ?? fontOptions[0]!;
                  setDraft((current) => ({
                    ...current,
                    fontSourceKind: option.sourceKind,
                    fontId: option.fontId,
                    fontFamily: option.family,
                  }));
                }}
                options={fontOptions}
                value={selectedFontOption.value}
              />
              <div className="field-grid">
                <NumberField
                  ariaLabel="Terminal font size"
                  disabled={loading || busy}
                  label="Font size"
                  max={32}
                  min={10}
                  onValueChange={(fontSize) =>
                    setDraft((current) => ({ ...current, fontSize }))
                  }
                  value={draft.fontSize}
                />
                <SelectField
                  ariaLabel="Terminal line height"
                  disabled={loading || busy}
                  label="Line height"
                  onValueChange={(value) =>
                    setDraft((current) => ({
                      ...current,
                      lineHeightMillis: Number(value),
                    }))
                  }
                  options={LINE_HEIGHT_OPTIONS}
                  value={String(draft.lineHeightMillis)}
                />
              </div>
              <SwitchField
                checked={draft.ligaturesEnabled}
                disabled={loading || busy}
                label="Programming ligatures"
                onCheckedChange={(ligaturesEnabled) =>
                  setDraft((current) => ({
                    ...current,
                    ligaturesEnabled,
                  }))
                }
              />
              <SelectField
                ariaLabel="East Asian ambiguous width"
                disabled={loading || busy}
                label="East Asian ambiguous width"
                onValueChange={(ambiguousWidth) =>
                  setDraft((current) => ({ ...current, ambiguousWidth }))
                }
                options={AMBIGUOUS_WIDTH_OPTIONS}
                value={draft.ambiguousWidth}
              />
            </div>

            <Button disabled={loading || busy} type="submit">
              {busy ? "Applying…" : "Apply appearance"}
            </Button>
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
                      <Button
                        className="danger-action"
                        disabled={busy}
                        onClick={() => void removeTheme(theme.id)}
                        size="small"
                        variant="danger"
                      >
                        Delete
                      </Button>
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
                      <Button
                        className="danger-action"
                        disabled={busy}
                        onClick={() => void removeFont(font.id)}
                        size="small"
                        variant="danger"
                      >
                        Delete
                      </Button>
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
