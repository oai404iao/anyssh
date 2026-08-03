import { Checkbox } from "@base-ui/react/checkbox";
import { useId, type ReactNode } from "react";
import { classNames } from "./classNames";

interface CheckboxFieldProps {
  checked: boolean;
  className?: string;
  description?: ReactNode;
  disabled?: boolean;
  label: ReactNode;
  onCheckedChange(checked: boolean): void;
}

export function CheckboxField({
  checked,
  className,
  description,
  disabled,
  label,
  onCheckedChange,
}: CheckboxFieldProps) {
  const inputId = useId();

  return (
    <div className={classNames("ui-checkbox-field", className)}>
      <Checkbox.Root
        checked={checked}
        className="ui-checkbox"
        data-ui-control="checkbox"
        disabled={disabled}
        id={inputId}
        onCheckedChange={(nextChecked) => onCheckedChange(nextChecked)}
      >
        <Checkbox.Indicator className="ui-checkbox-indicator">
          <CheckIcon />
        </Checkbox.Indicator>
      </Checkbox.Root>
      <label className="ui-toggle-copy" htmlFor={inputId}>
        <strong>{label}</strong>
        {description && <small>{description}</small>}
      </label>
    </div>
  );
}

function CheckIcon() {
  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 16 16">
      <path
        d="m3 8.5 3 3 7-7"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="1.8"
      />
    </svg>
  );
}
