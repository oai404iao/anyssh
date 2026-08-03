export interface TerminalModifiers {
  alt: boolean;
  control: boolean;
}

export type TerminalAuxiliaryKey =
  "escape" | "tab" | "arrowUp" | "arrowDown" | "arrowLeft" | "arrowRight";

export function applyTerminalModifiers(
  input: string,
  modifiers: TerminalModifiers,
): string {
  let transformed = modifiers.control ? controlInput(input) : input;
  if (modifiers.alt && transformed.length > 0) {
    transformed = `\u001b${transformed}`;
  }
  return transformed;
}

export function terminalAuxiliaryInput(
  key: TerminalAuxiliaryKey,
  modifiers: TerminalModifiers,
): string {
  const arrow = arrowCode(key);
  if (arrow) {
    const modifierParameter =
      1 + (modifiers.alt ? 2 : 0) + (modifiers.control ? 4 : 0);
    return modifierParameter === 1
      ? `\u001b[${arrow}`
      : `\u001b[1;${modifierParameter}${arrow}`;
  }

  return applyTerminalModifiers(key === "escape" ? "\u001b" : "\t", modifiers);
}

function controlInput(input: string): string {
  if (Array.from(input).length !== 1) return input;
  const character = input.toUpperCase();
  if (character === " " || character === "@") return "\u0000";
  if (character === "?") return "\u007f";

  const code = character.charCodeAt(0);
  if (code >= 65 && code <= 90) {
    return String.fromCharCode(code - 64);
  }
  if (code >= 91 && code <= 95) {
    return String.fromCharCode(code - 64);
  }
  return input;
}

function arrowCode(key: TerminalAuxiliaryKey): string | null {
  switch (key) {
    case "arrowUp":
      return "A";
    case "arrowDown":
      return "B";
    case "arrowRight":
      return "C";
    case "arrowLeft":
      return "D";
    default:
      return null;
  }
}
