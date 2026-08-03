import type { HTMLAttributes, ReactNode } from "react";
import { classNames } from "./classNames";

type SurfaceLevel = "low" | "default" | "high";

interface SurfaceProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  level?: SurfaceLevel;
}

export function Surface({
  children,
  className,
  level = "default",
  ...props
}: SurfaceProps) {
  return (
    <div
      className={classNames("ui-surface", `ui-surface-${level}`, className)}
      {...props}
    >
      {children}
    </div>
  );
}
