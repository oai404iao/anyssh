export function isLinuxDesktopPlatform(
  userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent,
): boolean {
  const normalized = userAgent.toLowerCase();
  return normalized.includes("linux") && !normalized.includes("android");
}

export function isAndroidPlatform(
  userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent,
): boolean {
  return userAgent.toLowerCase().includes("android");
}
