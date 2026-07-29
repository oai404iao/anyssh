import { invoke, isTauri } from "@tauri-apps/api/core";
import { sendSshInput } from "./ssh-bridge";

export interface SnippetSummary {
  id: string;
  label: string;
  variables: string[];
  lineCount: number;
  updatedAt: number;
}

export interface SnippetDraft {
  summary: SnippetSummary;
  body: string;
}

export interface SnippetInput {
  label: string;
  body: string;
}

export interface SnippetUpdate extends SnippetInput {
  snippetId: string;
}

export interface RunSnippetRequest {
  sessionId: string;
  snippetId: string;
  variables: Record<string, string>;
  appendEnter: boolean;
  confirmedMultiline: boolean;
}

interface BrowserSnippetRecord extends SnippetDraft {
  createdAt: number;
}

const BROWSER_SNIPPET_FIXTURES: BrowserSnippetRecord[] = [
  browserSnippet("browser-snippet-unicode", "Unicode preview", "unicode", 1),
  browserSnippet("browser-snippet-marker", "Echo marker", "echo {{marker}}", 2),
];

let browserSnippets = cloneRecords(BROWSER_SNIPPET_FIXTURES);
let nextBrowserSnippetId = browserSnippets.length + 1;
let nextBrowserTimestamp = 10;

export async function listSnippets(): Promise<SnippetSummary[]> {
  if (!isTauri()) {
    return browserSnippets.map((snippet) => cloneSummary(snippet.summary));
  }
  return invoke<SnippetSummary[]>("snippet_list");
}

export async function getSnippet(snippetId: string): Promise<SnippetDraft> {
  if (!isTauri()) {
    const snippet = browserSnippets.find(
      (candidate) => candidate.summary.id === snippetId,
    );
    if (!snippet) throw new Error("Snippet was not found");
    return cloneDraft(snippet);
  }
  return invoke<SnippetDraft>("snippet_get", {
    request: { snippetId },
  });
}

export async function createSnippet(
  input: SnippetInput,
): Promise<SnippetSummary> {
  if (!isTauri()) {
    const record = browserSnippet(
      `browser-snippet-${nextBrowserSnippetId++}`,
      input.label,
      input.body,
      nextBrowserTimestamp++,
    );
    browserSnippets.push(record);
    return cloneSummary(record.summary);
  }
  return invoke<SnippetSummary>("snippet_create", {
    request: input,
  });
}

export async function updateSnippet(
  input: SnippetUpdate,
): Promise<SnippetSummary> {
  if (!isTauri()) {
    const index = browserSnippets.findIndex(
      (snippet) => snippet.summary.id === input.snippetId,
    );
    if (index < 0) throw new Error("Snippet was not found");
    const createdAt = browserSnippets[index]!.createdAt;
    const record = browserSnippet(
      input.snippetId,
      input.label,
      input.body,
      nextBrowserTimestamp++,
    );
    record.createdAt = createdAt;
    browserSnippets[index] = record;
    return cloneSummary(record.summary);
  }
  return invoke<SnippetSummary>("snippet_update", {
    request: input,
  });
}

export async function deleteSnippet(snippetId: string): Promise<boolean> {
  if (!isTauri()) {
    const before = browserSnippets.length;
    browserSnippets = browserSnippets.filter(
      (snippet) => snippet.summary.id !== snippetId,
    );
    return browserSnippets.length !== before;
  }
  return invoke<boolean>("snippet_delete", {
    request: { snippetId },
  });
}

export async function runSnippet(request: RunSnippetRequest): Promise<void> {
  if (isTauri()) {
    return invoke<void>("snippet_run", { request });
  }

  const snippet = browserSnippets.find(
    (candidate) => candidate.summary.id === request.snippetId,
  );
  if (!snippet) throw new Error("Snippet was not found");
  const input = renderBrowserSnippet(snippet, request.variables);
  const multiline = input.includes("\n") || input.includes("\r");
  if (multiline && !request.confirmedMultiline) {
    throw new Error("multi-line Snippet requires explicit confirmation");
  }
  await sendSshInput(
    request.sessionId,
    request.appendEnter ? `${input}\r` : input,
  );
}

export function resetBrowserSnippetsForTests(): void {
  browserSnippets = cloneRecords(BROWSER_SNIPPET_FIXTURES);
  nextBrowserSnippetId = browserSnippets.length + 1;
  nextBrowserTimestamp = 10;
}

function browserSnippet(
  id: string,
  label: string,
  body: string,
  updatedAt: number,
): BrowserSnippetRecord {
  validateBody(body);
  return {
    summary: {
      id,
      label,
      variables: parseVariables(body),
      lineCount: body.split("\n").length,
      updatedAt,
    },
    body,
    createdAt: updatedAt,
  };
}

function renderBrowserSnippet(
  snippet: BrowserSnippetRecord,
  values: Record<string, string>,
): string {
  const names = Object.keys(values).sort();
  if (
    names.length !== snippet.summary.variables.length ||
    names.some((name, index) => name !== snippet.summary.variables[index])
  ) {
    throw new Error("Snippet variables are invalid");
  }
  let rendered = snippet.body;
  for (const name of names) {
    const value = values[name]!;
    if (value.length > 4096 || value.includes("\0")) {
      throw new Error("Snippet variables are invalid");
    }
    rendered = rendered.replaceAll(`{{${name}}}`, value);
  }
  if (rendered.length > 64 * 1024) {
    throw new Error("rendered Snippet exceeds the supported size");
  }
  return rendered;
}

function parseVariables(body: string): string[] {
  const names = new Set<string>();
  let index = 0;
  while (index < body.length) {
    if (body.startsWith("{{", index)) {
      const end = body.indexOf("}}", index + 2);
      if (end < 0) throw new Error("Snippet is invalid");
      const name = body.slice(index + 2, end);
      if (!/^[A-Za-z][A-Za-z0-9_]{0,31}$/u.test(name) || name.includes("{{")) {
        throw new Error("Snippet is invalid");
      }
      names.add(name);
      if (names.size > 16) throw new Error("Snippet is invalid");
      index = end + 2;
      continue;
    }
    if (body.startsWith("}}", index)) {
      throw new Error("Snippet is invalid");
    }
    index += 1;
  }
  return [...names].sort();
}

function validateBody(body: string): void {
  const byteLength = new TextEncoder().encode(body).length;
  const containsDisallowedControl = [...body].some((character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    return (
      codePoint === 0x7f ||
      (codePoint < 0x20 &&
        codePoint !== 0x09 &&
        codePoint !== 0x0a &&
        codePoint !== 0x0d)
    );
  });
  if (byteLength === 0 || byteLength > 64 * 1024 || containsDisallowedControl) {
    throw new Error("Snippet is invalid");
  }
  parseVariables(body);
}

function cloneSummary(summary: SnippetSummary): SnippetSummary {
  return {
    ...summary,
    variables: [...summary.variables],
  };
}

function cloneDraft(snippet: BrowserSnippetRecord): SnippetDraft {
  return {
    summary: cloneSummary(snippet.summary),
    body: snippet.body,
  };
}

function cloneRecords(
  snippets: BrowserSnippetRecord[],
): BrowserSnippetRecord[] {
  return snippets.map((snippet) => ({
    ...cloneDraft(snippet),
    createdAt: snippet.createdAt,
  }));
}
