import { NumberField as BaseNumberField } from "@base-ui/react/number-field";
import { useId } from "react";
import { classNames } from "./classNames";

type NumberFieldProps = {
  ariaLabel?: string;
  className?: string;
  disabled?: boolean;
  label: string;
  max?: number;
  min?: number;
  onValueChange: (value: number) => void;
  step?: number;
  value: number;
};

export function NumberField({
  ariaLabel,
  className,
  disabled = false,
  label,
  max,
  min,
  onValueChange,
  step = 1,
  value,
}: NumberFieldProps) {
  const inputId = useId();
  const accessibleName = ariaLabel ?? label;

  return (
    <BaseNumberField.Root
      className={classNames("ui-field ui-number-field", className)}
      disabled={disabled}
      id={inputId}
      max={max}
      min={min}
      onValueChange={(nextValue) => {
        if (nextValue !== null) {
          onValueChange(nextValue);
        }
      }}
      step={step}
      value={value}
    >
      <label className="ui-field-label" htmlFor={inputId}>
        {label}
      </label>
      <BaseNumberField.Group className="ui-number-field-group">
        <BaseNumberField.Decrement
          aria-label={`Decrease ${accessibleName}`}
          className="ui-number-field-stepper"
        >
          <MinusIcon />
        </BaseNumberField.Decrement>
        <BaseNumberField.Input
          aria-label={accessibleName}
          className="ui-number-field-input"
          data-ui-control="number"
        />
        <BaseNumberField.Increment
          aria-label={`Increase ${accessibleName}`}
          className="ui-number-field-stepper"
        >
          <PlusIcon />
        </BaseNumberField.Increment>
      </BaseNumberField.Group>
    </BaseNumberField.Root>
  );
}

function MinusIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 18 18">
      <path d="M4 9h10" />
    </svg>
  );
}

function PlusIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 18 18">
      <path d="M4 9h10M9 4v10" />
    </svg>
  );
}
