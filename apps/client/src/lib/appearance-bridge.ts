import { invoke, isTauri } from "@tauri-apps/api/core";

export type AppTheme = "system" | "dark" | "light";
export type FontSourceKind = "bundled" | "system" | "imported";
export type AmbiguousWidth = "narrow" | "wide";
export type FontAssetFormat = "ttf" | "otf" | "ttc" | "woff2";

export interface AppearanceSettings {
  appTheme: AppTheme;
  terminalThemeId: string;
  fontSourceKind: FontSourceKind;
  fontId: string | null;
  fontFamily: string;
  fontSize: number;
  lineHeightMillis: number;
  ligaturesEnabled: boolean;
  ambiguousWidth: AmbiguousWidth;
}

export interface TerminalPalette {
  background: string;
  foreground: string;
  cursor: string;
  cursorAccent: string;
  selectionBackground: string;
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  brightBlack: string;
  brightRed: string;
  brightGreen: string;
  brightYellow: string;
  brightBlue: string;
  brightMagenta: string;
  brightCyan: string;
  brightWhite: string;
}

export interface TerminalThemeSummary {
  id: string;
  label: string;
  schemaVersion: number;
  palette: TerminalPalette;
  builtIn?: boolean;
}

export interface FontAssetSummary {
  id: string;
  family: string;
  style: string;
  format: FontAssetFormat;
  sha256Hex: string;
  sizeBytes: number;
}

export interface SystemFontSummary {
  family: string;
  style: string;
  monospaced: boolean;
}

export const DEFAULT_APPEARANCE_SETTINGS: AppearanceSettings = {
  appTheme: "dark",
  terminalThemeId: "builtin:obsidian",
  fontSourceKind: "bundled",
  fontId: "builtin:anyssh-nerd-mono",
  fontFamily: "AnySSH Nerd Mono",
  fontSize: 13,
  lineHeightMillis: 1420,
  ligaturesEnabled: false,
  ambiguousWidth: "narrow",
};

export const BUILT_IN_TERMINAL_THEMES: TerminalThemeSummary[] = [
  {
    id: "builtin:obsidian",
    label: "Obsidian",
    schemaVersion: 1,
    builtIn: true,
    palette: {
      background: "#090d16",
      foreground: "#c8d0df",
      cursor: "#6be6d2",
      cursorAccent: "#090d16",
      selectionBackground: "#294a50",
      black: "#11151f",
      red: "#ff7888",
      green: "#6be6d2",
      yellow: "#ffc66d",
      blue: "#7aa2f7",
      magenta: "#b29cff",
      cyan: "#6be6d2",
      white: "#c8d0df",
      brightBlack: "#667188",
      brightRed: "#ff9aa6",
      brightGreen: "#93f2e2",
      brightYellow: "#ffdb9e",
      brightBlue: "#a5c2ff",
      brightMagenta: "#c9bdff",
      brightCyan: "#9af4e5",
      brightWhite: "#f1f5ff",
    },
  },
  {
    id: "builtin:aurora",
    label: "Aurora",
    schemaVersion: 1,
    builtIn: true,
    palette: {
      background: "#071716",
      foreground: "#d7f4ed",
      cursor: "#7df5cf",
      cursorAccent: "#071716",
      selectionBackground: "#255f57",
      black: "#0b2422",
      red: "#ff7f8f",
      green: "#7df5cf",
      yellow: "#ffd47d",
      blue: "#75bfff",
      magenta: "#c6a7ff",
      cyan: "#5de5dc",
      white: "#d7f4ed",
      brightBlack: "#5a837d",
      brightRed: "#ffa6b1",
      brightGreen: "#a5ffe3",
      brightYellow: "#ffe2a5",
      brightBlue: "#a4d5ff",
      brightMagenta: "#dccaff",
      brightCyan: "#94f5ee",
      brightWhite: "#f3fffc",
    },
  },
  {
    id: "builtin:solarized-light",
    label: "Solarized Light",
    schemaVersion: 1,
    builtIn: true,
    palette: {
      background: "#fdf6e3",
      foreground: "#586e75",
      cursor: "#268bd2",
      cursorAccent: "#fdf6e3",
      selectionBackground: "#eee8d5",
      black: "#073642",
      red: "#dc322f",
      green: "#859900",
      yellow: "#b58900",
      blue: "#268bd2",
      magenta: "#d33682",
      cyan: "#2aa198",
      white: "#eee8d5",
      brightBlack: "#657b83",
      brightRed: "#cb4b16",
      brightGreen: "#93a1a1",
      brightYellow: "#839496",
      brightBlue: "#6c71c4",
      brightMagenta: "#d33682",
      brightCyan: "#2aa198",
      brightWhite: "#fdf6e3",
    },
  },
];

let browserAppearance = cloneAppearance(DEFAULT_APPEARANCE_SETTINGS);
let browserThemes: TerminalThemeSummary[] = [];
let browserFonts: FontAssetSummary[] = [];
let nextBrowserThemeId = 1;
let nextBrowserFontId = 1;

const BROWSER_SYSTEM_FONTS: SystemFontSummary[] = [
  { family: "ui-monospace", style: "Regular", monospaced: true },
  { family: "monospace", style: "Regular", monospaced: true },
  { family: "Courier New", style: "Regular", monospaced: true },
];

export async function getAppearanceSettings(): Promise<AppearanceSettings> {
  if (!isTauri()) return cloneAppearance(browserAppearance);
  return invoke<AppearanceSettings>("appearance_get");
}

export async function updateAppearanceSettings(
  settings: AppearanceSettings,
): Promise<AppearanceSettings> {
  if (!isTauri()) {
    validateAppearance(settings);
    browserAppearance = cloneAppearance(settings);
    return cloneAppearance(browserAppearance);
  }
  return invoke<AppearanceSettings>("appearance_update", {
    request: settings,
  });
}

export async function listTerminalThemes(): Promise<TerminalThemeSummary[]> {
  if (!isTauri()) return browserThemes.map(cloneTheme);
  return invoke<TerminalThemeSummary[]>("terminal_theme_list");
}

export async function createTerminalTheme(
  label: string,
  palette: TerminalPalette,
): Promise<TerminalThemeSummary> {
  if (!isTauri()) {
    validatePalette(palette);
    const theme = {
      id: `theme-browser-${nextBrowserThemeId++}`,
      label,
      schemaVersion: 1,
      palette: clonePalette(palette),
    };
    browserThemes.push(theme);
    return cloneTheme(theme);
  }
  return invoke<TerminalThemeSummary>("terminal_theme_create", {
    request: { label, palette },
  });
}

export async function importTerminalTheme(): Promise<TerminalThemeSummary | null> {
  if (!isTauri()) {
    return createTerminalTheme("Browser QA Midnight", {
      ...BUILT_IN_TERMINAL_THEMES[1]!.palette,
      background: "#101426",
      cursorAccent: "#101426",
    });
  }
  return invoke<TerminalThemeSummary | null>("terminal_theme_import");
}

export async function deleteTerminalTheme(themeId: string): Promise<boolean> {
  if (!isTauri()) {
    const before = browserThemes.length;
    browserThemes = browserThemes.filter((theme) => theme.id !== themeId);
    if (browserAppearance.terminalThemeId === themeId) {
      browserAppearance.terminalThemeId = "builtin:obsidian";
    }
    return browserThemes.length !== before;
  }
  return invoke<boolean>("terminal_theme_delete", {
    request: { themeId },
  });
}

export async function listFontAssets(): Promise<FontAssetSummary[]> {
  if (!isTauri()) return browserFonts.map((font) => ({ ...font }));
  return invoke<FontAssetSummary[]>("font_asset_list");
}

export async function importFontAsset(): Promise<FontAssetSummary | null> {
  if (!isTauri()) {
    const font: FontAssetSummary = {
      id: `font-browser-${nextBrowserFontId++}`,
      family: "Browser QA Mono",
      style: "Regular",
      format: "ttf",
      sha256Hex: "a".repeat(64),
      sizeBytes: 4096,
    };
    browserFonts.push(font);
    return { ...font };
  }
  return invoke<FontAssetSummary | null>("font_asset_import");
}

export async function listSystemFonts(): Promise<SystemFontSummary[]> {
  if (!isTauri()) return BROWSER_SYSTEM_FONTS.map((font) => ({ ...font }));
  return invoke<SystemFontSummary[]>("font_system_list");
}

export async function deleteFontAsset(fontId: string): Promise<boolean> {
  if (!isTauri()) {
    const before = browserFonts.length;
    browserFonts = browserFonts.filter((font) => font.id !== fontId);
    if (
      browserAppearance.fontSourceKind === "imported" &&
      browserAppearance.fontId === fontId
    ) {
      browserAppearance = {
        ...browserAppearance,
        fontSourceKind: "bundled",
        fontId: DEFAULT_APPEARANCE_SETTINGS.fontId,
        fontFamily: DEFAULT_APPEARANCE_SETTINGS.fontFamily,
      };
    }
    return browserFonts.length !== before;
  }
  return invoke<boolean>("font_asset_delete", {
    request: { fontId },
  });
}

export function fontAssetUrl(font: FontAssetSummary): string {
  const path = `${font.id}/${font.sha256Hex}.${font.format}`;
  if (/Windows|Android/u.test(navigator.userAgent)) {
    return `https://anyssh-font.localhost/${path}`;
  }
  return `anyssh-font://localhost/${path}`;
}

export function importedFontCssFamily(fontId: string): string {
  return `AnySSH Imported ${fontId}`;
}

export function terminalThemeById(
  themes: TerminalThemeSummary[],
  themeId: string,
): TerminalThemeSummary {
  return (
    [...BUILT_IN_TERMINAL_THEMES, ...themes].find(
      (theme) => theme.id === themeId,
    ) ?? BUILT_IN_TERMINAL_THEMES[0]!
  );
}

export function resetBrowserAppearanceForTests(): void {
  browserAppearance = cloneAppearance(DEFAULT_APPEARANCE_SETTINGS);
  browserThemes = [];
  browserFonts = [];
  nextBrowserThemeId = 1;
  nextBrowserFontId = 1;
}

function validateAppearance(settings: AppearanceSettings): void {
  if (
    settings.fontSize < 10 ||
    settings.fontSize > 32 ||
    settings.lineHeightMillis < 1000 ||
    settings.lineHeightMillis > 2000 ||
    settings.fontFamily.length === 0 ||
    settings.fontFamily.length > 128
  ) {
    throw new Error("Appearance settings are invalid");
  }
  const themeExists = [...BUILT_IN_TERMINAL_THEMES, ...browserThemes].some(
    (theme) => theme.id === settings.terminalThemeId,
  );
  if (!themeExists) throw new Error("Terminal Theme was not found");
  if (
    settings.fontSourceKind === "bundled" &&
    (settings.fontId !== DEFAULT_APPEARANCE_SETTINGS.fontId ||
      settings.fontFamily !== DEFAULT_APPEARANCE_SETTINGS.fontFamily)
  ) {
    throw new Error("Bundled Font is invalid");
  }
  if (
    settings.fontSourceKind === "system" &&
    (settings.fontId !== null ||
      !BROWSER_SYSTEM_FONTS.some((font) => font.family === settings.fontFamily))
  ) {
    throw new Error("System Font was not found");
  }
  if (
    settings.fontSourceKind === "imported" &&
    !browserFonts.some(
      (font) =>
        font.id === settings.fontId && font.family === settings.fontFamily,
    )
  ) {
    throw new Error("Font Asset was not found");
  }
}

function validatePalette(palette: TerminalPalette): void {
  for (const color of Object.values(palette)) {
    if (!/^#[0-9a-fA-F]{6}(?:[0-9a-fA-F]{2})?$/u.test(color)) {
      throw new Error("Terminal Theme is invalid");
    }
  }
}

function cloneAppearance(settings: AppearanceSettings): AppearanceSettings {
  return { ...settings };
}

function clonePalette(palette: TerminalPalette): TerminalPalette {
  return { ...palette };
}

function cloneTheme(theme: TerminalThemeSummary): TerminalThemeSummary {
  return {
    ...theme,
    palette: clonePalette(theme.palette),
  };
}
