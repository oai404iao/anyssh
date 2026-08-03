import type { ReferenceOverride } from "../../lib/host-bridge";

interface ReferenceOverrideEditorProps {
  label: string;
  inheritLabel: string;
  setLabel: string;
  value: ReferenceOverride;
  options: Array<{ id: string; label: string }>;
  onChange(value: ReferenceOverride): void;
}

export function ReferenceOverrideEditor({
  label,
  inheritLabel,
  setLabel,
  value,
  options,
  onChange,
}: ReferenceOverrideEditorProps) {
  return (
    <fieldset className="override-editor">
      <legend>{label}</legend>
      <label>
        Behavior
        <select
          aria-label={`${label} behavior`}
          onChange={(event) => {
            switch (event.target.value) {
              case "set":
                onChange({
                  kind: "set",
                  value:
                    value.kind === "set" ? value.value : (options[0]?.id ?? ""),
                });
                break;
              case "clear":
                onChange({ kind: "clear" });
                break;
              default:
                onChange({ kind: "inherit" });
            }
          }}
          value={value.kind}
        >
          <option value="inherit">{inheritLabel}</option>
          <option value="set">{setLabel}</option>
          <option value="clear">Clear inherited value</option>
        </select>
      </label>
      {value.kind === "set" && (
        <label>
          {label} reference
          <select
            aria-label={`${label} reference`}
            onChange={(event) =>
              onChange({ kind: "set", value: event.target.value })
            }
            value={value.value}
          >
            {options.length === 0 && (
              <option value="">No {label} available</option>
            )}
            {options.map((option) => (
              <option key={option.id} value={option.id}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
      )}
    </fieldset>
  );
}
