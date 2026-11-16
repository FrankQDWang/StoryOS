import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import challengeFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-project-command-challenge.json";
import conversationFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-conversation.json";
import createFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-agent-run.json";
import listFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-project-conversations.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import treeFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-manuscript-tree.json";
import type { AgentRunStatus, CreateAgentRunRequest, ProjectConversationEntry, SkillUseInspect }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

// ADR 0094 section 10: the author opens an earlier Project Conversation in the panel and continues it.
const scope = { owner_user_id: OWNER, project_id: PROJECT };
const id = (suffix: string) => `018f0000-0000-7001-8000-000000000${suffix}`;
const [A, B, C] = [id("f01"), id("f02"), id("f03")];
const EXPIRED = "这段对话已超过 72 小时，助手可能不记得之前的内容。";
const NOT_FOUND = "找不到这段对话。";
const SKILL: SkillUseInspect = { provenance: "author_selected", name: "dialogue-style-guide" };

type StoredRun = { runId: string; conversationId: string; message: string; status: AgentRunStatus; uses?: SkillUseInspect[] };
type Setup = {
  runs?: StoredRun[];
  /** The Run of this browser session that `sessionStorage` holds. */
  stored?: StoredRun;
  list?: ProjectConversationEntry[] | "failed" | "other_scope";
  truncated?: boolean;
  expired?: boolean;
  /** A change of each Conversation read or Run read, as a response of another Scope. */
  conversationOverride?: Record<string, unknown>;
  runOverride?: Record<string, unknown>;
  requested?: string;
};

const entry = (conversationId: string, label: string, newest: string): ProjectConversationEntry => ({
  conversation_id: conversationId, label, first_run_created_at: "2026-11-16T09:00:00.000Z",
  newest_run_created_at: newest, newest_run_status: "completed" });

let cleanup: Array<() => Promise<void>> = [];
afterEach(async () => {
  for (const step of cleanup) await step();
  cleanup = [];
  sessionStorage.clear();
});

/** Opens the panel on a fake Server that holds `runs`, the oldest first. */
async function openPanel(setup: Setup) {
  const runs = [...setup.runs ?? []];
  if (setup.stored !== undefined) {
    runs.push(setup.stored);
    sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
      scope, message: setup.stored.message, chapterId: CHAPTER, correlationId: id("f91"), idempotencyKey: id("f92"),
      snapshotId: id("f93"), runId: setup.stored.runId, conversationId: setup.stored.conversationId }));
  }
  const requests: string[] = [];
  const creates: CreateAgentRunRequest[] = [];
  const scheduled: Array<() => void> = [];
  const runRead = (run: StoredRun) => ({ ...agentRunFixture, project_scope: scope,
    run_id: run.runId, conversation_id: run.conversationId, status: run.status, results: [],
    decision: ["completed", "refused", "cancelled"].includes(run.status)
      ? { kind: "advisory", decision_id: id("f99"), selected: true, text: `Answer to ${run.message}`,
        continuation: { kind: "absent" } } : { kind: "absent" },
    ...run.uses === undefined ? {} : { skill_uses: run.uses }, ...setup.runOverride });
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const route = path.replace(`/api/v1/projects/${PROJECT}`, "");
    if (route === "/assistance") return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (route === "/skills") return jsonResponse({ ...skillsFixture, project_scope: scope, skills: [] });
    if (route === "/manuscript/tree") return jsonResponse({ ...treeFixture, project_scope: scope,
      snapshot: { ...treeFixture.snapshot, project_scope: scope } });
    // The current exchange reads its research sources, as today.
    if (route.endsWith("/research-sources")) return jsonResponse({ code: "not_found" }, 404);
    requests.push(`${init?.method ?? "GET"} ${route}`);
    if (route === "/conversations") {
      if (setup.list === "failed") return jsonResponse({ code: "service_unavailable" }, 503);
      return jsonResponse({ ...listFixture, conversations: Array.isArray(setup.list) ? setup.list : [],
        completeness: setup.truncated === true ? "truncated" : "complete",
        project_scope: setup.list === "other_scope" ? { ...scope, project_id: id("f98") } : scope });
    }
    if (route.startsWith("/conversations/")) {
      const conversationId = route.split("/").at(-1)!;
      const rows = runs.filter((run) => run.conversationId === conversationId);
      if (rows.length === 0) return jsonResponse({ code: "not_found" }, 404);
      return jsonResponse({ ...conversationFixture, project_scope: scope, conversation_id: conversationId,
        completeness: setup.truncated === true ? "truncated" : "complete", earlier_turns_expired: setup.expired === true,
        runs: rows.map((run) => ({ run_id: run.runId, author_message: run.message, chapter_id: CHAPTER,
          status: run.status, created_at: "2026-11-16T09:30:00.000Z" })), ...setup.conversationOverride });
    }
    if (route.startsWith("/agent-runs/")) return jsonResponse(runRead(runs.find((run) => run.runId === route.split("/").at(-1))!));
    if (route === "/anti-forgery-challenges") return jsonResponse(challengeFixture);
    // createAgentRun: the Host admits the Run in the Conversation that the request names, or in Conversation C.
    const request = JSON.parse(String(init?.body)) as CreateAgentRunRequest;
    creates.push(request);
    const conversation = request.create_agent_run_input.conversation;
    const created: StoredRun = { runId: id(`f2${creates.length}`), message: request.create_agent_run_input.author_message.text,
      conversationId: conversation.kind === "existing" ? conversation.conversation_id : C, status: "completed" };
    runs.push(created);
    return jsonResponse({ ...createFixture, project_scope: scope, conversation_id: created.conversationId,
      effect: { ...createFixture.effect, run_id: created.runId, conversation_id: created.conversationId } }, 202);
  };
  const host = document.createElement("div");
  document.body.append(host);
  let root: Root | undefined;
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  const settle = async () => {
    for (let turn = 0; turn < 40; turn += 1) await act(async () => { await Promise.resolve(); });
  };
  const mount = async (requested?: string) => {
    root = createRoot(host);
    await act(async () => {
      root!.render(createElement(WritingAssistantPanel, {
        collapsed: false, requestedConversationId: requested,
        scheduleRefresh: (refresh) => {
          scheduled.push(refresh);
          return () => { scheduled.splice(scheduled.indexOf(refresh) >>> 0, 1); };
        },
        context: { scope, chapterId: CHAPTER, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto },
      }));
    });
    await settle();
  };
  cleanup.push(async () => {
    await act(async () => root?.unmount());
    host.remove();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  });
  await mount(setup.requested);
  const click = async (selector: string) => {
    await act(async () => { host.querySelector<HTMLElement>(selector)!.click(); });
    await settle();
  };
  const panel = {
    requests, creates, runs, click,
    historyButton: () => host.querySelector<HTMLButtonElement>("[data-assistant-conversation-history]")!,
    openList: () => click("[data-assistant-conversation-history]"),
    open: async (conversationId: string) => {
      await panel.openList();
      await click(`[data-assistant-conversation-entry="${conversationId}"]`);
    },
    send: async (text: string) => {
      host.querySelector("textarea")!.focus();
      await applyTrustedInput({ operation: "insert_text", text });
      await settle();
      await act(async () => { host.querySelector<HTMLButtonElement>("button.send-button")!.click(); });
      await settle();
    },
    refresh: async () => {
      await act(async () => { scheduled.shift()!(); });
      await settle();
    },
    reload: async () => {
      await act(async () => root?.unmount());
      await mount();
    },
    list: () => {
      const list = host.querySelector("[data-assistant-conversation-list]");
      return list === null ? null : {
        entries: [...list.querySelectorAll("[data-assistant-conversation-entry]")].map((button) => [
          button.querySelector(".assistant-conversation-label")?.textContent,
          button.querySelector("time")?.getAttribute("datetime"), button.getAttribute("aria-current")]),
        controls: list.querySelectorAll("button, input, a").length,
        lines: [...list.querySelectorAll("p")].map((line) => line.textContent),
      };
    },
    /** Each exchange as its author message, its result, its labels, and its notice lines. */
    exchanges: () => [...host.querySelectorAll("[data-assistant-history-run], [data-assistant-current-run]")]
      .map((section) => [...section.querySelectorAll(".assistant-author-message, .assistant-result, "
        + "[data-sent-reference-label], [data-sent-skill-label], [data-assistant-earlier-hidden], "
        + "[data-assistant-earlier-expired]")].map((line) => line.textContent)),
    status: () => host.querySelector("[role=status]")?.textContent ?? null,
    runStatus: () => host.querySelector("[data-assistant-run-status]")?.textContent ?? null,
  };
  return panel;
}

const run = (runId: string, conversationId: string, message: string, extra: Partial<StoredRun> = {}): StoredRun =>
  ({ runId, conversationId, message, status: "completed", ...extra });
const earlier = [run(id("f11"), A, "A first", { uses: [SKILL] }), run(id("f12"), A, "A second")];
const current = run(id("f13"), B, "B first");

it("lists the Project Conversations each time the list opens, and the list has no other control", async () => {
  const list = [entry(B, "B first", "2026-11-16T10:15:00.000Z"), entry(A, "A first", "2026-11-16T09:45:00.000Z")];
  const setup: Setup = { runs: earlier, stored: current, list, truncated: true };
  const panel = await openPanel(setup);
  panel.requests.length = 0;
  await panel.openList();
  const shown = panel.list();
  await panel.openList();
  const closed = panel.list();
  setup.list = [];
  setup.truncated = false;
  await panel.openList();
  const empty = panel.list();
  await panel.openList();
  setup.list = "failed";
  await panel.openList();
  const failed = { list: panel.list(), status: panel.status() };
  await panel.openList();
  setup.list = "other_scope";
  await panel.openList();

  expect({ shown, closed, empty, failed, otherScope: panel.list(), requests: panel.requests,
    usable: !panel.historyButton().disabled && !document.querySelector("textarea")!.disabled }).toEqual({
    shown: { entries: [["B first", "2026-11-16T10:15:00.000Z", "true"], ["A first", "2026-11-16T09:45:00.000Z", null]],
      controls: 2, lines: ["只显示最近的对话。"] },
    closed: null,
    empty: { entries: [], controls: 0, lines: ["还没有对话。"] },
    failed: { list: { entries: [], controls: 0, lines: ["无法读取历史对话，请稍后再试。"] },
      status: "无法读取历史对话，请稍后再试。" },
    otherScope: { entries: [], controls: 0, lines: ["无法读取历史对话，请稍后再试。"] },
    requests: ["GET /conversations", "GET /conversations", "GET /conversations", "GET /conversations"],
    // The panel stays usable after a failed read.
    usable: true,
  });
});

it.each([true, false])("opens an earlier Conversation with only its exchanges, the oldest first (expired %s)",
  async (expired) => {
    const panel = await openPanel({ runs: earlier, stored: current, list: [entry(A, "A first", "2026-11-16T09:45:00.000Z")],
      truncated: true, expired });
    panel.requests.length = 0;
    await panel.open(A);
    const opened = panel.exchanges();
    const requests = [...panel.requests];
    await panel.reload();

    const notice = expired ? [EXPIRED] : [];
    expect({ opened, requests, reloaded: panel.exchanges(), reloadRequests: panel.requests.slice(requests.length) })
      .toEqual({
        opened: [["更早的消息没有显示。", "A first", "$dialogue-style-guide", "Answer to A first"],
          ["A second", "Answer to A second", ...notice]],
        requests: ["GET /conversations", `GET /conversations/${A}`, `GET /agent-runs/${id("f11")}`,
          `GET /agent-runs/${id("f12")}`],
        reloaded: [["更早的消息没有显示。", "A first", "$dialogue-style-guide", "Answer to A first"],
          ["A second", "Answer to A second", ...notice]],
        // A reload reads the newest Run of the recorded Conversation, and not the Conversation again.
        reloadRequests: [`GET /agent-runs/${id("f12")}`],
      });
  });

it("continues the opened Conversation, and `新对话` then starts a new Conversation", async () => {
  const panel = await openPanel({ runs: earlier, stored: current, list: [entry(A, "A first", "2026-11-16T09:45:00.000Z")],
    expired: true });
  await panel.open(A);
  await panel.send("A third");
  const continued = panel.exchanges();
  await panel.click("[data-assistant-new-conversation]");
  await panel.send("C first");

  expect({ conversations: panel.creates.map((request) => request.create_agent_run_input.conversation),
    continued, last: panel.exchanges().at(-1) }).toEqual({
    conversations: [{ kind: "existing", conversation_id: A }, { kind: "new" }],
    continued: [["A first", "$dialogue-style-guide", "Answer to A first"], ["A second", "Answer to A second", EXPIRED],
      ["A third", "Answer to A third"]],
    last: ["C first", "Answer to C first"],
  });
});

it("follows the newest Run of an opened Conversation until it settles, with the controls of a Run in progress",
  async () => {
    const claimed = run(id("f14"), A, "A in progress", { status: "claimed" });
    const panel = await openPanel({ runs: [earlier[0]!, claimed], stored: current,
      list: [entry(A, "A first", "2026-11-16T09:45:00.000Z")] });
    await panel.open(A);
    const inProgress = { runStatus: panel.runStatus(), history: panel.historyButton().disabled,
      cancel: document.querySelector("[data-cancel-agent-run]") !== null,
      send: document.querySelector(".send-button")?.getAttribute("aria-label") };
    claimed.status = "completed";
    await panel.refresh();

    expect({ inProgress, settled: panel.exchanges().at(-1), history: panel.historyButton().disabled }).toEqual({
      inProgress: { runStatus: "正在处理", history: true, cancel: true, send: "暂停" },
      settled: ["A in progress", "Answer to A in progress"],
      history: false,
    });
  });

it("shows an empty panel in a new browser session and reads no Conversation", async () => {
  const panel = await openPanel({ runs: earlier });

  expect({ exchanges: panel.exchanges(), requests: panel.requests }).toEqual({ exchanges: [], requests: [] });
});

it.each([
  ["a Conversation of the Project", { requested: A }, [`GET /conversations/${A}`, `GET /agent-runs/${id("f11")}`,
    `GET /agent-runs/${id("f12")}`], null],
  ["a value that is not a UUID", { requested: "conversation-a" }, [], NOT_FOUND],
  ["a Conversation that the Host does not give", { requested: id("f04") }, [`GET /conversations/${id("f04")}`], NOT_FOUND],
  ["a response of another Project Scope", { requested: A,
    conversationOverride: { project_scope: { ...scope, project_id: id("f98") } } }, [`GET /conversations/${A}`], NOT_FOUND],
  ["a Run read of another Conversation", { requested: A, runOverride: { conversation_id: B } },
    [`GET /conversations/${A}`, `GET /agent-runs/${id("f11")}`], NOT_FOUND],
  ["a panel with a request that is not settled", { requested: A, stored: run(id("f15"), B, "B waits", { status: "queued" }) },
    [`GET /agent-runs/${id("f15")}`], NOT_FOUND],
] as const)("opens the Conversation of the address for %s, or gives one sentence", async (_, setup, requests, status) => {
  const panel = await openPanel({ runs: earlier, ...setup });
  const shown = panel.exchanges().filter((exchange) => exchange.some((line) => line?.startsWith("A")));

  expect({ requests: panel.requests, status: panel.status(), shown: shown.length }).toEqual({
    requests, status, shown: status === null ? 2 : 0 });
});
