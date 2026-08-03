import type { ReactNode } from "react";

export interface ManagerProps {
  loading: boolean;
  onChanged(): Promise<void>;
}

interface ManagerShellProps {
  eyebrow: string;
  title: string;
  description: string;
  action: ReactNode;
  children: ReactNode;
}

export function ManagerShell({
  eyebrow,
  title,
  description,
  action,
  children,
}: ManagerShellProps) {
  return (
    <section className="manager-shell">
      <header className="manager-header">
        <div>
          <p className="eyebrow">{eyebrow}</p>
          <h2>{title}</h2>
          <p>{description}</p>
        </div>
        {action}
      </header>
      {children}
    </section>
  );
}

export function ManagerError({ message }: { message: string }) {
  return (
    <div className="manager-error" role="alert">
      {message}
    </div>
  );
}

export function ManagerEmpty({ children }: { children: ReactNode }) {
  return <div className="manager-empty">{children}</div>;
}

interface EditorDialogProps {
  title: string;
  onClose(): void;
  closeDisabled?: boolean;
  children: ReactNode;
}

export function EditorDialog({
  title,
  onClose,
  closeDisabled = false,
  children,
}: EditorDialogProps) {
  return (
    <div className="dialog-backdrop resource-dialog-backdrop">
      <section
        aria-labelledby="resource-dialog-title"
        aria-modal="true"
        className="resource-dialog"
        role="dialog"
      >
        <header>
          <div>
            <p className="eyebrow">Vault configuration</p>
            <h2 id="resource-dialog-title">{title}</h2>
          </div>
          <button
            aria-label="Close editor"
            disabled={closeDisabled}
            onClick={onClose}
            type="button"
          >
            ×
          </button>
        </header>
        {children}
      </section>
    </div>
  );
}

export function EditorActions({
  busy,
  busyLabel = "Saving…",
  submitLabel,
}: {
  busy: boolean;
  busyLabel?: string;
  submitLabel: string;
}) {
  return (
    <div className="editor-actions">
      <button className="connect-button" disabled={busy} type="submit">
        {busy ? busyLabel : submitLabel}
      </button>
    </div>
  );
}
