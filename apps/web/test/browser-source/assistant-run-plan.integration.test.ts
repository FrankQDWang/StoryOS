import { act, createElement } from "react";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { openPanelHost, seedProseRequest } from "./assistant-panel-harness.ts";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const scope = { owner_user_id: OWNER, project_id: PROJECT };
const RUN = "018f0000-0000-7001-8000-000000000f51";
const EARLIER = "018f0000-0000-7001-8000-000000000f52";
const ANSWER = "第三章第一段已改好。";
const answer = { kind: "answer", decision_id: "018f0000-0000-7001-8000-000000000f53", text: ANSWER };
const settled = { kind: "advisory", decision_id: answer.decision_id, selected: true, text: ANSWER,
  continuation: { kind: "absent" } };

type Status = "pending" | "in_progress" | "completed";
const TEXTS = ["读第一章到第五章", "列出不一致之处", "改写第三章第一段"];
const plan = (statuses: readonly Status[]) => statuses.map((status, index) => ({ text: TEXTS[index]!, status }));
const MIXED = plan(["completed", "in_progress", "pending"]);
const DONE = plan(["completed", "completed", "completed"]);

/** One read of the Run `RUN` with `status`, and with `steps` as its plan when they are given. */
function read(status: "claimed" | "completed", steps?: readonly object[]) {
  return { ...agentRunFixture, project_scope: scope, run_id: RUN, status,
    ...(status === "completed" ? { decision: settled, results: [answer] } : { decision: { kind: "absent" } }),
    ...(steps === undefined ? {} : { plan: steps }) };
}

/** The text, the state attribute, and the accessible name of the mark of each step of the list in `section`. */
function shownPlan(section: Element | null | undefined) {
  return [...section?.querySelectorAll("[data-assistant-plan] > li") ?? []].map(step => [
    step.textContent, step.getAttribute("data-plan-step-status"),
    step.querySelector("[role=img]")?.getAttribute("aria-label")]);
}

/**
 * Renders the panel for the current Run `RUN` with `earlier` as one history turn when it is given. Each read of the
 * Run gives the next item of `reads`, and the last one after that. `next` fires the refresh that the panel scheduled.
 */
async function renderPanel(reads: readonly object[], earlier?: object) {
  seedProseRequest({ scope, message: "请检查第一章到第五章。", chapterId: CHAPTER,
    correlationId: "018f0000-0000-7001-8000-000000000f54", idempotencyKey: "018f0000-0000-7001-8000-000000000f55",
    snapshotId: "018f0000-0000-7001-8000-000000000f56", runId: RUN, conversationId: agentRunFixture.conversation_id,
    ...(earlier === undefined ? {} : { history: [{ requestId: "018f0000-0000-7001-8000-000000000f57",
      chapterId: CHAPTER, conversationId: agentRunFixture.conversation_id, runId: EARLIER, run: earlier,
      message: "先列一个计划。", status: "completed", result: ANSWER }] }) });
  const requests: string[] = [];
  let count = 0;
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    requests.push(path.replace(PROJECT, "<project>"));
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith(`/agent-runs/${RUN}`)) {
      count += 1;
      return jsonResponse(reads[Math.min(count, reads.length) - 1]);
    }
    if (earlier !== undefined && path.endsWith(`/agent-runs/${EARLIER}`)) return jsonResponse(earlier);
    throw new Error(`Unexpected request ${path}`);
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
    panel, requests,
    current: () => panel.host.querySelector("[data-assistant-current-run]"),
    async next() {
      const refresh = scheduled.shift();
      expect(refresh).toBeDefined();
      await act(async () => { refresh!(); });
      await settle();
    },
  };
}

// ADR 0103 section 2: while the Run works, the complete list is directly below the status line, and each step has
// its text, its state, and the accessible name of its mark. After the end, the list is directly above the first result.
it("shows the plan below the status line while the Run works and above the first result after the end", async () => {
  const shown = await renderPanel([read("claimed", MIXED), read("completed", MIXED)]);
  try {
    const status = shown.current()?.querySelector("[data-assistant-run-status]");
    const working = [shownPlan(shown.current()), status?.nextElementSibling?.hasAttribute("data-assistant-plan")];
    await shown.next();
    const list = shown.current()?.querySelector("[data-assistant-plan]");
    const ended = [shownPlan(shown.current()), shown.current()?.querySelector("[data-assistant-run-status]") ?? null,
      list?.nextElementSibling?.textContent, list?.nextElementSibling === shown.current()?.querySelector(".assistant-result")];

    expect([working, ended]).toEqual([
      [[[TEXTS[0], "completed", "已完成"], [TEXTS[1], "in_progress", "进行中"], [TEXTS[2], "pending", "未开始"]], true],
      [[[TEXTS[0], "completed", "已完成"], [TEXTS[1], "in_progress", "进行中"], [TEXTS[2], "pending", "未开始"]], null,
        ANSWER, true],
    ]);
  } finally {
    await shown.panel.close();
  }
});

// The list has no control: no button, no link, no input, and no step that takes the focus.
it("shows the plan of an earlier turn above its results, with no control", async () => {
  const earlier = { ...read("completed", DONE), run_id: EARLIER };
  const shown = await renderPanel([read("completed")], earlier);
  try {
    const turn = shown.panel.host.querySelector(`[data-assistant-history-run="${EARLIER}"]`);
    const list = turn?.querySelector("[data-assistant-plan]");

    expect([shownPlan(turn), list?.nextElementSibling === turn?.querySelector(".assistant-result"),
      list?.querySelectorAll("a, button, input, [tabindex]").length, shownPlan(shown.current())]).toEqual([
      TEXTS.map(text => [text, "completed", "已完成"]), true, 0, []]);
  } finally {
    await shown.panel.close();
  }
});

// ADR 0103 section 2: a Run with no plan shows no list. The panel shows the newest list, and a changed plan comes with
// the refresh of a Run that exists: the panel sends the same requests as for a Run with no plan.
it("replaces the list with the plan of a later read and sends no other request", async () => {
  const withPlan = await renderPanel([read("claimed", plan(["in_progress", "pending", "pending"])),
    read("claimed", MIXED), read("completed", DONE)]);
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
  const noPlan = await renderPanel([read("claimed"), read("claimed"), read("completed")]);
  try {
    await noPlan.next();
    await noPlan.next();
    lists.push(noPlan.panel.host.querySelectorAll("[data-assistant-plan]").length);
  } finally {
    await noPlan.panel.close();
  }

  expect([lists.map(list => Array.isArray(list) ? list.map(step => (step as string[])[1]) : list),
    withPlan.requests, withPlan.requests.length]).toEqual([
    [["in_progress", "pending", "pending"], ["completed", "in_progress", "pending"],
      ["completed", "completed", "completed"], 0],
    noPlan.requests, noPlan.requests.length]);
});
