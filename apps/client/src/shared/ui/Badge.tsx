import type { HTMLAttributes, ReactNode } from "react";
import { classNames } from "./classNames";

type BadgeTone = "neutral" | "primary" | "success" | "warning" | "danger";

interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  children: ReactNode;
  tone?: BadgeTone;
}

export function Badge({
  children,
  className,
  tone = "neutral",
  ...props
}: BadgeProps) {
  return (
    <span
      className={classNames("ui-badge", `ui-badge-${tone}`, className)}
      {...props}
    >
      {children}
    </span>
  );
}
