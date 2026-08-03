import { NavigationIcon } from "../../shared/icons/ProductIcons";
import type { TerminalAuxiliaryKey, TerminalModifiers } from "./terminal-input";

interface TerminalAuxiliaryKeyboardProps {
  connected: boolean;
  modifiers: TerminalModifiers;
  onSend(key: TerminalAuxiliaryKey): void;
  onToggleModifier(modifier: keyof TerminalModifiers): void;
}

const AUXILIARY_KEYS: Array<{
  key: TerminalAuxiliaryKey;
  label: string;
  accessibleName: string;
}> = [
  { key: "escape", label: "ESC", accessibleName: "Send Escape" },
  { key: "tab", label: "TAB", accessibleName: "Send Tab" },
  { key: "arrowUp", label: "↑", accessibleName: "Send Arrow Up" },
  { key: "arrowDown", label: "↓", accessibleName: "Send Arrow Down" },
  { key: "arrowLeft", label: "←", accessibleName: "Send Arrow Left" },
  { key: "arrowRight", label: "→", accessibleName: "Send Arrow Right" },
];

export function TerminalAuxiliaryKeyboard({
  connected,
  modifiers,
  onSend,
  onToggleModifier,
}: TerminalAuxiliaryKeyboardProps) {
  return (
    <div className="terminal-accessory-bar" aria-label="SSH auxiliary keyboard">
      <button
        aria-label="Toggle Control modifier"
        aria-pressed={modifiers.control}
        className={modifiers.control ? "active" : ""}
        disabled={!connected}
        onClick={() => onToggleModifier("control")}
        type="button"
      >
        CTRL
      </button>
      <button
        aria-label="Toggle Alt modifier"
        aria-pressed={modifiers.alt}
        className={modifiers.alt ? "active" : ""}
        disabled={!connected}
        onClick={() => onToggleModifier("alt")}
        type="button"
      >
        ALT
      </button>
      {AUXILIARY_KEYS.map((key) => (
        <button
          aria-label={key.accessibleName}
          disabled={!connected}
          key={key.key}
          onClick={() => onSend(key.key)}
          type="button"
        >
          {key.label}
        </button>
      ))}
    </div>
  );
}

interface MobileTerminalActionsProps {
  connectionPanelVisible: boolean;
  onFocusKeyboard(): void;
  onOpenForwarding(): void;
  onOpenSessions(): void;
  onOpenSnippets(): void;
}

export function MobileTerminalActions({
  connectionPanelVisible,
  onFocusKeyboard,
  onOpenForwarding,
  onOpenSessions,
  onOpenSnippets,
}: MobileTerminalActionsProps) {
  return (
    <nav className="mobile-terminal-actions" aria-label="Terminal actions">
      <button
        className={!connectionPanelVisible ? "active" : ""}
        onClick={onOpenSessions}
        type="button"
      >
        <span>
          <NavigationIcon name="terminal" />
        </span>
        Sessions
      </button>
      <button onClick={onOpenSnippets} type="button">
        <span>
          <NavigationIcon name="snippets" />
        </span>
        Snippets
      </button>
      <button
        className={connectionPanelVisible ? "active" : ""}
        onClick={onOpenForwarding}
        type="button"
      >
        <span>
          <NavigationIcon name="routes" />
        </span>
        Forwarding
      </button>
      <button onClick={onFocusKeyboard} type="button">
        <span>
          <NavigationIcon name="keys" />
        </span>
        Keyboard
      </button>
    </nav>
  );
}
