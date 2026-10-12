import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const RUN = "018f0000-0000-7001-8000-000000000c41";

it("reads a queued Run again after a failed read until it settles and shows the result", async () => {
  const scope = { owner_user_id: OWNER, project_id: PROJECT };
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
    scope, message: "Help with this passage.", chapterId: CHAPTER,
    correlationId: "018f0000-0000-7001-8000-000000000c42", idempotencyKey: "018f0000-0000-7001-8000-000000000c43",
    snapshotId: "018f0000-0000-7001-8000-000000000c44", runId: RUN, conversationId: agentRunFixture.conversation_id,
  }));
  const settled = { kind: "advisory", decision_id: "018f0000-0000-7001-8000-000000000c45", selected: true,
    text: "Keep the second sentence.", continuation: { kind: "absent" } };
  const statuses = ["queued", "unreadable", "completed"] as const;
  let reads = 0;
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith(`/agent-runs/${RUN}`)) {
      const status = statuses[Math.min(reads, statuses.length - 1)]!;
      reads += 1;
      if (status === "unreadable") return jsonResponse({ code: "resource_unavailable" }, 503);
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUN, status,
        decision: status === "completed" ? settled : { kind: "absent" } });
    }
    throw new Error(`Unexpected request ${path}`);
  };
  const scheduled: Array<() => void> = [];
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  const settle = async () => {
    for (let turn = 0; turn < 5; turn += 1) await act(async () => { await Promise.resolve(); });
  };
  const fire = async () => {
    const refresh = scheduled.shift();
    expect(refresh).toBeDefined();
    await act(async () => { refresh!(); });
    await settle();
  };
  const panelState = () => ({
    reads,
    pending: scheduled.length,
    status: host.querySelector("[data-assistant-run-status]")?.textContent ?? null,
    inspect: host.querySelector("[data-assistant-inspect]") !== null,
    result: host.querySelector("[data-assistant-result]")?.textContent ?? null,
  });
  try {
    await act(async () => {
      root.render(createElement(WritingAssistantPanel, {
        collapsed: false,
        context: { scope, chapterId: CHAPTER, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto },
        scheduleRefresh: (refresh) => {
          scheduled.push(refresh);
          return () => {
            const index = scheduled.indexOf(refresh);
            if (index >= 0) scheduled.splice(index, 1);
          };
        },
      }));
    });
    await settle();
    const first = panelState();
    await fire();
    const second = panelState();
    await fire();

    expect([first, second, panelState()]).toEqual([
      { reads: 1, pending: 1, status: "等待开始", inspect: true, result: "结果尚未生成。" },
      { reads: 2, pending: 1, status: "等待开始", inspect: true, result: "结果尚未生成。" },
      { reads: 3, pending: 0, status: null, inspect: false, result: "Keep the second sentence." },
    ]);
  } finally {
    await act(async () => root.unmount());
    host.remove();
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});
