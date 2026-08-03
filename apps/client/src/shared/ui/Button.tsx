import type { ButtonHTMLAttributes, ReactNode } from "react";
import { classNames } from "./classNames";

export type ButtonVariant = "filled" | "tonal" | "outlined" | "text" | "danger";
export type ButtonSize = "small" | "medium" | "icon";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  size?: ButtonSize;
  variant?: ButtonVariant;
}

export function Button({
  children,
  className,
  size = "medium",
  type = "button",
  variant = "filled",
  ...props
}: ButtonProps) {
  return (
    <button
      className={classNames(
        "ui-button",
        `ui-button-${variant}`,
        `ui-button-${size}`,
        className,
      )}
      type={type}
      {...props}
    >
      {children}
    </button>
  );
}
