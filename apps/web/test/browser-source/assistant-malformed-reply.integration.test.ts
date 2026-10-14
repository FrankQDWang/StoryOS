import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const RUN = "018f0000-0000-7001-8000-000000000c71";
const MALFORMED_REPLY = "模型这次的回复格式不正确，正文没有改动。你可以重试，或继续手动写作。";
// The shape of the S4-03 live reply: the second change has a duplicated "block_id": key.
const DUPLICATED_KEY = '{"kind":"prose_change","summary":"改写两段。","changes":['
  + '{"block_id":"b1","candidate_text":"雨更大了。","explanation":"更紧凑。"},'
  + '{"block_id":"block_id":"b2","candidate_text":"渡口很静。","explanation":"更紧凑。"}]}';
const FENCED_WITHOUT_CHANGES = '```json\n{"kind":"prose_change","summary":"Shorter."}\n```';
const WITHOUT_END = '```json\n{"kind":"prose_change","summary":"Shorter.","changes":[';

it.each([
  ["a change JSON that is not valid JSON", DUPLICATED_KEY, MALFORMED_REPLY],
  ["a fenced change JSON without changes", FENCED_WITHOUT_CHANGES, MALFORMED_REPLY],
  ["a fenced change JSON without its end", WITHOUT_END, MALFORMED_REPLY],
  ["a prose Advisory", "Keep the second sentence.", "Keep the second sentence."],
])("shows %s as the author result and changes no chapter", async (_, text, result) => {
  const scope = { owner_user_id: OWNER, project_id: PROJECT };
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
    scope, message: "Help with this passage.", chapterId: CHAPTER,
    correlationId: "018f0000-0000-7001-8000-000000000c72", idempotencyKey: "018f0000-0000-7001-8000-000000000c73",
    snapshotId: "018f0000-0000-7001-8000-000000000c74", runId: RUN, conversationId: agentRunFixture.conversation_id,
  }));
  const requests: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    requests.push(`${input instanceof Request ? input.method : init?.method ?? "GET"} ${path}`);
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith(`/agent-runs/${RUN}`)) {
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUN, status: "completed",
        decision: { kind: "advisory", decision_id: "018f0000-0000-7001-8000-000000000c75", selected: true,
          text, continuation: { kind: "absent" } } });
    }
    throw new Error(`Unexpected request ${path}`);
  };
  let openedProposals = 0;
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
        onOpenedProposal: () => { openedProposals += 1; },
        scheduleRefresh: () => () => {},
      }));
    });
    for (let turn = 0; turn < 5; turn += 1) await act(async () => { await Promise.resolve(); });

    expect({
      result: host.querySelector("[data-assistant-result]")?.textContent ?? null,
      showsChangeJson: host.textContent?.includes('"kind"') ?? false,
      writes: requests.filter(request => !request.startsWith("GET ")),
      openedProposals,
    }).toEqual({ result, showsChangeJson: false, writes: [], openedProposals: 0 });
  } finally {
    await act(async () => root.unmount());
    host.remove();
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});
