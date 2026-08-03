import {
  importedFontCssFamily,
  type AppearanceSettings,
} from "../../lib/appearance-bridge";

export function terminalFontFamily(settings: AppearanceSettings): string {
  const genericFamilies = new Set(["monospace", "ui-monospace"]);
  const family =
    settings.fontSourceKind === "imported" && settings.fontId
      ? importedFontCssFamily(settings.fontId)
      : settings.fontFamily;
  const primary = genericFamilies.has(family)
    ? family
    : `"${family.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
  return `${primary}, "Noto Emoji Variable", "Noto Sans Mono CJK SC", "SFMono-Regular", Consolas, monospace`;
}
