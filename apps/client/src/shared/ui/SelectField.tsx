import { Select } from "@base-ui/react/select";
import { useId, type ReactNode } from "react";
import { classNames } from "./classNames";

export interface SelectOption<Value extends string = string> {
  disabled?: boolean;
  label: string;
  value: Value;
}

interface SelectFieldProps<Value extends string> {
  ariaLabel?: string;
  className?: string;
  disabled?: boolean;
  label: ReactNode;
  name?: string;
  onValueChange(value: Value): void;
  options: readonly SelectOption<Value>[];
  placeholder?: string;
  triggerClassName?: string;
  value: Value;
}

export function SelectField<Value extends string>({
  ariaLabel,
  className,
  disabled,
  label,
  name,
  onValueChange,
  options,
  placeholder = "Select an option",
  triggerClassName,
  value,
}: SelectFieldProps<Value>) {
  const labelId = useId();

  return (
    <div className={classNames("ui-field", className)}>
      <span className="ui-field-label" id={labelId}>
        {label}
      </span>
      <Select.Root
        disabled={disabled}
        items={options}
        name={name}
        onValueChange={(nextValue) => {
          if (nextValue !== null) onValueChange(nextValue);
        }}
        value={value}
      >
        <Select.Trigger
          aria-label={ariaLabel}
          aria-labelledby={ariaLabel ? undefined : labelId}
          className={classNames("ui-select-trigger", triggerClassName)}
          data-ui-control="select"
          data-value={value}
        >
          <Select.Value className="ui-select-value" placeholder={placeholder} />
          <Select.Icon className="ui-select-icon">
            <ChevronIcon />
          </Select.Icon>
        </Select.Trigger>
        <Select.Portal>
          <Select.Positioner
            alignItemWithTrigger={false}
            className="ui-select-positioner"
            sideOffset={6}
          >
            <Select.Popup className="ui-select-popup">
              <Select.ScrollUpArrow className="ui-select-scroll-arrow">
                <span aria-hidden="true">↑</span>
              </Select.ScrollUpArrow>
              <Select.List className="ui-select-list">
                {options.map((option) => (
                  <Select.Item
                    className="ui-select-item"
                    disabled={option.disabled}
                    data-value={option.value}
                    key={option.value}
                    label={option.label}
                    value={option.value}
                  >
                    <Select.ItemIndicator className="ui-select-item-indicator">
                      <CheckIcon />
                    </Select.ItemIndicator>
                    <Select.ItemText>{option.label}</Select.ItemText>
                  </Select.Item>
                ))}
              </Select.List>
              <Select.ScrollDownArrow className="ui-select-scroll-arrow">
                <span aria-hidden="true">↓</span>
              </Select.ScrollDownArrow>
            </Select.Popup>
          </Select.Positioner>
        </Select.Portal>
      </Select.Root>
    </div>
  );
}

function ChevronIcon() {
  return (
    <svg aria-hidden="true" fill="none" viewBox="0 0 16 16">
      <path
        d="m4 6 4 4 4-4"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="1.6"
      />
    </svg>
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
        strokeWidth="1.7"
      />
    </svg>
  );
}
