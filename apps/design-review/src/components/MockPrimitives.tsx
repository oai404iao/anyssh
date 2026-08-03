import type { ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

interface MockButtonProps {
  children: ReactNode;
  compact?: boolean;
  icon?: IconName;
  onClick?: () => void;
  tone?: "filled" | "tonal" | "outlined" | "text" | "danger";
}

export function MockButton({
  children,
  compact = false,
  icon,
  onClick,
  tone = "filled",
}: MockButtonProps) {
  return (
    <button
      className={`mock-button mock-button-${tone} ${
        compact ? "mock-button-compact" : ""
      }`}
      onClick={onClick}
      type="button"
    >
      {icon && <Icon name={icon} />}
      <span>{children}</span>
    </button>
  );
}

interface MockIconButtonProps {
  label: string;
  name: IconName;
  onClick?: () => void;
  selected?: boolean;
}

export function MockIconButton({
  label,
  name,
  onClick,
  selected = false,
}: MockIconButtonProps) {
  return (
    <button
      aria-label={label}
      className={`mock-icon-button ${selected ? "selected" : ""}`}
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon name={name} />
    </button>
  );
}

interface MockFieldProps {
  label: string;
  supporting?: string;
  trailing?: ReactNode;
  value: string;
}

export function MockField({
  label,
  supporting,
  trailing,
  value,
}: MockFieldProps) {
  return (
    <div className="mock-field">
      <span className="mock-field-label">{label}</span>
      <div className="mock-field-control">
        <span>{value}</span>
        {trailing}
      </div>
      {supporting && (
        <span className="mock-field-supporting">{supporting}</span>
      )}
    </div>
  );
}

interface MockChipProps {
  children: ReactNode;
  selected?: boolean;
  tone?: "default" | "success" | "warning" | "danger";
}

export function MockChip({
  children,
  selected = false,
  tone = "default",
}: MockChipProps) {
  return (
    <span
      className={`mock-chip mock-chip-${tone} ${selected ? "selected" : ""}`}
    >
      {children}
    </span>
  );
}

interface MockSwitchProps {
  checked?: boolean;
  description?: string;
  label: string;
}

export function MockSwitch({
  checked = false,
  description,
  label,
}: MockSwitchProps) {
  return (
    <div className="mock-switch-row">
      <div>
        <strong>{label}</strong>
        {description && <span>{description}</span>}
      </div>
      <span className={`mock-switch ${checked ? "checked" : ""}`}>
        <span />
      </span>
    </div>
  );
}

interface MockListItemProps {
  badge?: ReactNode;
  description: string;
  icon: IconName;
  onClick?: () => void;
  title: string;
  trailing?: ReactNode;
}

export function MockListItem({
  badge,
  description,
  icon,
  onClick,
  title,
  trailing,
}: MockListItemProps) {
  const content = (
    <>
      <span className="mock-list-icon">
        <Icon name={icon} />
      </span>
      <span className="mock-list-copy">
        <strong>{title}</strong>
        <span>{description}</span>
        {badge}
      </span>
      <span className="mock-list-trailing">
        {trailing ?? <Icon name="chevron" />}
      </span>
    </>
  );

  if (onClick) {
    return (
      <button className="mock-list-item" onClick={onClick} type="button">
        {content}
      </button>
    );
  }

  return <div className="mock-list-item">{content}</div>;
}

export function SectionHeading({
  action,
  eyebrow,
  title,
}: {
  action?: ReactNode;
  eyebrow?: string;
  title: string;
}) {
  return (
    <div className="mock-section-heading">
      <div>
        {eyebrow && <span>{eyebrow}</span>}
        <h3>{title}</h3>
      </div>
      {action}
    </div>
  );
}

export function StatusDot({
  label,
  tone,
}: {
  label: string;
  tone: "success" | "warning" | "neutral" | "danger";
}) {
  return (
    <span className={`mock-status mock-status-${tone}`}>
      <span />
      {label}
    </span>
  );
}
