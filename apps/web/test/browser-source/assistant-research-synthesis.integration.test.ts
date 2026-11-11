import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import challengeFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-project-command-challenge.json";
import createFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-agent-run.json";
import treeFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-manuscript-tree.json";
import artifactFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-research-artifact.json";
import { USE_RESEARCH_MESSAGE } from "../../src/assistant-run-outcome.ts";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const RUN = artifactFixture.artifact.lineage.run_id;
const NEXT_RUN = createFixture.effect.run_id;
const CONVERSATION = agentRunFixture.conversation_id;
const ARTIFACT = artifactFixture.artifact.research_artifact_id;
const PREDECESSOR = "018f0000-0000-7001-8000-000000000e95";

// The getAgentRun facts of a continuation whose Retrieve answered 404 and whose one successor
// settled the synthesis (ADR 0088): the retrieval keeps the unknown outcome of the predecessor.
const recovered = {
  original_result_retrieval: { kind: "present", reconciliation_id: "018f0000-0000-7001-8000-000000000e96",
    disposition: "kept_unknown", keep_reason: "unknown_result", original_model_attempt_id: PREDECESSOR,
    repeats_original_create: false, resumes_stream: false, proves_create_idempotency: false,
    supplies_decision: false, supplies_tool_call: false, advances_continuation: false,
    reservation_released: false, usage_kind: "unknown" },
  unknown_create_successor: { kind: "present", recovery_id: "018f0000-0000-7001-8000-000000000e97",
    disposition: "dispatched", predecessor_model_attempt_id: PREDECESSOR,
    successor_model_attempt_id: artifactFixture.artifact.lineage.model_attempt_id,
    model_invocation_id: "018f0000-0000-7001-8000-000000000e98", predecessor_fenced: true,
    allowance_consumed: true, predecessor_usage_kind: "reported", predecessor_reservation_released: true,
    successor_settles_predecessor: false, supplies_tool_call: false,
    advances_predecessor_continuation: false, reuses_changed_context: false },
};

it.each([
  ["a synthesis", {}],
  ["the synthesis of a continuation successor", recovered],
])("shows %s with numbered citations and a gap reason, and sends the ordinary request to use it", async (_, recovery) => {
  const scope = { owner_user_id: OWNER, project_id: PROJECT };
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
    scope, message: "Check the tide tables of 1890.", chapterId: CHAPTER,
    correlationId: "018f0000-0000-7001-8000-000000000e92", idempotencyKey: "018f0000-0000-7001-8000-000000000e93",
    snapshotId: "018f0000-0000-7001-8000-000000000e94", runId: RUN, conversationId: CONVERSATION,
  }));
  const created: Array<Record<string, unknown>> = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith("/anti-forgery-challenges")) return jsonResponse(challengeFixture);
    if (path.endsWith("/manuscript/tree")) return jsonResponse({ ...treeFixture, project_scope: scope,
      snapshot: { ...treeFixture.snapshot, project_scope: scope } });
    if (path.endsWith(`/agent-runs/${RUN}`)) {
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUN, status: "completed",
        decision: { kind: "research_synthesis", decision_id: artifactFixture.artifact.lineage.decision_id,
          selected: true, research_artifact_id: ARTIFACT, continuation: { kind: "absent" } }, ...recovery });
    }
    if (path.endsWith(`/agent-runs/${RUN}/research-sources`)) return jsonResponse({ code: "resource_unavailable" }, 404);
    if (path.endsWith(`/research-artifacts/${ARTIFACT}`)) return jsonResponse({ ...artifactFixture, project_scope: scope });
    if (path.endsWith(`/agent-runs/${NEXT_RUN}`)) return jsonResponse({ ...agentRunFixture, project_scope: scope,
      run_id: NEXT_RUN, status: "queued", decision: { kind: "absent" } });
    if (path.endsWith("/agent-runs")) {
      created.push(JSON.parse(String(init?.body)) as Record<string, unknown>);
      return jsonResponse({ ...createFixture, project_scope: scope,
        conversation_id: CONVERSATION, effect: { ...createFixture.effect, conversation_id: CONVERSATION } }, 202);
    }
    if (path.endsWith("/activity")) return new Response("", { status: 200, headers: { "content-type": "text/event-stream" } });
    throw new Error(`Unexpected request ${path}`);
  };
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  const settle = async () => {
    for (let turn = 0; turn < 40; turn += 1) await act(async () => { await Promise.resolve(); });
  };
  try {
    await act(async () => {
      root.render(createElement(WritingAssistantPanel, {
        collapsed: false,
        context: { scope, chapterId: CHAPTER, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto },
        scheduleRefresh: () => () => {},
      }));
    });
    await settle();

    const time = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });
    const shown = host.querySelector("[data-assistant-research-artifact]");
    const { citations } = artifactFixture.artifact;
    const identifiers = [RUN, ARTIFACT, artifactFixture.artifact.content_digest,
      citations[0]!.snapshot!.source_snapshot_id, citations[1]!.gap!.source_gap_id, citations[0]!.snapshot!.tool_call_id];
    const view = {
      lines: [...shown?.querySelectorAll("p, li") ?? []].map(item => item.textContent),
      showsIdentifier: identifiers.some(id => host.textContent?.includes(id) ?? false),
    };
    await act(async () => { host.querySelector<HTMLButtonElement>("[data-assistant-use-research]")!.click(); });
    await settle();
    const input = created[0]?.create_agent_run_input as Record<string, unknown> | undefined;

    expect({ view, sent: created.length, conversation: input?.conversation, message: input?.author_message,
      target: input?.working_target }).toEqual({
      view: {
        lines: [
          "High water was at 10.42. The port log is not available.",
          "High water at Liverpool was at 10.42. [1]",
          "The port log could not be read. [2]",
          `[1] Liverpool tide table 1890（获取于 ${time.format(new Date("2026-11-11T03:00:00.000Z"))}）`,
          "[2] https://example.org/port：无法获取（读取失败）",
        ],
        showsIdentifier: false,
      },
      sent: 1,
      conversation: { kind: "existing", conversation_id: CONVERSATION },
      message: { text: USE_RESEARCH_MESSAGE },
      target: { kind: "current_chapter", chapter_id: CHAPTER },
    });
  } finally {
    await act(async () => root.unmount());
    host.remove();
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});
