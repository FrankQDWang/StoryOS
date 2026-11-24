import { act, createElement } from "react";
import { beforeAll, expect, it } from "vitest";
import { userEvent } from "vitest/browser";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import sourcesFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-research-sources.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import subrunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-subrun.json";
import { SubrunList } from "../../src/subrun-list.tsx";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { openPanelHost, seedProseRequest } from "./assistant-panel-harness.ts";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const scope = { owner_user_id: OWNER, project_id: PROJECT };
const project = `/api/v1/projects/${PROJECT}`;
const id = (suffix: number) => `018f0000-0000-7001-8000-0000000010${suffix.toString(16).padStart(2, "0")}`;
// Each render has its own Run, because the panel reads the research sources of a Run one time only.
const RUNS = [0, 1, 2, 3, 4, 5, 6, 7, 8].map(index => id(index));
const EARLIER = id(0x10);
const ANSWER = "第三章第一段已改好。";
const answer = { kind: "answer", decision_id: id(0x11), text: ANSWER };
const settled = { kind: "advisory", decision_id: answer.decision_id, selected: true, text: ANSWER,
  continuation: { kind: "absent" } };
const PLAN = [{ text: "读第一章到第五章", status: "in_progress" }, { text: "改写第三章第一段", status: "pending" }];
const BRIEF = "读第一章到第五章，\n列出林舟的名字的每种写法。";
// The sentence of the panel for a result that it cannot read.
const UNREADABLE = "请求结果暂不可读取，请稍后检查。";
const LONG_TITLE = "通读第一章到第五章，列出林舟、沈月、顾川的每种名字写法，再找出每处时间不一致的段落";

type State = "working" | "completed" | "no_result" | "stopped";
const entry = (suffix: number, state: State, title = "通读前五章") => ({ subrun_id: id(suffix), title, state });
const FOUR = [entry(0x20, "working", LONG_TITLE), entry(0x21, "completed"), entry(0x22, "no_result", "查名字"),
  entry(0x23, "stopped", "查时间")];
const LINES = { working: "正在通读前文", completed: "已通读前文", no_result: "通读前文没有得出结论", stopped: "已停止" };

/** One read of a Run with `status`, with the plan and the Subruns of `extra`. */
function read(status: "claimed" | "completed", extra: { plan?: object[]; subruns?: object[] } = {}) {
  return { ...agentRunFixture, project_scope: scope, status,
    ...(status === "completed" ? { decision: settled, results: [answer] } : { decision: { kind: "absent" } }),
    ...extra };
}

/** One record of the Subrun `subrunId`, with an answer only when `answer` is given. */
function record(subrunId: string, state: State, stepCount: number, steps: object[], answer?: string) {
  const { answer: _, ...base } = subrunFixture;
  return { ...base, project_scope: scope, subrun_id: subrunId, title: "通读前五章", brief: BRIEF, state,
    step_count: stepCount, steps, ...(answer === undefined ? {} : { answer }) };
}

beforeAll(async () => {
  const styles = Object.assign(document.createElement("link"), { rel: "stylesheet", href: "/src/writing-workspace.css" });
  const loaded = new Promise((resolve, reject) => { styles.onload = resolve; styles.onerror = reject; });
  document.head.append(styles);
  await loaded;
});

/**
 * Renders the panel for the current Run `run` with `earlier` as one history turn when it is given. Each read of the
 * Run gives the next item of `reads`, and each read of a Subrun gives the next item of its list in `records`; the last
 * item stays after that. `tick` fires each refresh that the panel and the drawer scheduled.
 */
async function renderPanel(run: string, reads: readonly object[],
  { earlier, records = {} }: { earlier?: object; records?: Record<string, readonly (object | null)[]> } = {}) {
  seedProseRequest({ scope, message: "请检查第一章到第五章。", chapterId: CHAPTER, correlationId: id(0x12),
    idempotencyKey: id(0x13), snapshotId: id(0x14), runId: run, conversationId: agentRunFixture.conversation_id,
    ...(earlier === undefined ? {} : { history: [{ requestId: id(0x15), chapterId: CHAPTER,
      conversationId: agentRunFixture.conversation_id, runId: EARLIER, run: earlier, message: "先通读前文。",
      status: "completed", result: ANSWER }] }) });
  const requests: string[] = [];
  const unexpected: string[] = [];
  const counts = new Map<string, number>();
  const next = <T,>(key: string, list: readonly T[]) => {
    const count = (counts.get(key) ?? 0) + 1;
    counts.set(key, count);
    return list[Math.min(count, list.length) - 1];
  };
  const routes = new Map<string, () => Response>([
    [`GET ${project}/assistance`, () => jsonResponse({ ...assistanceFixture, project_scope: scope })],
    [`GET ${project}/skills`, () => jsonResponse({ ...skillsFixture, project_scope: scope, skills: [] })],
    // A completed Run reads its research sources. This Run has none.
    [`GET ${project}/agent-runs/${run}/research-sources`, () => jsonResponse({ ...sourcesFixture,
      project_scope: scope, run_id: run, snapshots: [], gaps: [], unknown_calls: [] })],
    [`GET ${project}/agent-runs/${run}`, () => jsonResponse({ ...next(run, reads), run_id: run })],
    ...Object.entries(records).map(([subrun, list]) =>
      // A `null` record is a failed read.
      [`GET ${project}/subruns/${subrun}`, () => {
        const body = next(subrun, list);
        return body === null ? jsonResponse({}, 503) : jsonResponse(body);
      }] as const),
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
  const current = () => panel.host.querySelector("[data-assistant-current-run]");
  return {
    panel, requests, unexpected, current,
    cards: (turn: Element | null = current()) => [...turn?.querySelectorAll("[data-assistant-subrun]") ?? []],
    drawer: () => document.querySelector("[data-subrun-drawer]"),
    async click(element: Element | undefined | null) {
      await act(async () => { (element as HTMLElement).click(); });
      await settle();
    },
    async key(key: string) {
      await act(async () => { document.activeElement?.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true })); });
      await settle();
    },
    async tick() {
      const refreshes = scheduled.splice(0);
      await act(async () => { for (const refresh of refreshes) refresh(); });
      await settle();
    },
  };
}

/** Line 1, line 2, and the state attribute of each card. */
function shownCards(cards: Element[]) {
  return cards.map(card => [card.querySelector(".assistant-subrun-state")?.textContent,
    card.querySelector(".assistant-subrun-title")?.textContent, card.getAttribute("data-subrun-state")]);
}

/** The heading, the state text, and the text of each section of the drawer. */
function shownDrawer(drawer: Element | null) {
  return [drawer?.querySelector("h2")?.textContent, drawer?.querySelector("[data-subrun-drawer-state]")?.textContent,
    ...[...drawer?.querySelectorAll("section") ?? []].map(section =>
      [...section.children].map(child => child.textContent))];
}

// ADR 0103 section 3A: while the Run works, the cards are below the status line and the plan list; after the end,
// they are above the first result. The marks of the four states differ in shape, not in colour alone.
it("shows one card for each state below the status line and the plan, and above the first result after the end",
  async () => {
    const withPlan = await renderPanel(RUNS[0]!, [read("claimed", { plan: PLAN, subruns: FOUR }),
      read("completed", { plan: PLAN, subruns: FOUR })]);
    try {
      const working = withPlan.current();
      // The width of the assistant panel, so that the long title does not fit on one line.
      withPlan.panel.host.style.width = "360px";
      const marks = withPlan.cards().map(card => {
        const style = getComputedStyle(card.querySelector(".assistant-subrun-mark")!);
        return [style.borderRadius, style.transform, style.borderStyle, style.backgroundColor !== "rgba(0, 0, 0, 0)"]
          .join(" ");
      });
      const title = withPlan.cards()[0]!.querySelector<HTMLElement>(".assistant-subrun-title")!;
      const during = [shownCards(withPlan.cards()), new Set(marks).size,
        working?.querySelector("[data-assistant-run-status]")?.nextElementSibling?.hasAttribute("data-assistant-plan"),
        working?.querySelector("[data-assistant-plan]")?.nextElementSibling?.className,
        title.scrollWidth > title.clientWidth, getComputedStyle(title).textOverflow];
      await withPlan.tick();
      const list = withPlan.current()?.querySelector(".assistant-subruns");
      const after = [shownCards(withPlan.cards()), list?.previousElementSibling?.hasAttribute("data-assistant-plan"),
        list?.nextElementSibling?.textContent, withPlan.current()?.querySelector("[data-assistant-run-status]") ?? null];

      expect([during, after, withPlan.unexpected]).toEqual([
        [FOUR.map(({ state, title: text }) => [LINES[state], text, state]), 4, true, "assistant-subruns", true,
          "ellipsis"],
        [FOUR.map(({ state, title: text }) => [LINES[state], text, state]), true, ANSWER, null], []]);
    } finally {
      await withPlan.panel.close();
    }
    const noPlan = await renderPanel(RUNS[1]!, [read("claimed", { subruns: FOUR })]);
    try {
      const status = noPlan.current()?.querySelector("[data-assistant-run-status]");

      expect([status?.nextElementSibling?.className, shownCards(noPlan.cards()), noPlan.unexpected])
        .toEqual(["assistant-subruns", FOUR.map(({ state, title: text }) => [LINES[state], text, state]), []]);
    } finally {
      await noPlan.panel.close();
    }
  });

// The cards keep the order of the read, and an earlier turn shows its cards above its results.
it("shows two cards in the order of the read, and the cards of an earlier turn above its results", async () => {
  const two = [entry(0x24, "completed", "查名字"), entry(0x25, "working", "查时间")];
  const earlier = { ...read("completed", { subruns: [entry(0x26, "completed", "通读前五章")] }), run_id: EARLIER };
  const shown = await renderPanel(RUNS[2]!, [read("claimed", { subruns: two })], { earlier });
  try {
    const turn = shown.panel.host.querySelector(`[data-assistant-history-run="${EARLIER}"]`);
    const list = turn?.querySelector(".assistant-subruns");

    expect([shownCards(shown.cards()), shownCards(shown.cards(turn)), list?.nextElementSibling?.textContent,
      shown.unexpected]).toEqual([
      [[LINES.completed, "查名字", "completed"], [LINES.working, "查时间", "working"]],
      [[LINES.completed, "通读前五章", "completed"]], ANSWER, []]);
  } finally {
    await shown.panel.close();
  }
});

// The drawer has the heading, the state text, the brief, one line for each entry form (two `chapter_read` entries that
// follow one another are one line), and the answer of a completed Subrun. It closes in three ways, and the focus goes
// into the drawer and back to the card.
it("opens the drawer with the record, and closes it with the button, the dim layer, and Escape", async () => {
  const subrun = id(0x27);
  const steps = [{ kind: "text", text: "先找名字。" },
    { kind: "chapter_read", chapter_id: id(0x30), title: "渡口" },
    { kind: "chapter_read", chapter_id: id(0x31), title: "夜航" },
    { kind: "keyword_search", query: "林舟", matches: 3 },
    { kind: "hybrid_search", question: "谁在等船？", passages: 2 },
    { kind: "chapter_read", chapter_id: id(0x32), title: "归途" },
    { kind: "memory_read", title: "林舟" },
    { kind: "memory_search", query: "渡口", entries: 4 }];
  const shown = await renderPanel(RUNS[3]!, [read("completed", { subruns: [entry(0x27, "completed")] })],
    { records: { [subrun]: [record(subrun, "completed", 7, steps, "林舟只有一种写法。")] } });
  try {
    const card = shown.cards()[0]!;
    const closed: unknown[] = [];
    let opened: unknown[] = [];
    for (const close of [
      () => shown.click(shown.drawer()?.querySelector("button")),
      () => shown.click(document.querySelector("[data-subrun-drawer-layer]")),
      () => shown.key("Escape"),
    ]) {
      await shown.click(card);
      const drawer = shown.drawer();
      opened = [drawer?.getAttribute("role"), drawer?.getAttribute("aria-label"), shownDrawer(drawer),
        drawer?.contains(document.activeElement), drawer?.querySelectorAll("button, a, input, textarea").length,
        drawer?.querySelector("button")?.textContent];
      await close();
      closed.push([shown.drawer(), document.querySelector("[data-subrun-drawer-layer]"), document.activeElement === card]);
    }

    expect([opened, closed, shown.unexpected]).toEqual([
      ["dialog", "子任务记录", ["子任务：通读前文", "已完成 · 7 步", ["指令", BRIEF],
        ["过程", "先找名字。", "读取 渡口、夜航", "检索 “林舟” — 3 处", "检索 “谁在等船？” — 2 处", "读取 归途",
          "查看记忆 林舟", "检索记忆 “渡口” — 4 条"], ["结论", "林舟只有一种写法。"]], true, 1, "关闭"],
      Array(3).fill([null, null, true]), []]);
  } finally {
    await shown.panel.close();
  }
});

// The state text of each state. A working Subrun names its RunStep in work, `step_count` + 1, at most 12. A Subrun
// that is not `completed` has no section `结论`.
it("gives the state text of each state, with the RunStep in work of a working Subrun", async () => {
  const cases = [["working", 0, "进行中 · 第 1 步"], ["working", 4, "进行中 · 第 5 步"], ["working", 12, "进行中 · 第 12 步"],
    ["no_result", 3, "没有结论"], ["stopped", 2, "已停止"]] as const;
  const shownStates: unknown[] = [];
  for (const [index, [state, stepCount]] of cases.entries()) {
    const subrun = id(0x40 + index);
    const shown = await renderPanel(id(0x50 + index),
      [read("claimed", { subruns: [{ subrun_id: subrun, title: "通读前五章", state }] })],
      { records: { [subrun]: [record(subrun, state, stepCount, [])] } });
    try {
      await shown.click(shown.cards()[0]);
      shownStates.push([shownDrawer(shown.drawer())[1], shown.drawer()?.querySelectorAll("section").length]);
      expect(shown.unexpected).toEqual([]);
    } finally {
      await shown.click(shown.drawer()?.querySelector("button"));
      await shown.panel.close();
    }
  }

  expect(shownStates).toEqual(cases.map(([, , text]) => [text, 2]));
});

// The open drawer of a working Subrun reads again at each refresh and shows the new step while it stays open. A close
// cancels the scheduled read. A refresh that gives `completed` changes the open drawer, and the next refresh reads no
// record.
it("reads the record again while the Subrun works, and stops after the end and after the close", async () => {
  const subrun = id(0x28);
  const run = RUNS[4]!;
  const first = [{ kind: "text", text: "先读第一章。" }];
  const second = [...first, { kind: "chapter_read", chapter_id: id(0x33), title: "渡口" }];
  const third = [...second, { kind: "memory_read", title: "林舟" }];
  const shown = await renderPanel(run, [read("claimed", { subruns: [entry(0x28, "working")] })],
    { records: { [subrun]: [record(subrun, "working", 0, first), record(subrun, "working", 1, second),
      record(subrun, "working", 2, third), record(subrun, "completed", 3, third, "灯在渡口。")] } });
  try {
    const shownRecord = () => shownDrawer(shown.drawer()).slice(1, 4);
    // The requests that `step` sends, in the order of their paths: the order of two refreshes is not a contract.
    const sent = async (step: () => Promise<void>) => {
      const before = shown.requests.length;
      await step();
      return shown.requests.slice(before).sort();
    };
    const closeDrawer = () => shown.click(shown.drawer()?.querySelector("button"));
    const perStep = [await sent(shown.tick), await sent(() => shown.click(shown.cards()[0]))];
    const records = [shownRecord()];
    perStep.push(await sent(shown.tick));
    records.push(shownRecord());
    // The working drawer has a scheduled read when it closes.
    perStep.push(await sent(closeDrawer), await sent(shown.tick), await sent(() => shown.click(shown.cards()[0])));
    records.push(shownRecord());
    perStep.push(await sent(shown.tick));
    records.push(shownRecord());
    perStep.push(await sent(shown.tick));
    const readRun = `GET ${project}/agent-runs/${run}`;
    const readSubrun = `GET ${project}/subruns/${subrun}`;

    expect([perStep, records, shown.unexpected]).toEqual([
      [[readRun], [readSubrun], [readRun, readSubrun], [], [readRun], [readSubrun], [readRun, readSubrun], [readRun]],
      [["进行中 · 第 1 步", ["指令", BRIEF], ["过程", "先读第一章。"]],
        ["进行中 · 第 2 步", ["指令", BRIEF], ["过程", "先读第一章。", "读取 渡口"]],
        ["进行中 · 第 3 步", ["指令", BRIEF], ["过程", "先读第一章。", "读取 渡口", "查看记忆 林舟"]],
        ["已完成 · 3 步", ["指令", BRIEF], ["过程", "先读第一章。", "读取 渡口", "查看记忆 林舟"]]],
      []]);
  } finally {
    await shown.panel.close();
  }
});

// The close button is the only control of the drawer: after a click on the text of the drawer, Tab and Shift+Tab
// keep the focus on it, so no card behind the dim layer gets the focus.
it("keeps the focus in the drawer after a click on its text", async () => {
  const subrun = id(0x2e);
  const shown = await renderPanel(RUNS[8]!, [read("completed", { subruns: [entry(0x2e, "completed"),
    entry(0x2f, "completed")] })], { records: { [subrun]: [record(subrun, "completed", 1, [], "灯在渡口。")] } });
  try {
    await shown.click(shown.cards()[0]);
    const close = shown.drawer()?.querySelector("button");
    const focused: unknown[] = [];
    await userEvent.click(shown.drawer()!.querySelector("section p")!);
    focused.push(shown.drawer()?.contains(document.activeElement));
    await userEvent.tab();
    focused.push(document.activeElement === close);
    await userEvent.tab({ shift: true });
    focused.push(document.activeElement === close);

    expect([focused, document.querySelectorAll("[data-subrun-drawer]").length, shown.unexpected])
      .toEqual([[true, true, true], 1, []]);
  } finally {
    await shown.panel.close();
  }
});

// After a failed read, the last record stays. The drawer reads again at the next refresh while the card says
// `working`, and only at the next open when the card says another state.
it("keeps the last record after a failed read, and reads again only while the card says working", async () => {
  const working = id(0x29);
  const first = [{ kind: "text", text: "先读第一章。" }];
  const second = [...first, { kind: "chapter_read", chapter_id: id(0x34), title: "渡口" }];
  const shown = await renderPanel(RUNS[6]!, [read("claimed", { subruns: [entry(0x29, "working")] })],
    { records: { [working]: [record(working, "working", 0, first), null, record(working, "working", 1, second)] } });
  const readSubrun = (subrun: string) => `GET ${project}/subruns/${subrun}`;
  try {
    await shown.click(shown.cards()[0]);
    const shownWorking = [shownDrawer(shown.drawer()).slice(1)];
    await shown.tick();
    // The second read fails. The drawer has a record, so it keeps the state text and shows no sentence.
    shownWorking.push(shownDrawer(shown.drawer()).slice(1));
    await shown.tick();
    shownWorking.push(shownDrawer(shown.drawer()).slice(1));

    expect([shownWorking, shown.requests.filter(request => request === readSubrun(working)).length, shown.unexpected])
      .toEqual([[["进行中 · 第 1 步", ["指令", BRIEF], ["过程", "先读第一章。"]],
        ["进行中 · 第 1 步", ["指令", BRIEF], ["过程", "先读第一章。"]],
        ["进行中 · 第 2 步", ["指令", BRIEF], ["过程", "先读第一章。", "读取 渡口"]]], 3, []]);
  } finally {
    await shown.panel.close();
  }
  const ended = id(0x2a);
  const done = await renderPanel(RUNS[7]!, [read("completed", { subruns: [entry(0x2a, "completed")] })],
    { records: { [ended]: [null, record(ended, "completed", 1, first, "灯在渡口。")] } });
  try {
    await done.click(done.cards()[0]);
    const failed = [shownDrawer(done.drawer()), done.drawer()?.querySelectorAll("button").length];
    const before = done.requests.length;
    await done.tick();
    const afterTick = done.requests.slice(before);
    await done.click(done.drawer()?.querySelector("button"));
    await done.click(done.cards()[0]);

    expect([failed, afterTick, shownDrawer(done.drawer()).slice(1),
      done.requests.filter(request => request === readSubrun(ended)).length, done.unexpected]).toEqual([
      [["子任务：通读前文", UNREADABLE], 1], [],
      ["已完成 · 1 步", ["指令", BRIEF], ["过程", "先读第一章。"], ["结论", "灯在渡口。"]], 2, []]);
  } finally {
    await done.panel.close();
  }
});

// A failed read with no record shows the sentence of the panel in the place of the state text, and no section. The
// next read that gives a record removes the sentence.
it("shows the sentence for a failed read with no record until a read gives the record", async () => {
  const subrun = id(0x2e);
  const shown = await renderList(subrun, [null, record(subrun, "working", 0, [{ kind: "text", text: "先读第一章。" }])]);
  try {
    await shown.show([entry(0x2e, "working")]);
    await shown.open();
    const failed = [shownDrawer(shown.drawer()), shown.drawer()?.querySelectorAll("button").length];
    await shown.tick();

    expect([failed, shownDrawer(shown.drawer()), shown.requests, shown.unexpected]).toEqual([
      [["子任务：通读前文", UNREADABLE], 1],
      ["子任务：通读前文", "进行中 · 第 1 步", ["指令", BRIEF], ["过程", "先读第一章。"]],
      Array(2).fill(`GET ${project}/subruns/${subrun}`), []]);
  } finally {
    await shown.panel.close();
  }
});

/**
 * Renders the Subrun list of `run` by itself, so that a test changes the card with `show`. Each read of the Subrun
 * gives the next item of `records`, and a `null` item is a failed read. `tick` fires each scheduled refresh.
 */
async function renderList(subrunId: string, records: readonly (object | null)[]) {
  const requests: string[] = [];
  const unexpected: string[] = [];
  const scheduled: Array<() => void> = [];
  let reads = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const request = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    requests.push(request);
    if (request !== `GET ${project}/subruns/${subrunId}`) {
      unexpected.push(request);
      throw new Error(`unexpected request: ${request}`);
    }
    reads += 1;
    const body = records[Math.min(reads, records.length) - 1];
    return body === null ? jsonResponse({}, 503) : jsonResponse(body);
  };
  const context = { scope, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto };
  const scheduleRefresh = (refresh: () => void) => {
    scheduled.push(refresh);
    return () => {
      const index = scheduled.indexOf(refresh);
      if (index >= 0) scheduled.splice(index, 1);
    };
  };
  const panel = openPanelHost();
  const settle = async () => {
    for (let turn = 0; turn < 5; turn += 1) await act(async () => { await Promise.resolve(); });
  };
  const show = async (subruns: object[]) => {
    await act(async () => {
      panel.root.render(createElement(SubrunList, { run: { ...read("claimed"), subruns } as never, context,
        scheduleRefresh }));
    });
    await settle();
  };
  return {
    panel, requests, unexpected, show, scheduled,
    drawer: () => document.querySelector("[data-subrun-drawer]"),
    async open() {
      await act(async () => { panel.host.querySelector<HTMLElement>("[data-assistant-subrun]")!.click(); });
      await settle();
    },
    async tick() {
      const refreshes = scheduled.splice(0);
      await act(async () => { for (const refresh of refreshes) refresh(); });
      await settle();
    },
  };
}

// A read again after a failed read waits for the next refresh. When the card no longer says `working` at that
// refresh, the drawer sends no read.
it("sends no read again after a failed read when the card changed before the refresh", async () => {
  const subrun = id(0x2b);
  const shown = await renderList(subrun, [record(subrun, "working", 0, []), null]);
  try {
    await shown.show([entry(0x2b, "working")]);
    await shown.open();
    await shown.tick();
    const retry = shown.scheduled.length;
    await shown.show([entry(0x2b, "completed")]);
    await shown.tick();

    expect([retry, shown.requests, shownDrawer(shown.drawer())[1], shown.unexpected])
      .toEqual([1, Array(2).fill(`GET ${project}/subruns/${subrun}`), "进行中 · 第 1 步", []]);
  } finally {
    await shown.panel.close();
  }
});

// A Run that no longer has the opened Subrun closes the drawer and cancels its scheduled read.
it("closes the drawer when the Run no longer has the opened Subrun", async () => {
  const subrun = id(0x2c);
  const shown = await renderList(subrun, [record(subrun, "working", 0, [])]);
  try {
    await shown.show([entry(0x2c, "working")]);
    await shown.open();
    const scheduled = shown.scheduled.length;
    await shown.show([entry(0x2d, "working")]);

    expect([scheduled, shown.drawer(), document.querySelector("[data-subrun-drawer-layer]"), shown.scheduled.length,
      shown.requests, shown.unexpected]).toEqual([1, null, null, 0, [`GET ${project}/subruns/${subrun}`], []]);
  } finally {
    await shown.panel.close();
  }
});

// A Run with no Subrun shows no card and no empty element.
it("shows no card for a Run with no Subrun", async () => {
  const shown = await renderPanel(RUNS[5]!, [read("claimed", { plan: PLAN }), read("completed", { plan: PLAN })]);
  try {
    const working = [shown.cards().length, shown.current()?.querySelector(".assistant-subruns") ?? null];
    await shown.tick();

    expect([working, shown.cards().length, shown.current()?.querySelector(".assistant-subruns") ?? null,
      shown.unexpected]).toEqual([[0, null], 0, null, []]);
  } finally {
    await shown.panel.close();
  }
});
