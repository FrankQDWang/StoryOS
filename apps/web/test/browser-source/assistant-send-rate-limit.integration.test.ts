import { act, createElement } from "react";
import { expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import challengeFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-project-command-challenge.json";
import createFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-agent-run.json";
import treeFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-manuscript-tree.json";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { openPanelHost } from "./assistant-panel-harness.ts";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const RUN = createFixture.effect.run_id;
const MESSAGE = "Help with this passage.";
const scope = { owner_user_id: OWNER, project_id: PROJECT };

/** Sends `MESSAGE` from the panel. The Server refuses the first Command Challenge with a rate limit (#1679). */
async function sendThroughRateLimit() {
  const challengeKeys: string[] = [];
  const routes: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith("/manuscript/tree")) return jsonResponse({ ...treeFixture, project_scope: scope,
      snapshot: { ...treeFixture.snapshot, project_scope: scope } });
    if (path.endsWith("/anti-forgery-challenges")) {
      challengeKeys.push(String((JSON.parse(String(init?.body)) as { idempotency_key: string }).idempotency_key));
      if (challengeKeys.length === 1) {
        return new Response(JSON.stringify({ schema_id: "storyos.problem.v1", code: "challenge_rate_limited",
          message: "The command challenge rate limit is exceeded." }),
        { status: 429, headers: { "content-type": "application/json", "retry-after": "38" } });
      }
      return jsonResponse(challengeFixture);
    }
    if (path.endsWith("/agent-runs")) {
      routes.push("agent-runs");
      return jsonResponse({ ...createFixture, project_scope: scope }, 202);
    }
    if (path.endsWith(`/agent-runs/${RUN}`)) {
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUN, status: "queued",
        decision: { kind: "absent" } });
    }
    if (path.endsWith("/activity")) return new Response("", { status: 200, headers: { "content-type": "text/event-stream" } });
    throw new Error(`Unexpected request ${path}`);
  };
  const timers: Array<{ callback: () => void; timeout: number; cleared: boolean }> = [];
  const panel = openPanelHost();
  const { host, root } = panel;
  const settle = async () => {
    for (let turn = 0; turn < 40; turn += 1) await act(async () => { await Promise.resolve(); });
  };
  await act(async () => {
    root.render(createElement(WritingAssistantPanel, {
      collapsed: false,
      context: { scope, chapterId: CHAPTER, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto },
      scheduleRefresh: () => () => {},
      challengeTimers: {
        setTimeoutImpl: (callback, timeout) => timers.push({ callback, timeout, cleared: false }) - 1,
        clearTimeoutImpl: (timer) => { if (typeof timer === "number" && timers[timer]) timers[timer].cleared = true; },
      },
    }));
  });
  await settle();
  const textarea = host.querySelector("textarea")!;
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(textarea, MESSAGE);
    textarea.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await act(async () => { host.querySelector<HTMLButtonElement>("button.send-button")!.click(); });
  await settle();
  return {
    host, root, timers, challengeKeys, routes, settle,
    view: () => ({
      dispatch: host.querySelector("[data-writing-assistant]")?.getAttribute("data-assistant-dispatch"),
      status: host.querySelector("[role=status]")?.textContent ?? null,
      composer: host.querySelector("textarea")?.value ?? null,
      runId: host.querySelector("[data-writing-assistant]")?.getAttribute("data-assistant-run-id"),
    }),
    async close() {
      await panel.close();
    },
  };
}

it("keeps a send in progress through a Challenge rate limit and sends the message one time", async () => {
  const send = await sendThroughRateLimit();
  try {
    const waiting = { view: send.view(), timeouts: send.timers.map((timer) => timer.timeout), routes: [...send.routes] };

    await act(async () => { send.timers[0]!.callback(); });
    await send.settle();

    expect({ waiting, keys: new Set(send.challengeKeys).size, challenges: send.challengeKeys.length,
      routes: send.routes, view: send.view() }).toEqual({
      waiting: { view: { dispatch: "sending", status: null, composer: MESSAGE, runId: "" }, timeouts: [38_000],
        routes: [] },
      keys: 1,
      challenges: 2,
      routes: ["agent-runs"],
      view: { dispatch: "queued", status: null, composer: "", runId: RUN },
    });
  } finally {
    await send.close();
  }
});

it("ends the wait of a send when the author leaves the Project and sends no request later", async () => {
  const send = await sendThroughRateLimit();
  try {
    await act(async () => send.root.unmount());
    // The timer callback of a cleared wait does nothing.
    send.timers[0]!.callback();
    for (let turn = 0; turn < 20; turn += 1) await Promise.resolve();

    expect({ cleared: send.timers.map((timer) => timer.cleared), challenges: send.challengeKeys.length,
      routes: send.routes }).toEqual({ cleared: [true], challenges: 1, routes: [] });
  } finally {
    await send.close();
  }
});
