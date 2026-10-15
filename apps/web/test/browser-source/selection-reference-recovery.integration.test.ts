import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import type { ChapterTextReference } from "../../src/selection-reference.ts";
import { addDraftReferences, draftEntries } from "../../src/selection-reference-draft.ts";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { BLOCK, CHAPTER, OWNER, PROJECT, REVISION, jsonResponse } from "./scenario.ts";

const RUN = "018f0000-0000-7001-8000-000000000c71";
const CORRELATION = "018f0000-0000-7001-8000-000000000c72";

function reference(start: number, end: number): ChapterTextReference {
  return { kind: "chapter_text", chapter_id: CHAPTER, base_authoritative_revision_id: REVISION,
    coordinate_profile: "storyos.editor.utf16-code-unit.v1",
    start: { manuscript_block_id: BLOCK, offset: start }, end: { manuscript_block_id: BLOCK, offset: end } };
}

it("removes the sent references when a recovery finds the Run after a lost acknowledgement", async () => {
  const scope = { owner_user_id: OWNER, project_id: PROJECT };
  const label = { chapter: "第 1 章 渡口", part: { kind: "paragraphs" as const, first: 1, last: 1 } };
  const [sent] = addDraftReferences(scope, [{ reference: reference(0, 2), label, moved: false }])!;
  // The author added this reference while the request waited for its acknowledgement.
  const [added] = addDraftReferences(scope, [{ reference: reference(3, 5), label, moved: false }])!;
  // The request went to the Host, and its acknowledgement did not come back, so the reference has no Run.
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
    scope, message: "Help with this passage.", chapterId: CHAPTER, correlationId: CORRELATION,
    idempotencyKey: "018f0000-0000-7001-8000-000000000c73", snapshotId: "018f0000-0000-7001-8000-000000000c74",
    references: [reference(0, 2)], referenceLabels: ["第 1 章 渡口（第 1 段）"], referenceKeys: [sent],
  }));
  const created = { event_kind: "agent_run_created", correlation_id: CORRELATION, project_scope: scope,
    payload: { run_id: RUN } };
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith("/activity")) {
      return new Response(`data: ${JSON.stringify(created)}\n\n`, {
        status: 200, headers: { "content-type": "text/event-stream" } });
    }
    if (path.endsWith(`/agent-runs/${RUN}`)) {
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUN, status: "completed",
        decision: { kind: "absent" }, original_result_retrieval: { kind: "absent" },
        unknown_create_successor: { kind: "absent" } });
    }
    throw new Error(`Unexpected request ${path}`);
  };
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => {
      root.render(createElement(WritingAssistantPanel, {
        collapsed: false,
        context: { scope, chapterId: CHAPTER, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto },
        scheduleRefresh: () => () => {},
      }));
    });
    for (let turn = 0; turn < 5; turn += 1) await act(async () => { await Promise.resolve(); });

    expect({
      draft: draftEntries(scope).map((entry) => entry.key),
      composer: host.querySelectorAll("[data-selection-reference-label]").length,
      history: [...host.querySelectorAll("[data-sent-reference-label]")].map((item) => item.textContent),
    }).toEqual({ draft: [added], composer: 1, history: ["第 1 章 渡口（第 1 段）"] });
  } finally {
    await act(async () => root.unmount());
    host.remove();
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});
