import type { ReferenceOverride } from "../../lib/host-bridge";

export function cloneReferenceOverride(
  value: ReferenceOverride,
): ReferenceOverride {
  return value.kind === "set"
    ? { kind: "set", value: value.value }
    : { ...value };
}

export function overrideHasSelection(value: ReferenceOverride) {
  return value.kind !== "set" || Boolean(value.value);
}

export function overrideStateLabel(value: ReferenceOverride) {
  switch (value.kind) {
    case "inherit":
      return "Inherited";
    case "set":
      return "Set here";
    case "clear":
      return "Cleared";
  }
}
