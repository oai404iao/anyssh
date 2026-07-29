import { FormEvent, useMemo, useState } from "react";
import {
  createSnippet,
  deleteSnippet,
  getSnippet,
  runSnippet,
  updateSnippet,
  type SnippetDraft,
  type SnippetSummary,
} from "../lib/snippet-bridge";

interface SnippetWorkspaceProps {
  snippets: SnippetSummary[];
  loading: boolean;
  loadError: string | null;
  activeSessionId: string | null;
  activeSessionTitle: string;
  onChanged(): Promise<void>;
}

interface EditorState {
  snippetId: string | null;
  label: string;
  body: string;
}

interface RunnerState {
  draft: SnippetDraft;
  appendEnter: boolean;
  variables: Record<string, string>;
  confirmedMultiline: boolean;
}

export function SnippetWorkspace({
  snippets,
  loading,
  loadError,
  activeSessionId,
  activeSessionTitle,
  onChanged,
}: SnippetWorkspaceProps) {
  const [editor, setEditor] = useState<EditorState | null>(null);
  const [runner, setRunner] = useState<RunnerState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const preview = useMemo(
    () => (runner ? renderPreview(runner.draft.body, runner.variables) : ""),
    [runner],
  );
  const previewIsMultiline = preview.includes("\n") || preview.includes("\r");

  async function editSnippet(snippetId: string) {
    setBusy(true);
    setError(null);
    try {
      const draft = await getSnippet(snippetId);
      setEditor({
        snippetId,
        label: draft.summary.label,
        body: draft.body,
      });
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function submitEditor(event: FormEvent) {
    event.preventDefault();
    if (!editor) return;
    setBusy(true);
    setError(null);
    try {
      if (editor.snippetId) {
        await updateSnippet({
          snippetId: editor.snippetId,
          label: editor.label,
          body: editor.body,
        });
      } else {
        await createSnippet({
          label: editor.label,
          body: editor.body,
        });
      }
      setEditor(null);
      await onChanged();
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function removeSnippet(snippetId: string) {
    setBusy(true);
    setError(null);
    try {
      await deleteSnippet(snippetId);
      await onChanged();
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function openRunner(snippetId: string, appendEnter: boolean) {
    if (!activeSessionId) return;
    setBusy(true);
    setError(null);
    try {
      const draft = await getSnippet(snippetId);
      setRunner({
        draft,
        appendEnter,
        variables: Object.fromEntries(
          draft.summary.variables.map((name) => [name, ""]),
        ),
        confirmedMultiline: false,
      });
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function submitRunner(event: FormEvent) {
    event.preventDefault();
    if (!runner || !activeSessionId) return;
    setBusy(true);
    setError(null);
    try {
      await runSnippet({
        sessionId: activeSessionId,
        snippetId: runner.draft.summary.id,
        variables: runner.variables,
        appendEnter: runner.appendEnter,
        confirmedMultiline: runner.confirmedMultiline,
      });
      setRunner(null);
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="configuration-body snippet-workspace">
      <section className="manager-shell">
        <div className="manager-header">
          <div>
            <p className="eyebrow">Remote command data</p>
            <h2>Snippets</h2>
            <p>
              Snippets are literal templates sent only to the selected SSH PTY.
              They never execute a local shell or access Credentials.
            </p>
          </div>
          <button
            className="connect-button manager-primary-action"
            disabled={busy}
            onClick={() => setEditor({ snippetId: null, label: "", body: "" })}
            type="button"
          >
            New Snippet
          </button>
        </div>

        {(loadError || error) && (
          <div className="manager-error" role="alert">
            {error ?? loadError}
          </div>
        )}

        <div className="snippet-session-target">
          <span className={activeSessionId ? "connected" : ""} />
          <div>
            <strong>
              {activeSessionId ? activeSessionTitle : "No connected Session"}
            </strong>
            <small>
              {activeSessionId
                ? "Insert or run in this Session only."
                : "Connect a Session before running a Snippet."}
            </small>
          </div>
        </div>

        {loading ? (
          <p className="manager-empty">Loading Snippets…</p>
        ) : snippets.length === 0 ? (
          <p className="manager-empty">
            No Snippets yet. Create a bounded remote command template.
          </p>
        ) : (
          <div className="resource-list snippet-list">
            {snippets.map((snippet) => (
              <article className="resource-card snippet-card" key={snippet.id}>
                <div className="resource-icon snippet-resource-icon">SN</div>
                <div className="resource-main">
                  <strong>{snippet.label}</strong>
                  <span>
                    {snippet.lineCount} line
                    {snippet.lineCount === 1 ? "" : "s"} ·{" "}
                    {snippet.variables.length === 0
                      ? "No variables"
                      : snippet.variables
                          .map((variable) => `{{${variable}}}`)
                          .join(", ")}
                  </span>
                </div>
                <div className="resource-actions snippet-actions">
                  <button
                    disabled={busy || !activeSessionId}
                    onClick={() => void openRunner(snippet.id, false)}
                    type="button"
                  >
                    Insert
                  </button>
                  <button
                    disabled={busy || !activeSessionId}
                    onClick={() => void openRunner(snippet.id, true)}
                    type="button"
                  >
                    Run
                  </button>
                  <button
                    disabled={busy}
                    onClick={() => void editSnippet(snippet.id)}
                    type="button"
                  >
                    Edit
                  </button>
                  <button
                    className="danger-action"
                    disabled={busy}
                    onClick={() => void removeSnippet(snippet.id)}
                    type="button"
                  >
                    Delete
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>

      {editor && (
        <div className="dialog-backdrop">
          <form
            aria-labelledby="snippet-editor-title"
            aria-modal="true"
            className="resource-dialog snippet-editor editor-form"
            onSubmit={submitEditor}
            role="dialog"
          >
            <p className="eyebrow">Encrypted repository</p>
            <h2 id="snippet-editor-title">
              {editor.snippetId ? "Edit Snippet" : "New Snippet"}
            </h2>
            <p className="editor-note">
              Use variables such as {"{{host}}"}. Do not store Passwords, Tokens
              or Private Keys in a Snippet.
            </p>
            <label>
              Label
              <input
                autoFocus
                onChange={(event) =>
                  setEditor((current) =>
                    current ? { ...current, label: event.target.value } : null,
                  )
                }
                required
                value={editor.label}
              />
            </label>
            <label>
              Command template
              <textarea
                aria-label="Snippet command template"
                onChange={(event) =>
                  setEditor((current) =>
                    current ? { ...current, body: event.target.value } : null,
                  )
                }
                required
                rows={8}
                spellCheck={false}
                value={editor.body}
              />
            </label>
            <div className="dialog-actions">
              <button
                className="secondary-button"
                disabled={busy}
                onClick={() => setEditor(null)}
                type="button"
              >
                Cancel
              </button>
              <button className="connect-button" disabled={busy} type="submit">
                {busy ? "Saving…" : "Save Snippet"}
              </button>
            </div>
          </form>
        </div>
      )}

      {runner && (
        <div className="dialog-backdrop">
          <form
            aria-labelledby="snippet-runner-title"
            aria-modal="true"
            className="resource-dialog snippet-runner editor-form"
            onSubmit={submitRunner}
            role="dialog"
          >
            <p className="eyebrow">
              {runner.appendEnter ? "Run remote command" : "Insert into PTY"}
            </p>
            <h2 id="snippet-runner-title">{runner.draft.summary.label}</h2>
            {runner.draft.summary.variables.map((variable) => (
              <label key={variable}>
                {variable}
                <input
                  autoFocus={variable === runner.draft.summary.variables[0]}
                  onChange={(event) =>
                    setRunner((current) =>
                      current
                        ? {
                            ...current,
                            variables: {
                              ...current.variables,
                              [variable]: event.target.value,
                            },
                          }
                        : null,
                    )
                  }
                  required
                  value={runner.variables[variable] ?? ""}
                />
              </label>
            ))}
            <label>
              Full preview
              <textarea
                aria-label="Rendered Snippet preview"
                readOnly
                rows={Math.min(10, Math.max(3, preview.split("\n").length + 1))}
                value={preview}
              />
            </label>
            {previewIsMultiline && (
              <label className="toggle-field multiline-confirmation">
                <input
                  checked={runner.confirmedMultiline}
                  onChange={(event) =>
                    setRunner((current) =>
                      current
                        ? {
                            ...current,
                            confirmedMultiline: event.target.checked,
                          }
                        : null,
                    )
                  }
                  type="checkbox"
                />
                I reviewed every line and want to send this multi-line command.
              </label>
            )}
            <div className="dialog-actions">
              <button
                className="secondary-button"
                disabled={busy}
                onClick={() => setRunner(null)}
                type="button"
              >
                Cancel
              </button>
              <button
                className="connect-button"
                disabled={
                  busy || (previewIsMultiline && !runner.confirmedMultiline)
                }
                type="submit"
              >
                {busy
                  ? "Sending…"
                  : runner.appendEnter
                    ? "Run in Session"
                    : "Insert in Session"}
              </button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}

function renderPreview(
  template: string,
  variables: Record<string, string>,
): string {
  let rendered = template;
  for (const [name, value] of Object.entries(variables)) {
    rendered = rendered.replaceAll(`{{${name}}}`, value);
  }
  return rendered;
}
