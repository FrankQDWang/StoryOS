import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import challengeFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-project-command-challenge.json";
import createFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-agent-run.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import steerFixture from "../../../../generated/golden-wire/storyos-public-release-1/steer-agent-run.json";
import treeFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-manuscript-tree.json";
import { digestCreateAgentRun } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunRequest, SkillCatalogEntry, SkillUseInspect, SteerAgentRunRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import type { ChapterTextReference } from "../../src/selection-reference.ts";
import { addDraftReferences } from "../../src/selection-reference-draft.ts";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import { seedProseRequest } from "./assistant-panel-harness.ts";
import { BLOCK, CHAPTER, OWNER, PROJECT, REVISION, jsonResponse } from "./scenario.ts";

// ADR 0089 sections 5, 7, and 8: the author selects Skills with the $ picker, and no view shows a digest.
const scope = { owner_user_id: OWNER, project_id: PROJECT };
const CONVERSATION = agentRunFixture.conversation_id;
const RUN_IN_PROGRESS = "018f0000-0000-7001-8000-000000000c81";
const RUNS = ["018f0000-0000-7001-8000-000000000c82", "018f0000-0000-7001-8000-000000000c83"];
const UNAVAILABLE = "所选技能现在不可用，消息没有发送。请重新选择技能，或直接发送。";
const skill = (name: string, digit: string): SkillCatalogEntry => ({
  reference: { name, package_digest: `sha256:${digit.repeat(64)}` }, description: `${name} 的用法说明。` });
const DIALOGUE = skill("dialogue-style-guide", "a");
const COMMS = skill("internal-comms", "b");
const reference: ChapterTextReference = { kind: "chapter_text", chapter_id: CHAPTER,
  base_authoritative_revision_id: REVISION, coordinate_profile: "storyos.editor.utf16-code-unit.v1",
  start: { manuscript_block_id: BLOCK, offset: 0 }, end: { manuscript_block_id: BLOCK, offset: 4 } };
const REFERENCE_LABEL = "第 1 章 渡口（第 1 段）";
const addReference = () => addDraftReferences(scope, [{ reference, moved: false,
  label: { chapter: "第 1 章 渡口", part: { kind: "paragraphs", first: 1, last: 1 } } }]);

/** The create input that the panel sends for a message of a new Conversation. */
const createInput = (text: string, extra: Record<string, unknown>, correlationId: unknown) => ({
  conversation: { kind: "new" }, author_message: { text }, instruction: { kind: "absent" },
  cause: { kind: "author_request" },
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1", correlation_id: correlationId,
  working_target: { kind: "current_chapter", chapter_id: CHAPTER }, ...extra,
});

/** The body of one command or challenge request. */
type Post = { route: string;
  body: Partial<CreateAgentRunRequest & SteerAgentRunRequest> & { canonical_command_digest?: unknown } };
let cleanup: Array<() => Promise<void>> = [];
afterEach(async () => {
  for (const step of cleanup) await step();
  cleanup = [];
  sessionStorage.clear();
});

/**
 * Opens the panel on a fake Server. `skills` is the answer of listSkills, `claimed` stores a Run in
 * progress, and `create` gives the answer of each createAgentRun in order (the default admits the Run).
 */
async function openPanel(setup: { skills?: SkillCatalogEntry[] | "failed"; claimed?: boolean;
  create?: Array<"unavailable" | "refused">; uses?: SkillUseInspect[] } = {}) {
  if (setup.claimed === true) {
    seedProseRequest({
      scope, message: "Help with this passage.", chapterId: CHAPTER,
      correlationId: "018f0000-0000-7001-8000-000000000c84", idempotencyKey: "018f0000-0000-7001-8000-000000000c85",
      snapshotId: "018f0000-0000-7001-8000-000000000c86", runId: RUN_IN_PROGRESS, conversationId: CONVERSATION });
  }
  const posts: Post[] = [];
  const replies = [...setup.create ?? []];
  let skillReads = 0;
  let created = 0;
  const advisory = { kind: "advisory", decision_id: "018f0000-0000-7001-8000-000000000c87", selected: true,
    text: "Keep the second sentence.", continuation: { kind: "absent" } };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith("/skills")) {
      skillReads += 1;
      return setup.skills === "failed" ? jsonResponse({ code: "service_unavailable" }, 503)
        : jsonResponse({ ...skillsFixture, project_scope: scope, skills: setup.skills ?? [DIALOGUE, COMMS] });
    }
    if (path.endsWith("/manuscript/tree")) return jsonResponse({ ...treeFixture, project_scope: scope,
      snapshot: { ...treeFixture.snapshot, project_scope: scope } });
    if (init?.method === "GET") {
      const runId = path.split("/").at(-1)!;
      const inProgress = runId === RUN_IN_PROGRESS && !posts.some((post) => post.route === "steering-inputs");
      // The Run read of the first admitted Run gives `uses`.
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: runId,
        status: inProgress ? "claimed" : "completed", decision: inProgress ? { kind: "absent" } : advisory,
        ...runId === RUNS[0] && setup.uses !== undefined ? { skill_uses: setup.uses } : {} });
    }
    posts.push({ route: path.split("/").at(-1)!, body: JSON.parse(String(init?.body)) as Post["body"] });
    if (path.endsWith("/anti-forgery-challenges")) return jsonResponse(challengeFixture);
    if (path.endsWith("/steering-inputs")) return jsonResponse({ ...steerFixture, project_scope: scope,
      receipt: { ...steerFixture.receipt, project_scope: scope }, effect: { kind: "conflicted", reason: "terminal_run" } });
    const reply = replies.shift();
    if (reply === "unavailable") return jsonResponse({ code: "skill_selection_unavailable" }, 422);
    if (reply === "refused") return jsonResponse({ code: "invalid_request_shape" }, 422);
    const runId = RUNS[created++]!;
    return jsonResponse({ ...createFixture, project_scope: scope, effect: { ...createFixture.effect, run_id: runId } }, 202);
  };
  const host = document.createElement("div");
  document.body.append(host);
  let root: Root | undefined;
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  const settle = async () => {
    for (let turn = 0; turn < 40; turn += 1) await act(async () => { await Promise.resolve(); });
  };
  const mount = async () => {
    root = createRoot(host);
    await act(async () => {
      root!.render(createElement(WritingAssistantPanel, {
        collapsed: false, scheduleRefresh: () => () => {},
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
  await mount();
  const field = () => host.querySelector("textarea")!;
  const panel = {
    posts,
    skillReads: () => skillReads,
    /** Types with the trusted keyboard of the browser. */
    type: async (text: string) => {
      field().focus();
      await applyTrustedInput({ operation: "insert_text", text });
      await settle();
    },
    enter: async () => {
      field().focus();
      await applyTrustedInput({ operation: "enter" });
      await settle();
    },
    key: async (key: string, isComposing = false) => {
      await act(async () => {
        field().dispatchEvent(new KeyboardEvent("keydown", { key, isComposing, bubbles: true, cancelable: true }));
      });
    },
    click: async (selector: string) => {
      await act(async () => {
        const target = host.querySelector<HTMLElement>(selector)!;
        target.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
        target.click();
      });
      await settle();
    },
    send: async () => {
      await act(async () => { host.querySelector<HTMLButtonElement>("button.send-button")!.click(); });
      await settle();
    },
    /** A reload keeps `sessionStorage` and starts a new page. */
    reload: async () => {
      await act(async () => root?.unmount());
      await mount();
    },
    /** The outcome sentence and the Skill lines of each turn, each Skill list as its label and the number of
     * elements in it that are not a line, and whether a digest shows. */
    skillTurns: () => ({
      turns: [...host.querySelectorAll("[data-assistant-history-run], [data-assistant-current-run]")].map((section) =>
        [...section.querySelectorAll(".assistant-result, [data-assistant-skill-uses] li")]
          .map((line) => line.textContent)),
      lists: [...host.querySelectorAll("[data-assistant-skill-uses]")].map((list) =>
        [list.getAttribute("aria-label"), list.querySelectorAll(":scope > :not(li), li *").length]),
      digestShown: panel.view().digestShown,
    }),
    view: () => {
      const picker = host.querySelector("[data-skill-picker]");
      const labels = (selector: string) => [...host.querySelectorAll(`${selector} [data-sent-reference-label], `
        + `${selector} [data-sent-skill-label]`)].map((label) => label.textContent);
      return {
        picker: picker === null ? null : {
          role: picker.getAttribute("role"), label: picker.getAttribute("aria-label"),
          options: [...picker.querySelectorAll("[role=option]")].map((option) =>
            [...option.children].map((part) => part.textContent)),
          active: picker.querySelector("[aria-selected=true]")?.id ?? null,
        },
        activeDescendant: field().getAttribute("aria-activedescendant"),
        focused: document.activeElement === field(),
        text: field().value,
        chips: [...host.querySelectorAll("[data-skill-chip]")].map((chip) =>
          [chip.querySelector(".reference-label")?.textContent, chip.querySelector("button")?.getAttribute("aria-label")]),
        references: host.querySelectorAll("[data-selection-reference-label]").length,
        current: host.querySelector("[data-assistant-current-run]") === null ? null : labels("[data-assistant-current-run]"),
        history: [...host.querySelectorAll("[data-assistant-history-run]")].map((section) =>
          [...section.querySelectorAll("[data-sent-reference-label], [data-sent-skill-label]")]
            .map((label) => label.textContent)),
        status: host.querySelector("[role=status]")?.textContent ?? null,
        digestShown: (host.textContent ?? "").includes("sha256:")
          || (host.textContent ?? "").includes("a".repeat(64)),
      };
    },
  };
  return panel;
}

it.each([
  ["a $ at the start of the message", {}, "$", true],
  ["a ￥ after white space", {}, "Help ￥", true],
  ["a $ after a letter", {}, "US$", false],
  ["an empty Skill list", { skills: [] }, "$", false],
  ["a failed read of the Skills", { skills: "failed" as const }, "$", false],
  ["a steering message for a Run in progress", { claimed: true }, "$", false],
])("opens the picker for %s: %s", async (_, setup, text, open) => {
  const panel = await openPanel(setup);
  await panel.type(text);

  expect({ open: panel.view().picker !== null, text: panel.view().text }).toEqual({ open, text });
});

it("lists the name and the description of each Skill and filters the entries by a part of the name", async () => {
  const panel = await openPanel();
  await panel.type("$");
  const all = panel.view();
  await panel.type("comm");
  const filtered = panel.view().picker;
  await panel.type("x");

  expect({ all: { picker: all.picker, activeDescendant: all.activeDescendant, digestShown: all.digestShown },
    filtered, none: panel.view().picker, reads: panel.skillReads() }).toEqual({
    all: { picker: { role: "listbox", label: "选择技能", active: "skill-option-dialogue-style-guide",
      options: [["dialogue-style-guide", DIALOGUE.description], ["internal-comms", COMMS.description]] },
    activeDescendant: "skill-option-dialogue-style-guide", digestShown: false },
    filtered: { role: "listbox", label: "选择技能", active: "skill-option-internal-comms",
      options: [["internal-comms", COMMS.description]] },
    none: null,
    reads: 1,
  });
});

it("moves the active entry with the arrow keys, takes no key during a composition, and selects with Enter", async () => {
  const panel = await openPanel();
  await panel.type("Help $");
  const steps: Array<[string | null, string | null]> = [];
  const active = () => steps.push([panel.view().picker?.active ?? null, panel.view().activeDescendant]);
  await panel.key("ArrowDown");
  active();
  await panel.key("ArrowDown", true);
  active();
  await panel.key("ArrowUp");
  active();
  await panel.key("ArrowUp");
  active();
  await panel.key("Enter");
  const selected = panel.view();
  await panel.type("$");

  expect({ steps, picker: selected.picker, text: selected.text, chips: selected.chips, focused: selected.focused,
    activeDescendant: selected.activeDescendant, left: panel.view().picker?.options }).toEqual({
    steps: [
      ["skill-option-internal-comms", "skill-option-internal-comms"],
      ["skill-option-internal-comms", "skill-option-internal-comms"],
      ["skill-option-dialogue-style-guide", "skill-option-dialogue-style-guide"],
      ["skill-option-internal-comms", "skill-option-internal-comms"],
    ],
    picker: null, text: "Help ", chips: [["$internal-comms", "移除技能"]], focused: true, activeDescendant: null,
    // A Skill with a chip is not in the picker.
    left: [["dialogue-style-guide", DIALOGUE.description]],
  });
});

it("closes the picker with Escape and keeps the text, selects with a click, and keeps Enter for a new line", async () => {
  const panel = await openPanel();
  await panel.type("Use $");
  await panel.key("Escape");
  const escaped = panel.view();
  await panel.type(" $");
  await panel.click("[data-skill-option=internal-comms]");
  const clicked = panel.view();
  await panel.enter();

  expect({ escaped: [escaped.picker, escaped.text], clicked: [clicked.text, clicked.chips, clicked.focused],
    closed: panel.view().text, posts: panel.posts }).toEqual({
    escaped: [null, "Use $"],
    clicked: ["Use $ ", [["$internal-comms", "移除技能"]], true],
    closed: "Use $ \n",
    posts: [],
  });
});

it("opens no picker with 8 chips, removes a chip, and drops the chips at a reload", async () => {
  const many = [..."123456789"].map((digit) => skill(`skill-${digit}`, digit));
  const panel = await openPanel({ skills: many });
  for (let count = 0; count < 8; count += 1) {
    await panel.type("$");
    await panel.key("Enter");
  }
  await panel.type("$");
  const full = panel.view();
  await panel.click("[data-skill-chip=skill-1] [data-remove-skill]");
  const removed = panel.view();
  await panel.reload();

  expect({ full: [full.picker, full.chips.length], removed: [removed.chips.length, removed.picker?.options],
    reloaded: panel.view().chips }).toEqual({
    full: [null, 8],
    removed: [7, [["skill-1", many[0]!.description], ["skill-9", many[8]!.description]]],
    reloaded: [],
  });
});

it("sends the Skill chips in skill_selections under the command digest and keeps their labels at a reload", async () => {
  const panel = await openPanel();
  addReference();
  await panel.type("Help $dia");
  await panel.key("Enter");
  await panel.type("with the dialogue.");
  await panel.send();
  const admitted = panel.view();
  await panel.reload();
  const reloaded = panel.view();
  // A $ with no chip selects nothing.
  await panel.type("Price $5 now.");
  await panel.send();
  const [challenge, first, , second] = panel.posts;
  const input = (post: Post | undefined) => post?.body.create_agent_run_input;

  expect({
    routes: panel.posts.map((post) => post.route),
    first: input(first),
    digest: challenge?.body.canonical_command_digest,
    second: input(second),
    admitted: [admitted.text, admitted.chips, admitted.current],
    reloaded: [reloaded.chips, reloaded.current],
    turns: [panel.view().history, panel.view().current],
    digestShown: [admitted.digestShown, reloaded.digestShown, panel.view().digestShown],
  }).toEqual({
    routes: ["anti-forgery-challenges", "agent-runs", "anti-forgery-challenges", "agent-runs"],
    first: createInput("Help with the dialogue.", { skill_selections: [DIALOGUE.reference],
      working_target: { kind: "selection_references", source_chapter_id: CHAPTER, references: [reference] } },
    input(first)?.correlation_id),
    digest: await digestCreateAgentRun(first!.body as CreateAgentRunRequest, crypto),
    second: createInput("Price $5 now.", { conversation: { kind: "existing", conversation_id: CONVERSATION } },
      input(second)?.correlation_id),
    admitted: ["", [], [REFERENCE_LABEL, "$dialogue-style-guide"]],
    reloaded: [[], [REFERENCE_LABEL, "$dialogue-style-guide"]],
    turns: [[[REFERENCE_LABEL, "$dialogue-style-guide"]], []],
    digestShown: [false, false, false],
  });
});

it("sends no Skill with a steering message or with its resend as a new Run", async () => {
  const panel = await openPanel({ claimed: true });
  await panel.type("Shorter $dia");
  const picker = panel.view().picker;
  await panel.send();
  const [, steer, , create] = panel.posts;

  expect({ picker, routes: panel.posts.map((post) => post.route), steer: steer?.body.steer_agent_run_input,
    create: create?.body.create_agent_run_input }).toEqual({
    picker: null,
    routes: ["anti-forgery-challenges", "steering-inputs", "anti-forgery-challenges", "agent-runs"],
    steer: { conversation_id: CONVERSATION, author_message: { text: "Shorter $dia" },
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: steer?.body.steer_agent_run_input?.correlation_id },
    create: createInput("Shorter $dia", { conversation: { kind: "existing", conversation_id: CONVERSATION } },
      create?.body.create_agent_run_input?.correlation_id),
  });
});

it("keeps the message and its references and removes the Skill chips when the Server refuses the selection", async () => {
  const panel = await openPanel({ create: ["refused", "unavailable"] });
  addReference();
  await panel.type("$");
  await panel.key("Enter");
  await panel.type("Help.");
  await panel.send();
  const other = panel.view();
  await panel.send();
  const refused = panel.view();
  await panel.send();
  const creates = panel.posts.filter((post) => post.route === "agent-runs")
    .map((post) => post.body.create_agent_run_input?.skill_selections ?? null);

  expect({ other: [other.status, other.text, other.chips], refused, creates, reads: panel.skillReads() }).toEqual({
    // Each other refusal keeps its sentence of today and the chips.
    other: ["本次请求未被接收。请检查当前章节和写作助手状态。", "Help.", [["$dialogue-style-guide", "移除技能"]]],
    refused: { picker: null, activeDescendant: null, focused: true, text: "Help.", chips: [], references: 1,
      current: null, history: [], status: UNAVAILABLE, digestShown: false },
    creates: [[DIALOGUE.reference], [DIALOGUE.reference], null],
    reads: 2,
  });
});

it("names the Skills of a turn after its outcome sentence as plain text, and its history turn keeps them at a reload",
  async () => {
    const reason = "保持**对白**简短，参见 [指南](https://example.org/guide)。";
    const panel = await openPanel({ uses: [{ provenance: "author_selected", name: "dialogue-style-guide" },
      { provenance: "agent_selected", name: "internal-comms", reason }] });
    await panel.type("Help with the dialogue.");
    await panel.send();
    const current = panel.skillTurns();
    await panel.type("Next.");
    await panel.send();
    await panel.reload();
    const lines = ["Keep the second sentence.", "使用了你选择的技能「dialogue-style-guide」",
      `使用了技能「internal-comms」：${reason}`];

    // The second Run used no Skill, so its turn shows no list.
    expect({ current, reloaded: panel.skillTurns() }).toEqual({
      current: { turns: [lines], lists: [["本次使用的技能", 0]], digestShown: false },
      reloaded: { turns: [lines, ["Keep the second sentence."]], lists: [["本次使用的技能", 0]], digestShown: false },
    });
  });
