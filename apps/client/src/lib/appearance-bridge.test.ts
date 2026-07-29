import { beforeEach, describe, expect, it } from "vitest";
import {
  BUILT_IN_TERMINAL_THEMES,
  DEFAULT_APPEARANCE_SETTINGS,
  createTerminalTheme,
  deleteFontAsset,
  deleteTerminalTheme,
  getAppearanceSettings,
  importFontAsset,
  importTerminalTheme,
  listFontAssets,
  listSystemFonts,
  listTerminalThemes,
  resetBrowserAppearanceForTests,
  terminalThemeById,
  updateAppearanceSettings,
} from "./appearance-bridge";

describe("browser preview Appearance bridge", () => {
  beforeEach(() => {
    resetBrowserAppearanceForTests();
  });

  it("simulates native Theme and Font import with metadata only", async () => {
    const theme = await importTerminalTheme();
    expect(theme).not.toBeNull();
    expect(JSON.stringify(theme)).not.toContain("path");
    expect(JSON.stringify(theme)).not.toContain("script");

    const font = await importFontAsset();
    expect(font).toMatchObject({
      family: "Browser QA Mono",
      format: "ttf",
    });
    expect(JSON.stringify(font)).not.toContain("path");
    expect(JSON.stringify(font)).not.toContain("bytes");
    await expect(listFontAssets()).resolves.toEqual([font]);

    const systemFonts = await listSystemFonts();
    expect(systemFonts.some((candidate) => candidate.monospaced)).toBe(true);
    expect(JSON.stringify(systemFonts)).not.toContain("path");

    await updateAppearanceSettings({
      ...DEFAULT_APPEARANCE_SETTINGS,
      appTheme: "light",
      terminalThemeId: theme!.id,
      fontSourceKind: "imported",
      fontId: font!.id,
      fontFamily: font!.family,
    });
    await expect(deleteFontAsset(font!.id)).resolves.toBe(true);
    await expect(getAppearanceSettings()).resolves.toMatchObject({
      appTheme: "light",
      terminalThemeId: theme!.id,
      fontSourceKind: "bundled",
      fontId: DEFAULT_APPEARANCE_SETTINGS.fontId,
    });
  });

  it("persists bounded Appearance settings without executable fields", async () => {
    const updated = await updateAppearanceSettings({
      ...DEFAULT_APPEARANCE_SETTINGS,
      appTheme: "light",
      terminalThemeId: "builtin:solarized-light",
      fontSize: 17,
      lineHeightMillis: 1600,
      ligaturesEnabled: true,
      ambiguousWidth: "wide",
    });

    expect(updated).toMatchObject({
      appTheme: "light",
      terminalThemeId: "builtin:solarized-light",
      fontSize: 17,
      ligaturesEnabled: true,
    });
    await expect(getAppearanceSettings()).resolves.toEqual(updated);
    expect(JSON.stringify(updated)).not.toContain("script");
    expect(JSON.stringify(updated)).not.toContain("url");
    expect(JSON.stringify(updated)).not.toContain("path");
  });

  it("creates data-only Terminal Themes and falls back after deletion", async () => {
    const custom = await createTerminalTheme(
      "QA Theme",
      BUILT_IN_TERMINAL_THEMES[1]!.palette,
    );
    expect(custom.id).toMatch(/^theme-browser-/u);
    await expect(listTerminalThemes()).resolves.toEqual([custom]);

    await updateAppearanceSettings({
      ...DEFAULT_APPEARANCE_SETTINGS,
      terminalThemeId: custom.id,
    });
    expect(terminalThemeById(await listTerminalThemes(), custom.id).label).toBe(
      "QA Theme",
    );

    await expect(deleteTerminalTheme(custom.id)).resolves.toBe(true);
    await expect(getAppearanceSettings()).resolves.toMatchObject({
      terminalThemeId: "builtin:obsidian",
    });

    await expect(
      createTerminalTheme("Remote", {
        ...BUILT_IN_TERMINAL_THEMES[0]!.palette,
        background: "url(https://example.invalid/theme)",
      }),
    ).rejects.toThrow("invalid");
  });
});
