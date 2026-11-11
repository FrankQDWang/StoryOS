import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import artifactFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-research-artifact.json";
import sourcesFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-research-sources.json";
import { RESEARCH_SYNTHESIS_SENTENCE } from "../../src/assistant-run-outcome.ts";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const scope = { owner_user_id: OWNER, project_id: PROJECT };
const CONVERSATION = agentRunFixture.conversation_id;
const time = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });

/** A settled research synthesis Run of the Conversation. */
function synthesisRun(runId: string, researchArtifactId: string) {
  return { ...agentRunFixture, project_scope: scope, run_id: runId, status: "completed",
    decision: { kind: "research_synthesis", decision_id: artifactFixture.artifact.lineage.decision_id,
      selected: true, research_artifact_id: researchArtifactId, continuation: { kind: "absent" } } };
}

/** Stores a history exchange of the settled Run `earlier` before the current Run `current`. */
function storeConversation(earlier: ReturnType<typeof synthesisRun>, current: string, suffix: string) {
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
    scope, message: "Check the ferry times of 1890.", chapterId: CHAPTER,
    correlationId: `018f0000-0000-7001-8000-000000000${suffix}1`, idempotencyKey: `018f0000-0000-7001-8000-000000000${suffix}2`,
    snapshotId: `018f0000-0000-7001-8000-000000000${suffix}3`, runId: current, conversationId: CONVERSATION,
    history: [{ requestId: `018f0000-0000-7001-8000-000000000${suffix}4`, chapterId: CHAPTER, conversationId: CONVERSATION,
      runId: earlier.run_id, run: earlier, message: "Check the tide tables of 1890.", status: "completed",
      result: RESEARCH_SYNTHESIS_SENTENCE }],
  }));
}

/** Renders the panel, and returns a function that renders it again and one that removes it. */
async function renderPanel(fetchImpl: typeof fetch) {
  const host = document.createElement("div");
  document.body.append(host);
  let root = createRoot(host);
  const panel = () => createElement(WritingAssistantPanel, {
    collapsed: false,
    context: { scope, chapterId: CHAPTER, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto },
    scheduleRefresh: () => () => {},
  });
  const settle = async () => {
    for (let turn = 0; turn < 40; turn += 1) await act(async () => { await Promise.resolve(); });
  };
  await act(async () => root.render(panel()));
  await settle();
  return {
    host,
    rerender: async () => {
      await act(async () => root.render(panel()));
      await settle();
    },
    remount: async () => {
      await act(async () => root.unmount());
      root = createRoot(host);
      await act(async () => root.render(panel()));
      await settle();
    },
    remove: async () => {
      await act(async () => root.unmount());
      host.remove();
    },
  };
}

function lines(section: Element | null) {
  return [...section?.querySelectorAll("p, li") ?? []].map(item => item.textContent);
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

it("keeps the sources, claims, and citations of an earlier research turn after a second research Run, and reads them once", async () => {
  const first = synthesisRun("018f0000-0000-7001-8000-000000000f01", "018f0000-0000-7001-8000-000000000f02");
  const second = synthesisRun("018f0000-0000-7001-8000-000000000f03", "018f0000-0000-7001-8000-000000000f04");
  storeConversation(first, second.run_id, "f1");
  const [snapshot] = artifactFixture.artifact.citations;
  const artifacts = new Map([
    [first.decision.research_artifact_id, { ...artifactFixture.artifact, research_artifact_id: first.decision.research_artifact_id,
      lineage: { ...artifactFixture.artifact.lineage, run_id: first.run_id } }],
    [second.decision.research_artifact_id, { ...artifactFixture.artifact, research_artifact_id: second.decision.research_artifact_id,
      lineage: { ...artifactFixture.artifact.lineage, run_id: second.run_id }, summary: "The ferry left after high water.",
      claims: [{ text: "The ferry left at 11.00.", citations: [1] }], citations: [snapshot] }],
  ]);
  const reads = new Map<string, number>();
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    reads.set(path, (reads.get(path) ?? 0) + 1);
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    for (const run of [first, second]) {
      if (path.endsWith(`/agent-runs/${run.run_id}`)) return jsonResponse(run);
      if (path.endsWith(`/agent-runs/${run.run_id}/research-sources`)) return jsonResponse({ ...sourcesFixture,
        project_scope: scope, run_id: run.run_id, ...(run === second ? { gaps: [], unknown_calls: [] } : {}) });
      const artifact = artifacts.get(run.decision.research_artifact_id)!;
      if (path.endsWith(`/research-artifacts/${artifact.research_artifact_id}`)) {
        return jsonResponse({ ...artifactFixture, project_scope: scope, artifact });
      }
    }
    throw new Error(`Unexpected request ${path}`);
  };

  await inActEnvironment(async () => {
    const panel = await renderPanel(fetchImpl);
    try {
      const history = () => panel.host.querySelector(`[data-assistant-history-run="${first.run_id}"]`);
      const shown = () => ({
        history: lines(history()),
        historyUses: history()?.querySelectorAll("[data-assistant-use-research]").length,
        current: lines(panel.host.querySelector("[data-assistant-current-run]")),
        currentUses: panel.host.querySelectorAll("[data-assistant-current-run] [data-assistant-use-research]").length,
      });
      const before = shown();
      await panel.rerender();
      await panel.remount();
      const researchReads = Object.fromEntries([...reads].filter(([path]) => path.includes("/research-")));

      const source = `Liverpool tide table 1890（获取于 ${time.format(new Date("2026-11-10T03:00:00.000Z"))}）`;
      const cited = `[1] Liverpool tide table 1890（获取于 ${time.format(new Date("2026-11-11T03:00:00.000Z"))}）`;
      const runs = `/api/v1/projects/${PROJECT}/agent-runs`;
      const artifactPath = `/api/v1/projects/${PROJECT}/research-artifacts`;
      expect({ before, after: shown(), researchReads }).toEqual({
        before: {
          history: [
            "Check the tide tables of 1890.",
            RESEARCH_SYNTHESIS_SENTENCE,
            source,
            "https://example.org/port：无法获取（读取失败）",
            "可能已联网搜索，但没有收到结果：harbour log of 1890",
            "High water was at 10.42. The port log is not available.",
            "High water at Liverpool was at 10.42. [1]",
            "The port log could not be read. [2]",
            cited,
            "[2] https://example.org/port：无法获取（读取失败）",
          ],
          historyUses: 0,
          current: [
            "Check the ferry times of 1890.",
            RESEARCH_SYNTHESIS_SENTENCE,
            source,
            "The ferry left after high water.",
            "The ferry left at 11.00. [1]",
            cited,
          ],
          currentUses: 1,
        },
        after: before,
        researchReads: {
          [`${runs}/${first.run_id}/research-sources`]: 1,
          [`${artifactPath}/${first.decision.research_artifact_id}`]: 1,
          [`${runs}/${second.run_id}/research-sources`]: 1,
          [`${artifactPath}/${second.decision.research_artifact_id}`]: 1,
        },
      });
    } finally {
      await panel.remove();
    }
  });
});

it("shows only the result sentence for an earlier research turn whose Artifact read fails", async () => {
  const earlier = synthesisRun("018f0000-0000-7001-8000-000000000f05", "018f0000-0000-7001-8000-000000000f06");
  const current = { ...agentRunFixture, project_scope: scope, run_id: "018f0000-0000-7001-8000-000000000f07",
    status: "completed", decision: { kind: "advisory", decision_id: "018f0000-0000-7001-8000-000000000f08",
      selected: true, text: "The tide tables are in the port archive.", continuation: { kind: "absent" } } };
  storeConversation(earlier, current.run_id, "f2");
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith(`/agent-runs/${current.run_id}`)) return jsonResponse(current);
    if (path.includes("/research-")) return jsonResponse({ code: "resource_unavailable" }, 404);
    throw new Error(`Unexpected request ${path}`);
  };

  await inActEnvironment(async () => {
    const panel = await renderPanel(fetchImpl);
    try {
      const history = panel.host.querySelector(`[data-assistant-history-run="${earlier.run_id}"]`);
      expect({
        history: lines(history),
        research: history?.querySelectorAll("[data-assistant-research-sources], [data-assistant-research-artifact]").length,
      }).toEqual({
        history: ["Check the tide tables of 1890.", RESEARCH_SYNTHESIS_SENTENCE],
        research: 0,
      });
    } finally {
      await panel.remove();
    }
  });
});
