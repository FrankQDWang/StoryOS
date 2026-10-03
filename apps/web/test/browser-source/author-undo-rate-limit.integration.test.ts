import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";

import undoFixture from "../../../../generated/golden-wire/storyos-public-release-1/undo-latest-author-action.json";
import { ManuscriptEditor } from "../../src/manuscript-editor.tsx";
import type { PendingEditProjection } from "../../src/editor-types.ts";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import {
  SESSION,
  createBrowserScenario,
  jsonResponse,
  requestHeaders,
  requireRequestBody,
} from "./scenario.ts";

const UNDO_SCHEMA = "storyos.command.undo-latest-author-action.request.v1";

async function openRateLimitedUndo() {
  const test = await openJournalAppendTestWorkspace();
  const scenario = createBrowserScenario();
  const undoChallengeKeys: string[] = [];
  const undoKeys: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      const request = requireRequestBody(init);
      // Author Edits after an abandoned Undo stay in flight; this test observes only Undo.
      if (request.command_schema !== UNDO_SCHEMA) return new Promise<Response>(() => {});
      undoChallengeKeys.push(String(request.idempotency_key));
      if (undoChallengeKeys.length === 1) {
        return new Response(JSON.stringify({ schema_id: "storyos.problem.v1",
          code: "challenge_rate_limited", message: "The command challenge rate limit is exceeded." }),
        { status: 429, headers: { "content-type": "application/json", "retry-after": "7" } });
      }
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-10-03T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1",
        author_undo_frontier_sequence: "1" });
    }
    if (path.endsWith("/author-actions/undo")) {
      undoKeys.push(requestHeaders(init).get("idempotency-key") ?? "");
      return jsonResponse(undoFixture);
    }
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  const timers: Array<{ callback: () => void; timeout: number; cleared: boolean }> = [];
  const projections: PendingEditProjection[] = [];
  const failures: unknown[] = [];
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  await act(async () => {
    root.render(createElement(ManuscriptEditor, { blocks: test.workspace.pending.blocks, editable: true,
      persistWorkspace: test.workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
      controllerRef: { current: null },
      onProjection: (projection) => { projections.push(projection); },
      onFailure: (error) => { failures.push(error); },
      setTimeoutImpl: (callback, timeout) => timers.push({ callback, timeout, cleared: false }) - 1,
      clearTimeoutImpl: (timer) => { if (typeof timer === "number" && timers[timer]) timers[timer].cleared = true; } }));
  });
  const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
  surface.focus();
  const mac = navigator.platform.includes("Mac");
  surface.dispatchEvent(new KeyboardEvent("keydown", { key: "z", code: "KeyZ", bubbles: true, cancelable: true,
    metaKey: mac, ctrlKey: !mac }));
  await expect.poll(() => timers.some((timer) => timer.timeout === 7_000)).toBe(true);
  let unmounted = false;
  return {
    root, surface, timers, projections, failures, undoChallengeKeys, undoKeys,
    retryTimer: () => timers.find((timer) => timer.timeout === 7_000)!,
    unmount: async () => { unmounted = true; await act(async () => { root.unmount(); }); },
    async close() {
      if (!unmounted) await act(async () => { root.unmount(); });
      host.remove();
      await test.close();
      Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
    },
  };
}

it("keeps the editor editable while an Author Undo waits for Challenge admission and retries the same Undo", async () => {
  const undo = await openRateLimitedUndo();
  try {
    expect({ failures: undo.failures, editable: undo.surface.getAttribute("contenteditable"),
      saveState: undo.projections.at(-1)?.save_state, undoRequests: undo.undoKeys.length })
      .toEqual({ failures: [], editable: "true", saveState: "saving", undoRequests: 0 });

    await act(async () => { undo.retryTimer().callback(); });
    await expect.poll(() => undo.projections.at(-1)?.save_state).toBe("saved");

    expect({ failures: undo.failures, challengeKeys: undo.undoChallengeKeys.length,
      undoKeys: undo.undoKeys, editable: undo.surface.getAttribute("contenteditable") })
      .toEqual({ failures: [], challengeKeys: 2, undoKeys: [undo.undoChallengeKeys[0]], editable: "true" });
    expect(undo.undoChallengeKeys[1]).toBe(undo.undoChallengeKeys[0]);
  } finally {
    await undo.close();
  }
});

it.each(["input", "unmount"] as const)("abandons an Author Undo Challenge wait at the editor %s boundary", async (boundary) => {
  const undo = await openRateLimitedUndo();
  try {
    if (boundary === "input") await applyTrustedInput({ operation: "insert_text", text: "!" });
    else await undo.unmount();
    // One macrotask lets every continuation of the abandoned Undo run.
    await new Promise((resolve) => { setTimeout(resolve, 0); });

    expect({ cleared: undo.retryTimer().cleared, challengeKeys: undo.undoChallengeKeys.length,
      undoRequests: undo.undoKeys.length, failures: undo.failures })
      .toEqual({ cleared: true, challengeKeys: 1, undoRequests: 0, failures: [] });
  } finally {
    await undo.close();
  }
});
