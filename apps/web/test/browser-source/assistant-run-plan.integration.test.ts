import { act, createElement } from "react";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import sourcesFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-research-sources.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { openPanelHost, seedProseRequest } from "./assistant-panel-harness.ts";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const scope = { owner_user_id: OWNER, project_id: PROJECT };
const project = `/api/v1/projects/${PROJECT}`;
// Each render has its own Run, because the panel reads the research sources of a Run one time only.
const RUNS = ["0f51", "0f58", "0f59", "0f5a", "0f5b"].map(suffix => `018f0000-0000-7001-8000-00000000${suffix}`);
const EARLIER = "018f0000-0000-7001-8000-000000000f52";
const ANSWER = "第三章第一段已改好。";
const answer = { kind: "answer", decision_id: "018f0000-0000-7001-8000-000000000f53", text: ANSWER };
const settled = { kind: "advisory", decision_id: answer.decision_id, selected: true, text: ANSWER,
  continuation: { kind: "absent" } };

type Status = "pending" | "in_progress" | "completed";
const TEXTS = ["读第一章到第五章", "列出不一致之处", "改写第三章第一段"];
const plan = (statuses: readonly Status[], texts = TEXTS) =>
  statuses.map((status, index) => ({ text: texts[index]!, status }));
const MIXED = plan(["completed", "in_progress", "pending"]);
const DONE = plan(["completed", "completed", "completed"]);

/** One read of a Run with `status`, and with `steps` as its plan when they are given. */
function read(status: "claimed" | "completed", steps?: readonly object[]) {
  return { ...agentRunFixture, project_scope: scope, status,
    ...(status === "completed" ? { decision: settled, results: [answer] } : { decision: { kind: "absent" } }),
    ...(steps === undefined ? {} : { plan: steps }) };
}

/** The text, the state attribute, and the accessible name of the mark of each step of the list in `section`. */
function shownPlan(section: Element | null | undefined) {
  return [...section?.querySelectorAll("[data-assistant-plan] > li") ?? []].map(step => [
    step.textContent, step.getAttribute("data-plan-step-status"),
    step.querySelector("[role=img]")?.getAttribute("aria-label")]);
}

/** The shown list of `steps`. */
function expectedPlan(steps: readonly { text: string; status: Status }[]) {
  const names = { completed: "已完成", in_progress: "进行中", pending: "未开始" };
  return steps.map(({ text, status }) => [text, status, names[status]]);
}

/**
 * Renders the panel for the current Run `run` with `earlier` as one history turn when it is given. Each read of the
 * Run gives the next item of `reads`, and the last one after that. The reads whose numbers (from 1) are in `held`
 * wait until `release` gives their answer. `next` fires the refresh that the panel scheduled.
 */
async function renderPanel(run: string, reads: readonly object[],
  { earlier, held = [] }: { earlier?: object; held?: number[] } = {}) {
  seedProseRequest({ scope, message: "请检查第一章到第五章。", chapterId: CHAPTER,
    correlationId: "018f0000-0000-7001-8000-000000000f54", idempotencyKey: "018f0000-0000-7001-8000-000000000f55",
    snapshotId: "018f0000-0000-7001-8000-000000000f56", runId: run, conversationId: agentRunFixture.conversation_id,
    ...(earlier === undefined ? {} : { history: [{ requestId: "018f0000-0000-7001-8000-000000000f57",
      chapterId: CHAPTER, conversationId: agentRunFixture.conversation_id, runId: EARLIER, run: earlier,
      message: "先列一个计划。", status: "completed", result: ANSWER }] }) });
  const requests: string[] = [];
  const unexpected: string[] = [];
  const waiting: Array<() => void> = [];
  let count = 0;
  const routes = new Map<string, () => Promise<Response>>([
    [`GET ${project}/assistance`, async () => jsonResponse({ ...assistanceFixture, project_scope: scope })],
    [`GET ${project}/skills`, async () => jsonResponse({ ...skillsFixture, project_scope: scope, skills: [] })],
    // A completed Run reads its research sources. This Run has none.
    [`GET ${project}/agent-runs/${run}/research-sources`, async () => jsonResponse({ ...sourcesFixture,
      project_scope: scope, run_id: run, snapshots: [], gaps: [], unknown_calls: [] })],
    [`GET ${project}/agent-runs/${run}`, () => {
      count += 1;
      const body = { ...reads[Math.min(count, reads.length) - 1], run_id: run };
      return held.includes(count) ? new Promise(resolve => waiting.push(() => resolve(jsonResponse(body))))
        : Promise.resolve(jsonResponse(body));
    }],
  ]);
  const fetchImpl: typeof fetch = async (input, init) => {
    const request = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    requests.push(request);
    const route = routes.get(request);
    if (route === undefined) {
      unexpected.push(request);
      throw new Error(`unexpected request: ${request}`);
    }
    return route();
  };
  const scheduled: Array<() => void> = [];
  const panel = openPanelHost();
  const settle = async () => {
    for (let turn = 0; turn < 5; turn += 1) await act(async () => { await Promise.resolve(); });
  };
  await act(async () => {
    panel.root.render(createElement(WritingAssistantPanel, {
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
  return {
    panel, requests, unexpected, settle,
    current: () => panel.host.querySelector("[data-assistant-current-run]"),
    async next() {
      const refresh = scheduled.shift();
      expect(refresh).toBeDefined();
      await act(async () => { refresh!(); });
      await settle();
    },
    async release() {
      await act(async () => { waiting.shift()!(); });
      await settle();
    },
  };
}

// ADR 0103 section 2: while the Run works, the complete list is directly below the status line, and each step has
// its text, its state, and the accessible name of its mark. After the end, the list is directly above the first result.
it("shows the plan below the status line while the Run works and above the first result after the end", async () => {
  const shown = await renderPanel(RUNS[0]!, [read("claimed", MIXED), read("completed", MIXED)]);
  try {
    const status = shown.current()?.querySelector("[data-assistant-run-status]");
    const working = [shownPlan(shown.current()), status?.nextElementSibling?.hasAttribute("data-assistant-plan")];
    await shown.next();
    const list = shown.current()?.querySelector("[data-assistant-plan]");
    const ended = [shownPlan(shown.current()), shown.current()?.querySelector("[data-assistant-run-status]") ?? null,
      list?.nextElementSibling?.textContent, list?.nextElementSibling === shown.current()?.querySelector(".assistant-result")];

    expect([working, ended, shown.unexpected]).toEqual([
      [expectedPlan(MIXED), true], [expectedPlan(MIXED), null, ANSWER, true], []]);
  } finally {
    await shown.panel.close();
  }
});

// The list has no control: no button, no link, no input, and no step that takes the focus.
it("shows the plan of an earlier turn above its results, with no control", async () => {
  const earlier = { ...read("completed", DONE), run_id: EARLIER };
  const shown = await renderPanel(RUNS[1]!, [read("completed")], { earlier });
  try {
    const turn = shown.panel.host.querySelector(`[data-assistant-history-run="${EARLIER}"]`);
    const list = turn?.querySelector("[data-assistant-plan]");

    expect([shownPlan(turn), list?.nextElementSibling === turn?.querySelector(".assistant-result"),
      list?.querySelectorAll("a, button, input, [tabindex]").length, shownPlan(shown.current()), shown.unexpected])
      .toEqual([expectedPlan(DONE), true, 0, [], []]);
  } finally {
    await shown.panel.close();
  }
});

// ADR 0103 section 2: a Run with no plan shows no list. The panel shows the newest list, and a changed plan comes with
// the refresh of a Run that exists: the panel sends the same requests as for a Run with no plan.
it("replaces the list with the plan of a later read and sends no other request", async () => {
  const first = plan(["in_progress", "pending"], ["读第一章", "列出不一致之处"]);
  const withPlan = await renderPanel(RUNS[2]!, [read("claimed", first), read("claimed", MIXED), read("completed", DONE)]);
  const lists: unknown[] = [];
  try {
    lists.push(shownPlan(withPlan.current()));
    await withPlan.next();
    lists.push(shownPlan(withPlan.current()));
    await withPlan.next();
    lists.push(shownPlan(withPlan.current()));
  } finally {
    await withPlan.panel.close();
  }
  const noPlan = await renderPanel(RUNS[3]!, [read("claimed"), read("claimed"), read("completed")]);
  try {
    await noPlan.next();
    await noPlan.next();
    lists.push(noPlan.panel.host.querySelectorAll("[data-assistant-plan]").length);
  } finally {
    await noPlan.panel.close();
  }

  const sequence = (run: string) => [`GET ${project}/assistance`, `GET ${project}/skills`,
    ...Array(3).fill(`GET ${project}/agent-runs/${run}`), `GET ${project}/agent-runs/${run}/research-sources`];
  expect([lists, withPlan.requests, noPlan.requests, withPlan.unexpected, noPlan.unexpected]).toEqual([
    [expectedPlan(first), expectedPlan(MIXED), expectedPlan(DONE), 0], sequence(RUNS[2]!), sequence(RUNS[3]!), [], []]);
});

// When the refresh and the author's check of the result overlap, a read that ends later with an older plan does not
// replace the list of the newer read.
it("keeps the newer plan when an older read ends after it", async () => {
  const shown = await renderPanel(RUNS[4]!, [read("claimed", plan(["in_progress", "pending", "pending"])),
    read("claimed", plan(["completed", "in_progress", "pending"])), read("claimed", DONE)], { held: [2] });
  try {
    await shown.next();
    await act(async () => { shown.current()?.querySelector<HTMLButtonElement>("[data-assistant-inspect]")?.click(); });
    await shown.settle();
    const newer = shownPlan(shown.current());
    await shown.release();

    expect([newer, shownPlan(shown.current()), shown.unexpected]).toEqual([expectedPlan(DONE), expectedPlan(DONE), []]);
  } finally {
    await shown.panel.close();
  }
});
