import { act, createElement } from "react";
import { afterEach, expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import conversationFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-conversation.json";
import listFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-project-conversations.json";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import { openPanelHost, seedProseRequest } from "./assistant-panel-harness.ts";
import { BLOCK, CHAPTER, OWNER, PROJECT, REVISION, jsonResponse } from "./scenario.ts";

// #1936: a Conversation switch shows the author messages at once and reads the Runs and the Proposals together.
const scope = { owner_user_id: OWNER, project_id: PROJECT };
const PREFIX = `/api/v1/projects/${PROJECT}`;
const id = (suffix: string) => `018f0000-0000-7001-8000-000000000${suffix}`;
const [A, B, C, P] = [id("e61"), id("e62"), id("e63"), id("e64")];
const [PROPOSAL_1, PROPOSAL_2] = [id("e71"), id("e72")];
const LOADING = "正在读取结果…";
const OPEN_FAILED = "无法打开这段对话，请稍后再试。";

type StoredRun = { runId: string; conversationId: string; message: string; decision?: object };
const run = (suffix: string, conversationId: string, message: string): StoredRun =>
  ({ runId: id(suffix), conversationId, message });
const STORED = run("e65", B, "B first");
const RUNS = [run("e66", A, "A first"), run("e67", A, "A second"), run("e68", A, "A third"),
  run("e69", C, "C first")];
const revised = (proposalId: string, suffix: string) => ({
  chapter_id: CHAPTER, manuscript_block_id: BLOCK, base_authoritative_revision_id: REVISION,
  candidate_text: `Candidate ${suffix}`, explanation: `Reason ${suffix}`, current: null,
  outcome: { kind: "revised", proposal_id: proposalId, operation_id: id(`e7${suffix}`), revision_id: id(`e8${suffix}`),
    prior_revision_id: id(`e9${suffix}`), validation_receipt_id: id(`ea${suffix}`) } });
const PROSE = { ...run("e6a", P, "P first"), decision: { kind: "prose_change", decision_id: id("e6b"), selected: true,
  text: "Two changes.", producer_input: "[]", locations: [revised(PROPOSAL_1, "3"), revised(PROPOSAL_2, "4")],
  continuation: { kind: "absent" }, authoritative: false, opened_proposal: { kind: "absent" } } };
const ALL_RUNS = [STORED, ...RUNS, PROSE];
const runRoute = (stored: StoredRun) => `GET /agent-runs/${stored.runId}`;
const [A1, A2, A3, C1] = RUNS.map(runRoute) as [string, string, string, string];
const [PROPOSAL_READ_1, PROPOSAL_READ_2] = [PROPOSAL_1, PROPOSAL_2]
  .map((proposalId) => `GET /proposals/${proposalId}`) as [string, string];

let cleanup: Array<() => Promise<void>> = [];
afterEach(async () => {
  for (const step of cleanup) await step();
  cleanup = [];
});

/** One request to the fake Server. It is answered when the panel read the whole response body. */
type Request = { request: string; held: boolean; answered: boolean };

/**
 * Opens the panel on the Run of Conversation B on a fake Server. The fake serves only the requests of the panel and of
 * `conversations`, with the Project in the path. A held request answers only after `release`.
 */
async function openPanel(conversations: readonly string[], answers: Record<string, () => Response> = {}) {
  seedProseRequest({ scope, message: STORED.message, chapterId: CHAPTER, correlationId: id("e6c"),
    idempotencyKey: id("e6d"), snapshotId: id("e6e"), runId: STORED.runId, conversationId: B });
  const runRead = (stored: StoredRun) => jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: stored.runId,
    conversation_id: stored.conversationId, status: "completed", results: [],
    decision: stored.decision ?? { kind: "advisory", decision_id: id("e6f"), selected: true,
      text: `Answer to ${stored.message}`, continuation: { kind: "absent" } } });
  const runsOf = (conversationId: string) => ALL_RUNS.filter((stored) => stored.conversationId === conversationId);
  const conversationRead = (conversationId: string) => jsonResponse({ ...conversationFixture, project_scope: scope,
    conversation_id: conversationId, completeness: "complete", runs: runsOf(conversationId).map((stored) => ({
      run_id: stored.runId, author_message: stored.message, chapter_id: CHAPTER, status: "completed",
      created_at: "2026-11-27T09:30:00.000Z" })) });
  const proposalRead = (proposalId: string) => jsonResponse({ ...proposalFixture, project_scope: scope,
    proposal: { ...proposalFixture.proposal, proposal_id: proposalId,
      source: { kind: "agent_run_decision", run_id: PROSE.runId, decision_id: id("e6b") } } });
  // The current exchange reads its research sources, as today.
  const current = (stored: StoredRun): [string, () => Response][] => [[runRoute(stored), () => runRead(stored)],
    [`${runRoute(stored)}/research-sources`, () => jsonResponse({ code: "not_found" }, 404)]];
  const routes: Record<string, () => Response> = {
    "GET /assistance": () => jsonResponse({ ...assistanceFixture, project_scope: scope }),
    "GET /skills": () => jsonResponse({ ...skillsFixture, project_scope: scope, skills: [] }),
    ...Object.fromEntries(current(STORED)),
    "GET /conversations": () => jsonResponse({ ...listFixture, project_scope: scope, completeness: "complete",
      conversations: conversations.map((conversationId) => ({ conversation_id: conversationId, label: conversationId,
        first_run_created_at: "2026-11-27T09:00:00.000Z", newest_run_created_at: "2026-11-27T09:00:00.000Z",
        newest_run_status: "completed" })) }),
    ...Object.fromEntries(conversations.flatMap((conversationId) => {
      const runs = runsOf(conversationId);
      return [[`GET /conversations/${conversationId}`, () => conversationRead(conversationId)],
        ...runs.slice(0, -1).map((stored) => [runRoute(stored), () => runRead(stored)]), ...current(runs.at(-1)!)];
    })),
    ...Object.fromEntries(conversations.includes(P) ? [PROPOSAL_1, PROPOSAL_2].map((proposalId) =>
      [`GET /proposals/${proposalId}`, () => proposalRead(proposalId)]) : []),
    ...answers,
  };
  const unexpected: string[] = [];
  const requests: Request[] = [];
  const holds = new Map<string, { promise: Promise<void>; release: () => void }>();
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const request = `${init?.method ?? "GET"} ${path.startsWith(`${PREFIX}/`) ? path.slice(PREFIX.length) : path}`;
    const answer = path.startsWith(`${PREFIX}/`) ? routes[request] : undefined;
    if (answer === undefined) {
      unexpected.push(request);
      throw new Error(`Unexpected request ${request}`);
    }
    const hold = holds.get(request);
    const entry: Request = { request, held: hold !== undefined, answered: false };
    requests.push(entry);
    await hold?.promise;
    const response = answer();
    const text = response.text.bind(response);
    response.text = async () => {
      try {
        return await text();
      } finally {
        entry.answered = true;
      }
    };
    return response;
  };
  const opened: string[] = [];
  const panelHost = openPanelHost();
  const host = panelHost.host;
  cleanup.push(() => panelHost.close());
  // Waits until each request that is not held is answered, and the panel handled each answer: the work after an
  // answer runs in microtasks, and they all run before the next task.
  const settle = async () => {
    do {
      await act(async () => { await new Promise((resolve) => setTimeout(resolve, 0)); });
    } while (requests.some((entry) => !entry.held && !entry.answered));
  };
  await act(async () => {
    panelHost.root.render(createElement(WritingAssistantPanel, {
      collapsed: false, scheduleRefresh: () => () => {},
      onOpenedProposal: (locator) => { opened.push(locator.proposalId); },
      context: { scope, chapterId: CHAPTER, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto },
    }));
  });
  await settle();
  const click = async (selector: string) => {
    await act(async () => { host.querySelector<HTMLElement>(selector)!.click(); });
    await settle();
  };
  const button = (selector: string) => host.querySelector<HTMLButtonElement>(selector)!;
  return {
    unexpected, opened,
    /** Each request that has no answer yet, also a second request of the same route. */
    inFlight: () => requests.filter((entry) => !entry.answered).map((entry) => entry.request).sort(),
    hold: (...held: string[]) => {
      for (const request of held) {
        let release = () => {};
        const promise = new Promise<void>((resolve) => { release = resolve; });
        holds.set(request, { promise, release });
      }
    },
    release: async (...held: string[]) => {
      for (const request of held) {
        holds.get(request)!.release();
        holds.delete(request);
        for (const entry of requests) if (entry.request === request) entry.held = false;
      }
      await settle();
    },
    select: async (conversationId: string) => {
      await click("[data-assistant-conversation-history]");
      await click(`[data-assistant-conversation-entry="${conversationId}"]`);
    },
    type: async (text: string) => {
      host.querySelector("textarea")!.focus();
      await applyTrustedInput({ operation: "insert_text", text });
      await settle();
    },
    /** Each exchange as its author message, its result, and its line of a result that loads. */
    exchanges: () => [...host.querySelectorAll("[data-assistant-history-run], [data-assistant-current-run]")]
      .map((section) => [...section.querySelectorAll(".assistant-author-message, .assistant-result, "
        + "[data-assistant-result-loading]")].map((line) => line.textContent)),
    shownRunId: () => host.querySelector("[data-assistant-current-run]") === null ? null
      : host.querySelector("#writing-assistant-panel")!.getAttribute("data-assistant-run-id"),
    status: () => host.querySelector("[role=status]")?.textContent ?? null,
    controls: () => ({ send: !button("button.send-button").disabled,
      newConversation: !button("[data-assistant-new-conversation]").disabled,
      history: !button("[data-assistant-conversation-history]").disabled }),
  };
}

const B_SHOWN = [["B first", "Answer to B first"]];
const A_SHOWN = [["A first", "Answer to A first"], ["A second", "Answer to A second"], ["A third", "Answer to A third"]];
const C_SHOWN = [["C first", "Answer to C first"]];

it("shows the author messages of the selected Conversation before its Runs answer, and no part of the earlier one",
  async () => {
    const panel = await openPanel([A]);
    const before = panel.exchanges();
    await panel.type("Next request");
    panel.hold(`GET /conversations/${A}`, A1, A2, A3);
    await panel.select(A);
    const selected = { exchanges: panel.exchanges(), runId: panel.shownRunId() };
    await panel.release(`GET /conversations/${A}`);
    const messages = { exchanges: panel.exchanges(), controls: panel.controls() };
    await panel.release(A2);
    const second = panel.exchanges();
    await panel.release(A3);
    const newest = panel.exchanges();
    await panel.release(A1);

    expect({ before, selected, messages, second, newest, opened: panel.exchanges(), runId: panel.shownRunId(),
      controls: panel.controls(), unexpected: panel.unexpected }).toEqual({
      before: B_SHOWN,
      selected: { exchanges: [], runId: null },
      // The composer and `新对话` wait for the Runs, and the list stays usable for another selection.
      messages: { exchanges: [["A first", LOADING], ["A second", LOADING], ["A third", LOADING]],
        controls: { send: false, newConversation: false, history: true } },
      second: [["A first", LOADING], ["A second", "Answer to A second"], ["A third", LOADING]],
      // The newest Run shows its result when it becomes the current exchange.
      newest: [["A first", LOADING], ["A second", "Answer to A second"], ["A third", LOADING]],
      opened: A_SHOWN,
      runId: RUNS[2]!.runId,
      controls: { send: true, newConversation: true, history: true },
      unexpected: [],
    });
  });

it("starts the reads of the three Runs of a Conversation at the same time", async () => {
  const panel = await openPanel([A]);
  panel.hold(A1, A2, A3);
  await panel.select(A);
  const inFlight = panel.inFlight();
  await panel.release(A1, A2, A3);

  expect({ inFlight, opened: panel.exchanges(), unexpected: panel.unexpected })
    .toEqual({ inFlight: [A1, A2, A3].sort(), opened: A_SHOWN, unexpected: [] });
});

it.each([
  ["its Conversation read", "the earlier selection", [`GET /conversations/${A}`]],
  ["its Conversation read", "the later selection", [`GET /conversations/${A}`]],
  ["its Run reads", "the earlier selection", [A1, A2, A3]],
  ["its Run reads", "the later selection", [A1, A2, A3]],
] as const)("keeps a later selection when the earlier one answers late at %s, and %s answers first",
  async (_, first, earlier) => {
    const panel = await openPanel([A, C]);
    panel.hold(...earlier, C1);
    await panel.select(A);
    await panel.select(C);
    const selected = panel.exchanges();
    if (first === "the earlier selection") await panel.release(...earlier);
    else await panel.release(C1);
    const afterFirst = panel.exchanges();
    if (first === "the earlier selection") await panel.release(C1);
    else await panel.release(...earlier);

    expect({ selected, afterFirst, shown: panel.exchanges(), runId: panel.shownRunId(), unexpected: panel.unexpected })
      .toEqual({ selected: [["C first", LOADING]],
        afterFirst: first === "the earlier selection" ? [["C first", LOADING]] : C_SHOWN,
        shown: C_SHOWN, runId: RUNS[3]!.runId, unexpected: [] });
  });

it.each(["the Proposal reads", "the later selection"] as const)(
  "gives no Proposal of an earlier selection to the editor after a later selection, with the answers of %s first",
  async (first) => {
    const panel = await openPanel([P, A]);
    panel.hold(PROPOSAL_READ_1, PROPOSAL_READ_2, `GET /conversations/${A}`);
    await panel.select(P);
    await panel.select(A);
    if (first === "the Proposal reads") await panel.release(PROPOSAL_READ_1, PROPOSAL_READ_2);
    else await panel.release(`GET /conversations/${A}`);
    const afterFirst = [...panel.opened];
    if (first === "the Proposal reads") await panel.release(`GET /conversations/${A}`);
    else await panel.release(PROPOSAL_READ_1, PROPOSAL_READ_2);

    expect({ afterFirst, opened: panel.opened, shown: panel.exchanges(), unexpected: panel.unexpected })
      .toEqual({ afterFirst: [], opened: [], shown: A_SHOWN, unexpected: [] });
  });

it.each([
  ["a failed Conversation read", { [`GET /conversations/${A}`]: () => jsonResponse({ code: "service_unavailable" }, 503) }],
  ["a failed Run read", { [A2]: () => jsonResponse({ code: "service_unavailable" }, 503) }],
  ["a Run read of another Conversation", { [A2]: () => jsonResponse({ ...agentRunFixture, project_scope: scope,
    run_id: RUNS[1]!.runId, conversation_id: B, status: "completed", results: [] }) }],
])("gives the sentence of today for %s, and the panel shows the Conversation of before the selection",
  async (_, answers) => {
    const panel = await openPanel([A], answers);
    await panel.type("Next request");
    await panel.select(A);

    expect({ status: panel.status(), shown: panel.exchanges(), runId: panel.shownRunId(), controls: panel.controls(),
      unexpected: panel.unexpected }).toEqual({ status: OPEN_FAILED, shown: B_SHOWN, runId: STORED.runId,
      controls: { send: true, newConversation: true, history: true }, unexpected: [] });
  });

it.each([
  ["the first Proposal read", PROPOSAL_READ_1, PROPOSAL_READ_2],
  ["the second Proposal read", PROPOSAL_READ_2, PROPOSAL_READ_1],
])("reads the Proposals of the opened Run together, and gives them in their order after the reads, when %s answers first",
  async (_, first, second) => {
    const panel = await openPanel([P]);
    panel.hold(first, second);
    await panel.select(P);
    const inFlight = panel.inFlight();
    await panel.release(first);
    const afterFirst = [...panel.opened];
    await panel.release(second);

    expect({ inFlight, afterFirst, opened: panel.opened, unexpected: panel.unexpected }).toEqual({
      inFlight: [PROPOSAL_READ_1, PROPOSAL_READ_2].sort(), afterFirst: [], opened: [PROPOSAL_1, PROPOSAL_2],
      unexpected: [] });
  });
