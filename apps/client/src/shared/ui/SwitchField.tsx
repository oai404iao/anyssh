import { Switch } from "@base-ui/react/switch";
import { useId, type ReactNode } from "react";
import { classNames } from "./classNames";

interface SwitchFieldProps {
  checked: boolean;
  className?: string;
  description?: ReactNode;
  disabled?: boolean;
  label: ReactNode;
  onCheckedChange(checked: boolean): void;
}

export function SwitchField({
  checked,
  className,
  description,
  disabled,
  label,
  onCheckedChange,
}: SwitchFieldProps) {
  const inputId = useId();

  return (
    <div className={classNames("ui-toggle-field", className)}>
      <label className="ui-toggle-copy" htmlFor={inputId}>
        <strong>{label}</strong>
        {description && <small>{description}</small>}
      </label>
      <Switch.Root
        checked={checked}
        className="ui-switch"
        data-ui-control="switch"
        disabled={disabled}
        id={inputId}
        onCheckedChange={(nextChecked) => onCheckedChange(nextChecked)}
      >
        <Switch.Thumb className="ui-switch-thumb" />
      </Switch.Root>
    </div>
  );
}
