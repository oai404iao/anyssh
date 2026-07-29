import { beforeEach, describe, expect, it } from "vitest";
import {
  createSnippet,
  deleteSnippet,
  getSnippet,
  listSnippets,
  resetBrowserSnippetsForTests,
  runSnippet,
  updateSnippet,
} from "./snippet-bridge";

describe("browser preview Snippet bridge", () => {
  beforeEach(() => {
    resetBrowserSnippetsForTests();
  });

  it("lists summaries without returning the stored body", async () => {
    const created = await createSnippet({
      label: "Deploy",
      body: "printf '%s' {{target}}",
    });
    expect(created).toMatchObject({
      label: "Deploy",
      variables: ["target"],
      lineCount: 1,
    });

    const summaries = await listSnippets();
    expect(summaries).toContainEqual(created);
    expect(JSON.stringify(summaries)).not.toContain("printf");

    const draft = await getSnippet(created.id);
    expect(draft.body).toBe("printf '%s' {{target}}");
    const updated = await updateSnippet({
      snippetId: created.id,
      label: "Deploy updated",
      body: "echo {{target}}\necho complete",
    });
    expect(updated.lineCount).toBe(2);
    await expect(deleteSnippet(created.id)).resolves.toBe(true);
    await expect(getSnippet(created.id)).rejects.toThrow("not found");
  });

  it("requires exact variables and explicit multi-line confirmation", async () => {
    const created = await createSnippet({
      label: "Multi",
      body: "echo {{target}}\necho done",
    });
    await expect(
      runSnippet({
        sessionId: "browser-missing-session",
        snippetId: created.id,
        variables: {},
        appendEnter: true,
        confirmedMultiline: false,
      }),
    ).rejects.toThrow("variables");
    await expect(
      runSnippet({
        sessionId: "browser-missing-session",
        snippetId: created.id,
        variables: { target: "server" },
        appendEnter: true,
        confirmedMultiline: false,
      }),
    ).rejects.toThrow("explicit confirmation");
  });

  it("rejects malformed or control-bearing templates", async () => {
    await expect(
      createSnippet({ label: "Broken", body: "echo {{missing" }),
    ).rejects.toThrow("invalid");
    await expect(
      createSnippet({ label: "Control", body: "echo \u001b[31mred" }),
    ).rejects.toThrow("invalid");
  });
});
