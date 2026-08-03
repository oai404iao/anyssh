import type { ReactNode } from "react";
import { isLinuxDesktopPlatform } from "../shared/platform/runtime";
import { WindowTitlebar } from "./shell/WindowTitlebar";

interface AppFrameProps {
  children: ReactNode;
  workspaceTitle: string;
}

export function AppFrame({ children, workspaceTitle }: AppFrameProps) {
  const customWindowChrome = isLinuxDesktopPlatform();

  return (
    <div
      className={`app-frame ${
        customWindowChrome ? "has-custom-window-chrome" : ""
      }`}
    >
      {customWindowChrome && <WindowTitlebar workspaceTitle={workspaceTitle} />}
      <div className="app-frame-content">{children}</div>
    </div>
  );
}
