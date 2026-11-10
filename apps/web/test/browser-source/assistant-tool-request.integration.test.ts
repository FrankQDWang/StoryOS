import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const RUN = "018f0000-0000-7001-8000-000000000c81";
const DECISION = "018f0000-0000-7001-8000-000000000c85";

it("shows one line for each research call of a function request and no identifier", async () => {
  const scope = { owner_user_id: OWNER, project_id: PROJECT };
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
    scope, message: "Check the tide tables of 1890.", chapterId: CHAPTER,
    correlationId: "018f0000-0000-7001-8000-000000000c82", idempotencyKey: "018f0000-0000-7001-8000-000000000c83",
    snapshotId: "018f0000-0000-7001-8000-000000000c84", runId: RUN, conversationId: agentRunFixture.conversation_id,
  }));
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith(`/agent-runs/${RUN}`)) {
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUN, status: "completed",
        decision: { kind: "tool_request", decision_id: DECISION, calls: [
          { kind: "search", query: "tide tables of 1890", outcome: "settled" },
          { kind: "page_read", urls: ["https://e.org/tides", "https://e.org/port"], outcome: "settled" },
          { kind: "search", query: "harbour log", outcome: "failed" },
        ] } });
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
      result: host.querySelector("[data-assistant-result]")?.textContent ?? null,
      showsIdentifier: [RUN, DECISION].some(id => host.textContent?.includes(id) ?? false),
    }).toEqual({
      result: "已联网搜索：tide tables of 1890\n已阅读：https://e.org/tides、https://e.org/port\n有一次联网检索没有完成。",
      showsIdentifier: false,
    });
  } finally {
    await act(async () => root.unmount());
    host.remove();
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});
