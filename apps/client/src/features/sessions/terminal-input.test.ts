import { describe, expect, it } from "vitest";
import {
  applyTerminalModifiers,
  terminalAuxiliaryInput,
} from "./terminal-input";

describe("terminal mobile input", () => {
  it("converts a latched Control modifier into ASCII control input", () => {
    expect(applyTerminalModifiers("c", { alt: false, control: true })).toBe(
      "\u0003",
    );
    expect(applyTerminalModifiers("[", { alt: false, control: true })).toBe(
      "\u001b",
    );
  });

  it("prefixes the next input with Escape for Alt", () => {
    expect(applyTerminalModifiers("x", { alt: true, control: false })).toBe(
      "\u001bx",
    );
    expect(applyTerminalModifiers("x", { alt: true, control: true })).toBe(
      "\u001b\u0018",
    );
  });

  it("preserves composed Unicode input when Control has no mapping", () => {
    expect(applyTerminalModifiers("中文", { alt: false, control: true })).toBe(
      "中文",
    );
  });

  it("encodes plain and modified arrow keys", () => {
    expect(
      terminalAuxiliaryInput("arrowUp", {
        alt: false,
        control: false,
      }),
    ).toBe("\u001b[A");
    expect(
      terminalAuxiliaryInput("arrowLeft", {
        alt: true,
        control: true,
      }),
    ).toBe("\u001b[1;7D");
  });
});
