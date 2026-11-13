import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import artifactFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-research-artifact.json";
import { RESEARCH_SYNTHESIS_SENTENCE } from "../../src/assistant-run-outcome.ts";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { BLOCK, CHAPTER, OWNER, PROJECT, REVISION, jsonResponse } from "./scenario.ts";

const scope = { owner_user_id: OWNER, project_id: PROJECT };
const CONVERSATION = agentRunFixture.conversation_id;
const ARTIFACT = artifactFixture.artifact.research_artifact_id;
const PROPOSAL = "018f0000-0000-7001-8000-000000000d21";
const OPERATION = "018f0000-0000-7001-8000-000000000d22";
const PROPOSAL_DECISION = "018f0000-0000-7001-8000-000000000d23";
const ANSWER = "I changed the first passage and checked the tide tables.";
const PROPOSAL_SENTENCE = "这次修改涉及 1 个章节、1 处文字，具体建议在下方。";
const LOCATION = "· 第1处修改↗Keep the narrator voice.";

const changeLocation = {
  chapter_id: CHAPTER, manuscript_block_id: BLOCK, base_authoritative_revision_id: REVISION,
  candidate_text: "The tide rose at ten.", explanation: "Keep the narrator voice.",
  outcome: { kind: "opened", proposal_id: PROPOSAL, operation_id: OPERATION,
    revision_id: "018f0000-0000-7001-8000-000000000d24", validation_receipt_id: "018f0000-0000-7001-8000-000000000d25" },
  current: null,
};
const proposal = { kind: "proposal", decision_id: PROPOSAL_DECISION, proposal_id: PROPOSAL, locations: [changeLocation] };
const research = { kind: "research_artifact", decision_id: artifactFixture.artifact.lineage.decision_id,
  research_artifact_id: ARTIFACT };
const answer = { kind: "answer", decision_id: "018f0000-0000-7001-8000-000000000d26", text: ANSWER };

/** A completed Run with `results`, as the Run read gives it: the derived decision keeps the Proposal. */
function completedRun(runId: string, results: readonly object[]) {
  return { ...agentRunFixture, project_scope: scope, run_id: runId, status: "completed", results,
    decision: { kind: "prose_change", decision_id: PROPOSAL_DECISION, selected: true, text: "Kept the voice.",
      producer_input: "[]", locations: [changeLocation], continuation: { kind: "absent" }, authoritative: false,
      opened_proposal: { kind: "present", proposal_id: PROPOSAL } } };
}

const ALL = completedRun(artifactFixture.artifact.lineage.run_id, [research, proposal, answer]);
const ARTIFACT_LINES = [
  "High water was at 10.42. The port log is not available.",
  "High water at Liverpool was at 10.42. [1]",
  "The port log could not be read. [2]",
  `[1] Liverpool tide table 1890（获取于 ${new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium",
    timeStyle: "short" }).format(new Date("2026-11-11T03:00:00.000Z"))}）`,
  "[2] https://example.org/port：无法获取（读取失败）",
];

/** Renders the panel for the stored request and returns its host, the opened Proposals, and a cleanup. */
async function renderPanel(runs: readonly ReturnType<typeof completedRun>[], stored: object) {
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({ scope, chapterId: CHAPTER,
    conversationId: CONVERSATION, ...stored }));
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    const run = runs.find(item => path.endsWith(`/agent-runs/${item.run_id}`));
    if (run !== undefined) return jsonResponse(run);
    if (path.endsWith("/research-sources")) return jsonResponse({ code: "resource_unavailable" }, 404);
    if (path.endsWith(`/research-artifacts/${ARTIFACT}`)) return jsonResponse({ ...artifactFixture, project_scope: scope });
    throw new Error(`Unexpected request ${path}`);
  };
  const opened: string[] = [];
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  await act(async () => {
    root.render(createElement(WritingAssistantPanel, {
      collapsed: false,
      context: { scope, chapterId: CHAPTER, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto },
      onOpenedProposal: (locator) => { opened.push(locator.proposalId); },
      scheduleRefresh: () => () => {},
    }));
  });
  for (let turn = 0; turn < 40; turn += 1) await act(async () => { await Promise.resolve(); });
  return { host, opened, remove: async () => { await act(async () => root.unmount()); host.remove(); } };
}

/** The result lines of one exchange in their order, with the controls that go with them. */
function shownResults(section: Element | null) {
  return [...section?.querySelectorAll(".assistant-result, [data-assistant-research-artifact] p, "
    + "[data-assistant-research-artifact] li, [data-assistant-use-research], [data-proposal-location]") ?? []]
    .map(item => item.textContent);
}

async function inActEnvironment(body: () => Promise<void>) {
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await body();
  } finally {
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
}

it.each([
  ["a Research Artifact, a Proposal, and a final answer text", ALL,
    [RESEARCH_SYNTHESIS_SENTENCE, ...ARTIFACT_LINES, "用到故事里", PROPOSAL_SENTENCE, LOCATION, ANSWER]],
  ["a Proposal and a final answer text", completedRun("018f0000-0000-7001-8000-000000000d31", [proposal, answer]),
    [PROPOSAL_SENTENCE, LOCATION, ANSWER]],
])("shows each result of a Run with %s in the order of the model", async (_, run, expected) => {
  await inActEnvironment(async () => {
    const panel = await renderPanel([run], { message: "Check the tides and revise the passage.",
      correlationId: "018f0000-0000-7001-8000-000000000d11", idempotencyKey: "018f0000-0000-7001-8000-000000000d12",
      snapshotId: "018f0000-0000-7001-8000-000000000d13", runId: run.run_id });
    try {
      expect({ results: shownResults(panel.host.querySelector("[data-assistant-current-run]")), opened: panel.opened })
        .toEqual({ results: expected, opened: [PROPOSAL] });
    } finally {
      await panel.remove();
    }
  });
});

it("shows the same results in the history turn of the Conversation", async () => {
  const current = { ...agentRunFixture, project_scope: scope, run_id: "018f0000-0000-7001-8000-000000000d41",
    status: "queued", decision: { kind: "absent" } } as unknown as ReturnType<typeof completedRun>;
  await inActEnvironment(async () => {
    const panel = await renderPanel([ALL, current], { message: "Use this in the story.",
      correlationId: "018f0000-0000-7001-8000-000000000d14", idempotencyKey: "018f0000-0000-7001-8000-000000000d15",
      snapshotId: "018f0000-0000-7001-8000-000000000d16", runId: current.run_id,
      history: [{ requestId: "018f0000-0000-7001-8000-000000000d17", chapterId: CHAPTER, conversationId: CONVERSATION,
        runId: ALL.run_id, run: ALL, message: "Check the tides and revise the passage.", status: "completed",
        result: PROPOSAL_SENTENCE }] });
    try {
      // The button "用到故事里" stays with the current Run (#1427).
      expect(shownResults(panel.host.querySelector(`[data-assistant-history-run="${ALL.run_id}"]`)))
        .toEqual([RESEARCH_SYNTHESIS_SENTENCE, ...ARTIFACT_LINES, PROPOSAL_SENTENCE, LOCATION, ANSWER]);
    } finally {
      await panel.remove();
    }
  });
});
