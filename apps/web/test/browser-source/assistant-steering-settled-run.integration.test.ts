import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import challengeFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-project-command-challenge.json";
import createFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-agent-run.json";
import treeFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-manuscript-tree.json";
import steerFixture from "../../../../generated/golden-wire/storyos-public-release-1/steer-agent-run.json";
import type { ChapterTextReference } from "../../src/selection-reference.ts";
import { addDraftReferences } from "../../src/selection-reference-draft.ts";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { BLOCK, CHAPTER, OWNER, PROJECT, REVISION, jsonResponse } from "./scenario.ts";

const RUN = "018f0000-0000-7001-8000-000000000c51";
const NEXT_RUN = createFixture.effect.run_id;
const CONVERSATION = agentRunFixture.conversation_id;
const MESSAGE = "Now make it shorter.";
const scope = { owner_user_id: OWNER, project_id: PROJECT };
const reference: ChapterTextReference = { kind: "chapter_text", chapter_id: CHAPTER,
  base_authoritative_revision_id: REVISION, coordinate_profile: "storyos.editor.utf16-code-unit.v1",
  start: { manuscript_block_id: BLOCK, offset: 0 }, end: { manuscript_block_id: BLOCK, offset: 4 } };

type SteeringReply = "conflicted" | "refused" | "unknown";

/**
 * The panel reads the Run as claimed. Then the Run completes on the Server before the next read, and
 * the author sends a message with one selection reference. The Server replies to the steering input
 * with `steering`.
 */
async function sendAfterCompletion(steering: SteeringReply) {
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
    scope, message: "Help with this passage.", chapterId: CHAPTER,
    correlationId: "018f0000-0000-7001-8000-000000000c52", idempotencyKey: "018f0000-0000-7001-8000-000000000c53",
    snapshotId: "018f0000-0000-7001-8000-000000000c54", runId: RUN, conversationId: CONVERSATION,
  }));
  let completed = false;
  const commands: Array<{ route: string; body: Record<string, unknown> }> = [];
  const advisory = { kind: "advisory", decision_id: "018f0000-0000-7001-8000-000000000c55", selected: true,
    text: "Keep the second sentence.", continuation: { kind: "absent" } };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith("/anti-forgery-challenges")) return jsonResponse(challengeFixture);
    if (path.endsWith("/manuscript/tree")) return jsonResponse({ ...treeFixture, project_scope: scope,
      snapshot: { ...treeFixture.snapshot, project_scope: scope } });
    if (path.endsWith(`/agent-runs/${RUN}`)) return jsonResponse({ ...agentRunFixture, project_scope: scope,
      run_id: RUN, status: completed ? "completed" : "claimed", decision: completed ? advisory : { kind: "absent" } });
    if (path.endsWith(`/agent-runs/${NEXT_RUN}`)) return jsonResponse({ ...agentRunFixture, project_scope: scope,
      run_id: NEXT_RUN, status: "queued", decision: { kind: "absent" } });
    commands.push({ route: path.split("/").at(-1)!, body: JSON.parse(String(init?.body)) as Record<string, unknown> });
    if (path.endsWith("/steering-inputs")) {
      if (steering === "unknown") throw new TypeError("Failed to fetch");
      if (steering === "refused") return jsonResponse({ code: "project_command_refused" }, 403);
      return jsonResponse({ ...steerFixture, project_scope: scope, receipt: { ...steerFixture.receipt, project_scope: scope },
        effect: { kind: "conflicted", reason: "terminal_run" } });
    }
    if (path.endsWith("/agent-runs")) return jsonResponse({ ...createFixture, project_scope: scope,
      conversation_id: CONVERSATION, effect: { ...createFixture.effect, conversation_id: CONVERSATION } }, 202);
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
        // No scheduled read runs, so the panel keeps the claimed Run until the steering reply.
        scheduleRefresh: () => () => {},
      }));
    });
    await settle();
    addDraftReferences(scope, [{ reference, moved: false,
      label: { chapter: "第 1 章 渡口", part: { kind: "paragraphs", first: 1, last: 1 } } }]);
    const textarea = host.querySelector("textarea")!;
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(textarea, MESSAGE);
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
    });
    completed = true;
    await act(async () => { host.querySelector<HTMLButtonElement>("button.send-button")!.click(); });
    await settle();
    return {
      commands,
      status: host.querySelector("[role=status]")?.textContent ?? null,
      composer: textarea.value,
      runId: host.querySelector("[data-writing-assistant]")?.getAttribute("data-assistant-run-id"),
      retainedIntent: sessionStorage.getItem(`run_control:${OWNER}:${PROJECT}`) !== null,
    };
  } finally {
    await act(async () => root.unmount());
    host.remove();
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
}

it("sends a message as a new AgentRun in the same Conversation when steering finds the Run completed", async () => {
  const result = await sendAfterCompletion("conflicted");

  expect(result.commands.map((command) => command.route)).toEqual(["steering-inputs", "agent-runs"]);
  expect(result.commands[1]!.body).toMatchObject({ create_agent_run_input: {
    conversation: { kind: "existing", conversation_id: CONVERSATION },
    author_message: { text: MESSAGE },
    working_target: { kind: "selection_references", source_chapter_id: CHAPTER, references: [reference] },
  } });
  expect({ status: result.status, composer: result.composer, runId: result.runId, retainedIntent: result.retainedIntent })
    .toEqual({ status: null, composer: "", runId: NEXT_RUN, retainedIntent: false });
});

it("keeps the message and sends no AgentRun when the Server refuses the steering input", async () => {
  const result = await sendAfterCompletion("refused");

  expect({ routes: result.commands.map((command) => command.route), status: result.status,
    composer: result.composer, retainedIntent: result.retainedIntent }).toEqual({ routes: ["steering-inputs"],
    status: "这次操作未被接收。请检查当前任务和写作权限。", composer: MESSAGE, retainedIntent: false });
});

it("keeps the steering input for recovery and sends no AgentRun when its result is unknown", async () => {
  const result = await sendAfterCompletion("unknown");

  expect({ routes: result.commands.map((command) => command.route), status: result.status,
    composer: result.composer, retainedIntent: result.retainedIntent }).toEqual({ routes: ["steering-inputs"],
    status: "操作结果待确认。重新打开后将检查同一条操作，不会创建新任务。", composer: MESSAGE, retainedIntent: true });
});
