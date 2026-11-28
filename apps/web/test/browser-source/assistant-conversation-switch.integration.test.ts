import { act, createElement } from "react";
import { afterEach, describe, expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import cancelFixture from "../../../../generated/golden-wire/storyos-public-release-1/cancel-agent-run.json";
import challengeFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-project-command-challenge.json";
import conversationFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-conversation.json";
import listFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-project-conversations.json";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import treeFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-manuscript-tree.json";
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
const NEXT_IS_NEW = "下一条消息将开始新对话。";

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
// A reloaded Run of Conversation B with one revised Proposal.
const STORED_PROSE = { ...run("e60", B, "B first"), decision: { ...PROSE.decision, locations: [revised(PROPOSAL_1, "5")] } };
// A Run of Conversation B that runs at the first read and is completed with one revised Proposal at each later read.
const RUNNING = run("eb1", B, "B first");
const ALL_RUNS = [STORED, ...RUNS, PROSE];
const runRoute = (stored: StoredRun) => `GET /agent-runs/${stored.runId}`;
const [A1, A2, A3, C1] = RUNS.map(runRoute) as [string, string, string, string];
const A3_SOURCES = `${A3}/research-sources`;
const A_READ = `GET /conversations/${A}`;
const [PROPOSAL_READ_1, PROPOSAL_READ_2] = [PROPOSAL_1, PROPOSAL_2]
  .map((proposalId) => `GET /proposals/${proposalId}`) as [string, string];

let cleanup: Array<() => Promise<void>> = [];
afterEach(async () => {
  for (const step of cleanup) await step();
  cleanup = [];
});

/** One request to the fake Server. It is answered when the panel read the whole response body. */
type Request = { request: string; hold?: string; held: boolean; answered: boolean };

type Setup = {
  conversations: readonly string[];
  /** A changed answer of a route, or `null` for a route that the test does not send. */
  answers?: Record<string, (() => Response) | null>;
  /** The Run of Conversation B that the browser session holds. */
  stored?: StoredRun;
  /** The routes that are held from the start. */
  held?: readonly string[];
};

/**
 * Opens the panel on a Run of Conversation B on a fake Server. The fake serves only the requests of the panel and of
 * `conversations`, with the Project in the path. A held request answers only after `release`.
 */
async function openPanel({ conversations, answers = {}, stored: storedRun = STORED, held: heldAtStart = [] }: Setup) {
  seedProseRequest({ scope, message: storedRun.message, chapterId: CHAPTER, correlationId: id("e6c"),
    idempotencyKey: id("e6d"), snapshotId: id("e6e"), runId: storedRun.runId, conversationId: B });
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
  let runningReads = 0;
  const served: Record<string, (() => Response) | null> = {
    "GET /assistance": () => jsonResponse({ ...assistanceFixture, project_scope: scope }),
    "GET /skills": () => jsonResponse({ ...skillsFixture, project_scope: scope, skills: [] }),
    ...Object.fromEntries(current(storedRun)),
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
    ...storedRun === STORED_PROSE ? { [PROPOSAL_READ_1]: () => proposalRead(PROPOSAL_1) } : {},
    ...storedRun === RUNNING ? {
      [runRoute(RUNNING)]: () => (runningReads += 1) === 1
        ? jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUNNING.runId, conversation_id: B,
          status: "claimed", results: [], decision: { kind: "absent" } })
        : runRead({ ...RUNNING, decision: STORED_PROSE.decision }),
      "POST /anti-forgery-challenges": () => jsonResponse(challengeFixture),
      [`POST /agent-runs/${RUNNING.runId}/cancel`]: () => jsonResponse({ ...cancelFixture, project_scope: scope,
        receipt: { ...cancelFixture.receipt, project_scope: scope }, effect: { ...cancelFixture.effect, run_id: RUNNING.runId } }),
      [PROPOSAL_READ_1]: () => proposalRead(PROPOSAL_1),
    } : {},
    ...answers,
  };
  const routes = Object.fromEntries(Object.entries(served).flatMap(([route, answer]) =>
    answer === null ? [] : [[route, answer]]));
  const unexpected: string[] = [];
  const requests: Request[] = [];
  const bodies: Record<string, unknown> = {};
  const holds = new Map<string, { promise: Promise<void>; release: () => void }>();
  const hold = (...held: string[]) => {
    for (const request of held) {
      let release = () => {};
      const promise = new Promise<void>((resolve) => { release = resolve; });
      holds.set(request, { promise, release });
    }
  };
  hold(...heldAtStart);
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const request = `${init?.method ?? "GET"} ${path.startsWith(`${PREFIX}/`) ? path.slice(PREFIX.length) : path}`;
    const answer = path.startsWith(`${PREFIX}/`) ? routes[request] : undefined;
    if (answer === undefined) {
      unexpected.push(request);
      throw new Error(`Unexpected request ${request}`);
    }
    // A hold of `<route>#<n>` holds only the request number n of the route.
    const count = `${request}#${requests.filter((entry) => entry.request === request).length + 1}`;
    const key = holds.has(request) ? request : holds.has(count) ? count : undefined;
    const holding = key === undefined ? undefined : holds.get(key);
    if (typeof init?.body === "string") bodies[request] = JSON.parse(init.body);
    const entry: Request = { request, ...key === undefined ? {} : { hold: key }, held: holding !== undefined,
      answered: false };
    requests.push(entry);
    await holding?.promise;
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
  const scheduled: Array<() => void> = [];
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
      collapsed: false,
      scheduleRefresh: (refresh) => {
        scheduled.push(refresh);
        return () => { scheduled.splice(scheduled.indexOf(refresh) >>> 0, 1); };
      },
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
    unexpected, opened, hold, click, bodies,
    /** Runs the next scheduled read of the current Run. */
    poll: async () => {
      await act(async () => { scheduled.shift()!(); });
      await settle();
    },
    /** Each route that the fake serves and the panel did not request. */
    unused: () => Object.keys(routes).filter((route) => !requests.some((entry) => entry.request === route)),
    /** Each request that has no answer yet, also a second request of the same route. */
    inFlight: () => requests.filter((entry) => !entry.answered).map((entry) => entry.request).sort(),
    release: async (...held: string[]) => {
      for (const request of held) {
        holds.get(request)!.release();
        holds.delete(request);
        for (const entry of requests) if (entry.hold === request) entry.held = false;
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
    currentSection: () => host.querySelector("[data-assistant-current-run]"),
    exchanges: () => [...host.querySelectorAll("[data-assistant-history-run], [data-assistant-current-run]")]
      .map((section) => [...section.querySelectorAll(".assistant-author-message, .assistant-result, "
        + "[data-assistant-result-loading]")].map((line) => line.textContent)),
    shownRunId: () => host.querySelector("[data-assistant-current-run]") === null ? null
      : host.querySelector("#writing-assistant-panel")!.getAttribute("data-assistant-run-id"),
    status: () => host.querySelector("[role=status]")?.textContent ?? null,
    lines: () => [...host.querySelectorAll(".assistant-status")].map((line) => line.textContent),
    controls: () => ({ send: !button("button.send-button").disabled,
      newConversation: !button("[data-assistant-new-conversation]").disabled,
      history: !button("[data-assistant-conversation-history]").disabled }),
  };
}

type Panel = Awaited<ReturnType<typeof openPanel>>;

const B_SHOWN = [["B first", "Answer to B first"]];
const A_SHOWN = [["A first", "Answer to A first"], ["A second", "Answer to A second"], ["A third", "Answer to A third"]];
const C_SHOWN = [["C first", "Answer to C first"]];
const SERVICE_UNAVAILABLE = () => jsonResponse({ code: "service_unavailable" }, 503);

it("shows the author messages of the selected Conversation before its Runs answer, and no part of the earlier one",
  async () => {
    const panel = await openPanel({ conversations: [A] });
    const before = panel.exchanges();
    await panel.type("Next request");
    panel.hold(A_READ, A1, A2, A3);
    await panel.select(A);
    const selected = { exchanges: panel.exchanges(), runId: panel.shownRunId() };
    await panel.release(A_READ);
    const messages = { exchanges: panel.exchanges(), controls: panel.controls() };
    await panel.release(A2);
    const second = panel.exchanges();
    await panel.release(A3);
    const newest = { exchanges: panel.exchanges(), section: panel.currentSection() };
    await panel.release(A1);

    expect({ before, selected, messages, second, newest: newest.exchanges, opened: panel.exchanges(),
      runId: panel.shownRunId(), sameSection: panel.currentSection() === newest.section, controls: panel.controls(),
      unexpected: panel.unexpected, unused: panel.unused() }).toEqual({
      before: B_SHOWN,
      selected: { exchanges: [], runId: null },
      // The composer and `新对话` wait for the Runs, and the list stays usable for another selection.
      messages: { exchanges: [["A first", LOADING], ["A second", LOADING], ["A third", LOADING]],
        controls: { send: false, newConversation: false, history: true } },
      second: [["A first", LOADING], ["A second", "Answer to A second"], ["A third", LOADING]],
      newest: [["A first", LOADING], ["A second", "Answer to A second"], ["A third", "Answer to A third"]],
      opened: A_SHOWN,
      runId: RUNS[2]!.runId,
      // The end of the open keeps the current exchange and its controls.
      sameSection: true,
      controls: { send: true, newConversation: true, history: true },
      unexpected: [],
      unused: [],
    });
  });

it("starts the reads of the three Runs of a Conversation at the same time", async () => {
  const panel = await openPanel({ conversations: [A] });
  panel.hold(A1, A2, A3);
  await panel.select(A);
  const inFlight = panel.inFlight();
  await panel.release(A1, A2, A3);

  expect({ inFlight, opened: panel.exchanges(), unexpected: panel.unexpected, unused: panel.unused() })
    .toEqual({ inFlight: [A1, A2, A3].sort(), opened: A_SHOWN, unexpected: [], unused: [] });
});

it.each([
  ["its Conversation read", "the earlier selection", [A_READ], [A1, A2, A3, A3_SOURCES]],
  ["its Conversation read", "the later selection", [A_READ], [A1, A2, A3, A3_SOURCES]],
  ["its Run reads", "the earlier selection", [A1, A2, A3], [A3_SOURCES]],
  ["its Run reads", "the later selection", [A1, A2, A3], [A3_SOURCES]],
] as const)("keeps a later selection when the earlier one answers late at %s, and %s answers first",
  async (_, first, earlier, notSent) => {
    const panel = await openPanel({ conversations: [A, C],
      answers: Object.fromEntries(notSent.map((route) => [route, null])) });
    panel.hold(...earlier, C1);
    await panel.select(A);
    await panel.select(C);
    const selected = panel.exchanges();
    if (first === "the earlier selection") await panel.release(...earlier);
    else await panel.release(C1);
    const afterFirst = panel.exchanges();
    if (first === "the earlier selection") await panel.release(C1);
    else await panel.release(...earlier);

    expect({ selected, afterFirst, shown: panel.exchanges(), runId: panel.shownRunId(), unexpected: panel.unexpected,
      unused: panel.unused() }).toEqual({ selected: [["C first", LOADING]],
      afterFirst: first === "the earlier selection" ? [["C first", LOADING]] : C_SHOWN,
      shown: C_SHOWN, runId: RUNS[3]!.runId, unexpected: [], unused: [] });
  });

it.each([
  ["an opened Conversation", "the Proposal reads"],
  ["an opened Conversation", "the later selection"],
  ["the reloaded request", "the Proposal reads"],
  ["the reloaded request", "the later selection"],
] as const)("gives no Proposal of the Run of %s to the editor after a selection, with the answers of %s first",
  async (source, first) => {
    const reads = source === "an opened Conversation" ? [PROPOSAL_READ_1, PROPOSAL_READ_2] : [PROPOSAL_READ_1];
    const panel = source === "an opened Conversation" ? await openPanel({ conversations: [P, A] })
      : await openPanel({ conversations: [A], stored: STORED_PROSE, held: reads });
    if (source === "an opened Conversation") panel.hold(...reads);
    panel.hold(A_READ);
    if (source === "an opened Conversation") await panel.select(P);
    await panel.select(A);
    if (first === "the Proposal reads") await panel.release(...reads);
    else await panel.release(A_READ);
    const afterFirst = [...panel.opened];
    if (first === "the Proposal reads") await panel.release(A_READ);
    else await panel.release(...reads);

    expect({ afterFirst, opened: panel.opened, shown: panel.exchanges(), unexpected: panel.unexpected,
      unused: panel.unused() }).toEqual({ afterFirst: [], opened: [], shown: A_SHOWN, unexpected: [], unused: [] });
  });

const OTHER_CONVERSATION = () => jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUNS[1]!.runId,
  conversation_id: B, status: "completed", results: [] });

// A send after the failure: the fake answers the steps of a send up to the AgentRun command, which it holds.
const SEND = { "GET /manuscript/tree": () => jsonResponse({ ...treeFixture, project_scope: scope,
  snapshot: { ...treeFixture.snapshot, project_scope: scope } }),
"POST /anti-forgery-challenges": () => jsonResponse(challengeFixture),
"POST /agent-runs": () => jsonResponse({ code: "not_reached" }, 500) };

it.each([
  ["a failed Conversation read", "", { [A_READ]: SERVICE_UNAVAILABLE, [A1]: null, [A2]: null, [A3]: null,
    [A3_SOURCES]: null }, []],
  // The newest Run answers after the failure, so the panel never shows it.
  ["a failed Run read", " before the newest Run", { [A2]: SERVICE_UNAVAILABLE, [A3_SOURCES]: null }, [A3]],
  // The panel shows the newest Run and reads its research sources, then the failure comes.
  ["a failed Run read", " after the newest Run", { [A2]: SERVICE_UNAVAILABLE }, [A2]],
  ["a Run read of another Conversation", " before the newest Run", { [A2]: OTHER_CONVERSATION, [A3_SOURCES]: null }, [A3]],
  ["a Run read of another Conversation", " after the newest Run", { [A2]: OTHER_CONVERSATION }, [A2]],
] as const)("gives the sentence of today for %s%s, shows no Conversation, and the next message starts a new one",
  async (_, __, answers, late) => {
    // The coordinator decided on #1966 that a failed open sets the new Conversation choice.
    const panel = await openPanel({ conversations: [A], answers: { ...answers, ...SEND } });
    await panel.type("Next request");
    panel.hold(...late, "POST /agent-runs");
    await panel.select(A);
    await panel.release(...late);
    const failed = { status: panel.status(), shown: panel.exchanges(), lines: panel.lines(),
      controls: panel.controls() };
    await panel.click("button.send-button");

    const request = panel.bodies["POST /agent-runs"] as { create_agent_run_input: { conversation: unknown } };
    expect({ failed, conversation: request.create_agent_run_input.conversation,
      earlierShown: panel.exchanges().flat().includes(STORED.message),
      unexpected: panel.unexpected, unused: panel.unused() }).toEqual({
      failed: { status: OPEN_FAILED, shown: [], lines: [NEXT_IS_NEW],
        controls: { send: true, newConversation: false, history: true } },
      conversation: { kind: "new" }, earlierShown: false, unexpected: [], unused: [] });
  });

it("changes nothing when the read of a selection fails after a later selection", async () => {
  const panel = await openPanel({ conversations: [A, C], answers: { [A_READ]: SERVICE_UNAVAILABLE, [A1]: null,
    [A2]: null, [A3]: null, [A3_SOURCES]: null }, held: [A_READ] });
  await panel.select(A);
  await panel.select(C);
  const selected = { shown: panel.exchanges(), runId: panel.shownRunId() };
  await panel.release(A_READ);

  expect({ selected, status: panel.status(), shown: panel.exchanges(), runId: panel.shownRunId(),
    lines: panel.lines(), unexpected: panel.unexpected, unused: panel.unused() }).toEqual({
    selected: { shown: C_SHOWN, runId: RUNS[3]!.runId }, status: null, shown: C_SHOWN, runId: RUNS[3]!.runId,
    lines: [], unexpected: [], unused: [] });
});

const runningRead = (status: string, decision: object) => () => jsonResponse({ ...agentRunFixture, project_scope: scope,
  run_id: RUNNING.runId, conversation_id: B, status, results: [], decision });
// The Run settles with a Proposal that the editor gets with no Proposal read.
const OPENED_PROPOSAL = runningRead("completed", { ...PROSE.decision, locations: [],
  opened_proposal: { kind: "present", proposal_id: PROPOSAL_1 } });
const CANCELLED = runningRead("cancelled", { kind: "absent" });

describe.each([
  // The scheduled read is held, and the check of the author finds the Run completed.
  ["a scheduled Run read", OPENED_PROPOSAL, async (panel: Panel) => {
    await panel.poll();
    await panel.click("button[data-assistant-inspect]");
  }, { "POST /anti-forgery-challenges": null, [`POST /agent-runs/${RUNNING.runId}/cancel`]: null }],
  // The check of the author is held, and the scheduled read finds the Run completed.
  ["the check of the author", OPENED_PROPOSAL, async (panel: Panel) => {
    await panel.click("button[data-assistant-inspect]");
    await panel.poll();
  }, { "POST /anti-forgery-challenges": null, [`POST /agent-runs/${RUNNING.runId}/cancel`]: null }],
  // The read after the run control is held, and the scheduled read finds the Run cancelled.
  ["the Run read after a run control", CANCELLED, async (panel: Panel) => {
    await panel.click("button[data-cancel-agent-run]");
    await panel.poll();
  }, {}],
] as const)("%s that started before a selection", (_, settled, start, unusedRoutes) => {
  it.each([
    ["fails", "before", SERVICE_UNAVAILABLE], ["fails", "after", SERVICE_UNAVAILABLE],
    ["answers", "before", settled], ["answers", "after", settled],
  ] as const)("%s %s the selected Conversation opens, and shows nothing", async (__, order, late) => {
    let reads = 0;
    const panel = await openPanel({ conversations: [A], stored: RUNNING, answers: { ...unusedRoutes,
      // In the order of the answers: the first read finds the Run running, the read that is not held finds it
      // settled, and the held read answers last.
      [runRoute(RUNNING)]: () => [runningRead("claimed", { kind: "absent" }), settled, late][reads++]!(),
      [PROPOSAL_READ_1]: null } });
    const heldRead = `${runRoute(RUNNING)}#2`;
    panel.hold(heldRead);
    await start(panel);
    const beforeSelection = [...panel.opened];
    if (order === "before") {
      panel.hold(A_READ);
      await panel.select(A);
      await panel.release(heldRead, A_READ);
    } else {
      await panel.select(A);
      await panel.release(heldRead);
    }

    expect({ opened: panel.opened, status: panel.status(), shown: panel.exchanges(), runId: panel.shownRunId(),
      unexpected: panel.unexpected, unused: panel.unused() }).toEqual({ opened: beforeSelection, status: null,
      shown: A_SHOWN, runId: RUNS[2]!.runId, unexpected: [], unused: [] });
  });
});

it.each([
  ["the first Proposal read", PROPOSAL_READ_1, PROPOSAL_READ_2],
  ["the second Proposal read", PROPOSAL_READ_2, PROPOSAL_READ_1],
])("reads the Proposals of the opened Run together, and gives them in their order after the reads, when %s answers first",
  async (_, first, second) => {
    const panel = await openPanel({ conversations: [P] });
    panel.hold(first, second);
    await panel.select(P);
    const inFlight = panel.inFlight();
    await panel.release(first);
    const afterFirst = [...panel.opened];
    await panel.release(second);

    expect({ inFlight, afterFirst, opened: panel.opened, unexpected: panel.unexpected, unused: panel.unused() })
      .toEqual({ inFlight: [PROPOSAL_READ_1, PROPOSAL_READ_2].sort(), afterFirst: [], opened: [PROPOSAL_1, PROPOSAL_2],
        unexpected: [], unused: [] });
  });

it.each(["the Run read", "the later selection"] as const)(
  "gives no Proposal to the editor from a Run read of a run control that started before a selection, with %s first",
  async (first) => {
    const panel = await openPanel({ conversations: [A], stored: RUNNING });
    const cancel = `POST /agent-runs/${RUNNING.runId}/cancel`;
    panel.hold(cancel);
    await panel.click("button[data-cancel-agent-run]");
    // The scheduled read finds the Run completed with its Proposal, and the list of Conversations becomes usable.
    await panel.poll();
    const beforeSelection = [...panel.opened];
    panel.hold(runRoute(RUNNING), A_READ);
    // The answer of the run control makes the panel read the Run again.
    await panel.release(cancel);
    await panel.select(A);
    if (first === "the Run read") await panel.release(runRoute(RUNNING));
    else await panel.release(A_READ);
    if (first === "the Run read") await panel.release(A_READ);
    else await panel.release(runRoute(RUNNING));

    expect({ beforeSelection, opened: panel.opened, shown: panel.exchanges(), unexpected: panel.unexpected,
      unused: panel.unused() }).toEqual({ beforeSelection: [PROPOSAL_1], opened: [PROPOSAL_1], shown: A_SHOWN,
      unexpected: [], unused: [] });
  });

it.each(["before", "after"] as const)(
  "shows no result of a run control whose answer arrives after a later selection, %s the selected Conversation opens",
  async (order) => {
    let reads = 0;
    const panel = await openPanel({ conversations: [A], stored: RUNNING, answers: {
      [runRoute(RUNNING)]: () => [runningRead("claimed", { kind: "absent" }), CANCELLED][reads++]!(),
      [PROPOSAL_READ_1]: null } });
    const cancel = `POST /agent-runs/${RUNNING.runId}/cancel`;
    panel.hold(cancel);
    await panel.click("button[data-cancel-agent-run]");
    // The scheduled read finds the Run cancelled, and the list of Conversations becomes usable.
    await panel.poll();
    if (order === "before") {
      panel.hold(A_READ);
      await panel.select(A);
      await panel.release(cancel, A_READ);
    } else {
      await panel.select(A);
      await panel.release(cancel);
    }

    expect({ reads, status: panel.status(), shown: panel.exchanges(), unexpected: panel.unexpected,
      unused: panel.unused() }).toEqual({ reads: 2, status: null, shown: A_SHOWN, unexpected: [], unused: [] });
  });

it("gives the Proposal of the opened Run to the editor when the browser storage is full", async () => {
  const panel = await openPanel({ conversations: [P], answers: {
    [runRoute(PROSE)]: () => jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: PROSE.runId,
      conversation_id: P, status: "completed", results: [], decision: { ...PROSE.decision, locations: [],
        opened_proposal: { kind: "present", proposal_id: PROPOSAL_1 } } }),
    [PROPOSAL_READ_1]: null, [PROPOSAL_READ_2]: null } });
  const setItem = Storage.prototype.setItem;
  Storage.prototype.setItem = () => { throw new DOMException("full", "QuotaExceededError"); };
  cleanup.push(async () => { Storage.prototype.setItem = setItem; });
  await panel.select(P);

  expect({ opened: panel.opened, runId: panel.shownRunId(), unexpected: panel.unexpected, unused: panel.unused() })
    .toEqual({ opened: [PROPOSAL_1], runId: PROSE.runId, unexpected: [], unused: [] });
});
