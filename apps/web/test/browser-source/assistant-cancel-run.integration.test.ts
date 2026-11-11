import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import { digestCancelAgentRun } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CancelAgentRunRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import cancelFixture from "../../../../generated/golden-wire/storyos-public-release-1/cancel-agent-run.json";
import challengeFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-project-command-challenge.json";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const RUN = "018f0000-0000-7001-8000-000000000c71";
const CONVERSATION = agentRunFixture.conversation_id;
const CANCEL_ROUTE = `/api/v1/projects/${PROJECT}/agent-runs/${RUN}/cancel`;
const PENDING = "操作结果待确认。重新打开后将检查同一条操作，不会创建新任务。";
const scope = { owner_user_id: OWNER, project_id: PROJECT };

/** The status of the Run that the panel reads first. `unread` is a read that never answers. */
type RunState = "claimed" | "paused" | "completed" | "unread";

/**
 * The panel reads the Run in `state`, and the author presses "取消任务" when the panel shows it. A
 * `lost` acknowledgement is a cancel request that the Server records but whose response does not
 * arrive. Then the page reloads.
 */
async function cancelFrom(state: RunState, acknowledgement: "received" | "lost" = "received") {
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
    scope, message: "Help with this passage.", chapterId: CHAPTER,
    correlationId: "018f0000-0000-7001-8000-000000000c72", idempotencyKey: "018f0000-0000-7001-8000-000000000c73",
    snapshotId: "018f0000-0000-7001-8000-000000000c74", runId: RUN, conversationId: CONVERSATION,
  }));
  let cancelled = false;
  let lost = acknowledgement === "lost";
  const challenges: Array<Record<string, unknown>> = [];
  const commands: Array<{ path: string; key: string | null; body: CancelAgentRunRequest }> = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith("/anti-forgery-challenges")) {
      challenges.push(JSON.parse(String(init?.body)) as Record<string, unknown>);
      return jsonResponse(challengeFixture);
    }
    if (path.endsWith(`/agent-runs/${RUN}`)) {
      if (state === "unread") return new Promise<Response>(() => {});
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUN,
        status: cancelled ? "cancelled" : state, decision: { kind: "absent" },
        original_result_retrieval: { kind: "absent" }, unknown_create_successor: { kind: "absent" } });
    }
    if (path === CANCEL_ROUTE) {
      commands.push({ path, key: new Headers(init?.headers).get("idempotency-key"),
        body: JSON.parse(String(init?.body)) as CancelAgentRunRequest });
      cancelled = true;
      if (lost) {
        lost = false;
        throw new TypeError("Failed to fetch");
      }
      return jsonResponse({ ...cancelFixture, project_scope: scope, receipt: { ...cancelFixture.receipt, project_scope: scope },
        effect: { ...cancelFixture.effect, run_id: RUN } });
    }
    throw new Error(`Unexpected request ${path}`);
  };
  const host = document.createElement("div");
  document.body.append(host);
  let root = createRoot(host);
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  const settle = async () => {
    for (let turn = 0; turn < 40; turn += 1) await act(async () => { await Promise.resolve(); });
  };
  const mount = async () => {
    await act(async () => {
      root.render(createElement(WritingAssistantPanel, {
        collapsed: false,
        context: { scope, chapterId: CHAPTER, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto },
        // No scheduled read runs, so only the cancel control makes the panel read the Run again.
        scheduleRefresh: () => () => {},
      }));
    });
    await settle();
  };
  const control = () => host.querySelector<HTMLButtonElement>("button[data-cancel-agent-run]");
  const view = () => ({
    control: control() === null ? null : { text: control()!.textContent, disabled: control()!.disabled },
    sendLabel: host.querySelector(".send-button")?.getAttribute("aria-label") ?? null,
    status: host.querySelector("[role=status]")?.textContent ?? null,
    dispatch: host.querySelector("[data-writing-assistant]")?.getAttribute("data-assistant-dispatch") ?? null,
    retainedIntent: sessionStorage.getItem(`run_control:${OWNER}:${PROJECT}`) !== null,
  });
  try {
    await mount();
    const before = view();
    let unacknowledged: ReturnType<typeof view> | undefined;
    if (control() !== null) {
      await act(async () => { control()!.click(); });
      await settle();
      if (acknowledgement === "lost") {
        unacknowledged = view();
        // A reload keeps `sessionStorage` and starts a new page.
        await act(async () => root.unmount());
        root = createRoot(host);
        await mount();
      }
    }
    return { before, unacknowledged, after: view(), commands, challenges };
  } finally {
    await act(async () => root.unmount());
    host.remove();
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
}

/** The challenge of one cancel request: its route, its schema, its digest, and its idempotency key. */
async function challengeOf(command: { key: string | null; body: CancelAgentRunRequest }) {
  return { method: "POST", route_template: "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
    command_schema: "storyos.command.cancel-agent-run.request.v1",
    canonical_command_digest: await digestCancelAgentRun(command.body), idempotency_key: command.key };
}

it.each([
  ["a running Run", "claimed" as const, "暂停"],
  ["a paused Run", "paused" as const, "发送"],
])("sends cancelAgentRun one time for %s and shows the cancelled Run", async (_, state, sendLabel) => {
  const result = await cancelFrom(state);

  expect(result.before).toEqual({ control: { text: "取消任务", disabled: false }, sendLabel, status: null,
    dispatch: state, retainedIntent: false });
  expect(result.commands).toHaveLength(1);
  const [command] = result.commands;
  expect(command!.path).toBe(CANCEL_ROUTE);
  expect(command!.body.command_schema).toBe("storyos.command.cancel-agent-run.request.v1");
  expect(result.challenges).toEqual([await challengeOf(command!)]);
  expect(result.after).toEqual({ control: null, sendLabel: "发送", status: "任务已取消。", dispatch: "cancelled",
    retainedIntent: false });
});

it("sends the same cancellation one time again after a lost acknowledgement and a reload", async () => {
  const result = await cancelFrom("claimed", "lost");

  expect(result.unacknowledged).toEqual({ control: { text: "取消任务", disabled: true }, sendLabel: "暂停",
    status: PENDING, dispatch: "claimed", retainedIntent: true });
  // The resend has the idempotency key and the body of the first request, so the Server gives its one Receipt.
  expect(result.commands).toHaveLength(2);
  expect(result.commands[1]).toEqual(result.commands[0]);
  const challenge = await challengeOf(result.commands[0]!);
  expect(result.challenges).toEqual([challenge, challenge]);
  expect(result.after).toEqual({ control: null, sendLabel: "发送", status: "任务已取消。", dispatch: "cancelled",
    retainedIntent: false });
});

it.each([
  ["a settled Run", "completed" as const, "completed"],
  ["a Run that the panel did not read", "unread" as const, "idle"],
])("shows no cancel control for %s and sends no command", async (_, state, dispatch) => {
  const result = await cancelFrom(state);

  expect({ control: result.before.control, dispatch: result.before.dispatch, commands: result.commands.length })
    .toEqual({ control: null, dispatch, commands: 0 });
});
