import { describe, expect, it } from "vitest";
import { isAndroidPlatform, isLinuxDesktopPlatform } from "./runtime";

describe("isLinuxDesktopPlatform", () => {
  it("enables custom chrome for Linux desktop WebViews", () => {
    expect(
      isLinuxDesktopPlatform(
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1",
      ),
    ).toBe(true);
  });

  it("keeps Android and Windows on their platform chrome", () => {
    expect(
      isLinuxDesktopPlatform(
        "Mozilla/5.0 (Linux; Android 16; Pixel) AppleWebKit/537.36",
      ),
    ).toBe(false);
    expect(
      isLinuxDesktopPlatform(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
      ),
    ).toBe(false);
  });
});

describe("isAndroidPlatform", () => {
  it("detects Android WebViews without treating Linux desktop as Android", () => {
    expect(
      isAndroidPlatform(
        "Mozilla/5.0 (Linux; Android 16; Pixel) AppleWebKit/537.36",
      ),
    ).toBe(true);
    expect(
      isAndroidPlatform("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1"),
    ).toBe(false);
  });
});
