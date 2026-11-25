import { expect, it } from "vitest";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";

import { openEditorWorkspace } from "../../src/editor-session.ts";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import { ManuscriptEditor } from "../../src/manuscript-editor.tsx";
import { applyImeComposition, applyTrustedInput } from "../support/browser-command-client.ts";
import {
  OWNER,
  PROJECT,
  REVISION,
  SESSION,
  chapterRevision,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
  replaceBlockUnit,
  requireEditorReady,
} from "./scenario.ts";

// One block with line break characters, as an accepted suggestion of three paragraphs makes it (#1894).
const BODY = "One\nTwo\nThree";

async function sha256Hex(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

it.each([
  {
    label: "typing",
    input: () => applyTrustedInput({ operation: "insert_text", text: "!" }),
    primitives: replaceBlockUnit(BODY.length, BODY.length, "!").normalized_primitives,
    shown: `${BODY}!`,
  },
  {
    label: "a deletion",
    input: () => applyTrustedInput({ operation: "backspace" }),
    primitives: replaceBlockUnit(BODY.length - 1, BODY.length, "").normalized_primitives,
    shown: BODY.slice(0, -1),
  },
  {
    label: "an IME confirmation",
    input: async () => {
      await applyImeComposition({ text: "中", replacementStart: BODY.length, replacementEnd: BODY.length,
        selectionStart: 1, selectionEnd: 1 });
      await applyTrustedInput({ operation: "insert_text", text: "中文" });
    },
    primitives: replaceBlockUnit(BODY.length, BODY.length, "中文").normalized_primitives,
    shown: `${BODY}中文`,
  },
])("keeps each line break character of the block after $label", async ({ input, primitives, shown }) => {
  const base = createBrowserScenario();
  const revision = chapterRevision(REVISION, BODY);
  const scenario = {
    ...base,
    chapter: { ...base.chapter, chapter: { ...base.chapter.chapter, current_revision: revision } },
    session: { ...base.session, base_snapshot: { ...base.session.base_snapshot, materialized_revision: revision,
      materialized_payload_digest: { ...base.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: await sha256Hex(BODY) } } },
  };
  const projectPath = `/api/v1/projects/${PROJECT}`;
  const sent: unknown[] = [];
  const unexpected: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const request = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    if (request === `POST ${projectPath}/anti-forgery-challenges`) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (request === `POST ${projectPath}/editor-sessions`) return jsonResponse(scenario.session);
    if (request === `GET ${projectPath}/editor-sessions/${SESSION}`) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (request === `POST ${projectPath}/manuscript/author-edits`) {
      sent.push(JSON.parse(String(init?.body)));
      return new Promise<Response>(() => {});
    }
    unexpected.push(request);
    throw new Error(`unexpected request ${request}`);
  };
  await deleteJournal(scenario.journalName);
  const workspace = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
    chapter: scenario.chapter, profile: scenario.profile, fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  requireEditorReady(workspace);
  let failure: unknown;
  const writing = createEditorSessionWritingController({ workspace, baseUrl: location.origin, fetchImpl,
    onFailure: (error) => { failure = error; } });
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => {
      root.render(createElement(ManuscriptEditor, { blocks: workspace.openedProjection.blocks, editable: true, writing }));
    });
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    expect(surface.querySelector("p")!.textContent).toBe(BODY);
    surface.focus();
    const text = surface.querySelector("p")!.firstChild!;
    window.getSelection()!.setBaseAndExtent(text, BODY.length, text, BODY.length);
    await input();
    await expect.poll(() => surface.querySelector("p")!.textContent).toBe(shown);
    void writing.flush();
    await expect.poll(() => sent.length).toBe(1);
    expect((sent[0] as { author_edit_units: { normalized_primitives: unknown }[] }).author_edit_units
      .map((unit) => unit.normalized_primitives)).toEqual([primitives]);
    expect(unexpected).toEqual([]);
    expect(failure).toBeUndefined();
  } finally {
    await act(async () => { root.unmount(); });
    writing.close();
    host.remove();
    workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});
