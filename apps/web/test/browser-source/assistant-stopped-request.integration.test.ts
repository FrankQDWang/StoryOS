import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const RUN = "018f0000-0000-7001-8000-000000000c61";
const ATTEMPT = "018f0000-0000-7001-8000-000000000c62";
const NO_RESULT = "没有收到这次的结果，正文没有改动。请开始新对话重试。";

const retrieval = {
  kind: "present", reconciliation_id: "018f0000-0000-7001-8000-000000000c63", disposition: "kept_unknown",
  keep_reason: "unknown_result", original_model_attempt_id: ATTEMPT, repeats_original_create: false,
  resumes_stream: false, proves_create_idempotency: false, supplies_decision: false, supplies_tool_call: false,
  advances_continuation: false, reservation_released: false, usage_kind: "unknown",
};
const successor = (disposition: string) => ({
  kind: "present", recovery_id: "018f0000-0000-7001-8000-000000000c64", disposition,
  predecessor_model_attempt_id: ATTEMPT, model_invocation_id: "018f0000-0000-7001-8000-000000000c65",
  predecessor_fenced: true, allowance_consumed: true, predecessor_usage_kind: "unknown",
  predecessor_reservation_released: false, successor_settles_predecessor: false, supplies_tool_call: false,
  advances_predecessor_continuation: false, reuses_changed_context: false,
});

it.each([
  ["an unknown outcome that waits", { status: "waiting", original_result_retrieval: retrieval }, NO_RESULT],
  ["a paused successor", { status: "paused", original_result_retrieval: retrieval,
    unknown_create_successor: successor("paused") }, NO_RESULT],
  ["a successor with no result", { status: "completed", original_result_retrieval: retrieval,
    unknown_create_successor: successor("dispatched") }, NO_RESULT],
  ["a request after one with no result", { status: "refused",
    decision: { kind: "execution_refused", capability: "expiry_rebuild_unknown_create" } },
  "上一条请求还没有结果，请开始新对话。"],
])("shows %s as a stopped request with one sentence and a new conversation", async (_, run, sentence) => {
  const scope = { owner_user_id: OWNER, project_id: PROJECT };
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
    scope, message: "Help with this passage.", chapterId: CHAPTER,
    correlationId: "018f0000-0000-7001-8000-000000000c66", idempotencyKey: "018f0000-0000-7001-8000-000000000c67",
    snapshotId: "018f0000-0000-7001-8000-000000000c68", runId: RUN, conversationId: agentRunFixture.conversation_id,
  }));
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith(`/agent-runs/${RUN}`)) {
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUN,
        decision: { kind: "absent" }, original_result_retrieval: { kind: "absent" },
        unknown_create_successor: { kind: "absent" }, ...run });
    }
    throw new Error(`Unexpected request ${path}`);
  };
  // The refreshes that are still scheduled. A stopped request leaves none.
  let pendingRefreshes = 0;
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
        scheduleRefresh: () => {
          pendingRefreshes += 1;
          return () => { pendingRefreshes -= 1; };
        },
      }));
    });
    for (let turn = 0; turn < 5; turn += 1) await act(async () => { await Promise.resolve(); });
    const newConversation = host.querySelector<HTMLButtonElement>("[data-assistant-new-conversation]");

    expect({
      result: host.querySelector("[data-assistant-result]")?.textContent ?? null,
      runStatus: host.querySelector("[data-assistant-run-status]")?.textContent ?? null,
      newConversationEnabled: newConversation?.disabled === false,
      sendLabel: host.querySelector(".send-button")?.getAttribute("aria-label") ?? null,
      pendingRefreshes,
    }).toEqual({
      result: sentence,
      runStatus: null,
      newConversationEnabled: true,
      sendLabel: "发送",
      pendingRefreshes: 0,
    });
  } finally {
    await act(async () => root.unmount());
    host.remove();
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});
