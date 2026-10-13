import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const RUN = "018f0000-0000-7001-8000-000000000c51";

it.each([
  ["disclosure_outside_grant", "这次请求超出了本项目允许发送给 AI 的范围，没有发送任何内容。你可以继续手动写作。"],
  ["conversation_too_long", "这段对话已经太长，请开始一个新对话。你可以继续手动写作。"],
])("shows one plain sentence and no decision for a refused request: %s", async (capability, sentence) => {
  const scope = { owner_user_id: OWNER, project_id: PROJECT };
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
    scope, message: "Help with this passage.", chapterId: CHAPTER,
    correlationId: "018f0000-0000-7001-8000-000000000c52", idempotencyKey: "018f0000-0000-7001-8000-000000000c53",
    snapshotId: "018f0000-0000-7001-8000-000000000c54", runId: RUN, conversationId: agentRunFixture.conversation_id,
  }));
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith(`/agent-runs/${RUN}`)) {
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUN, status: "refused",
        decision: { kind: "execution_refused", capability } });
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
      buttons: [...host.querySelectorAll("button")].map(button => button.textContent).filter(text => /允许|批准/.test(text ?? "")),
    }).toEqual({
      result: sentence,
      buttons: [],
    });
  } finally {
    await act(async () => root.unmount());
    host.remove();
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});
