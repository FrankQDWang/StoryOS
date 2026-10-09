import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";

import undoFixture from "../../../../generated/golden-wire/storyos-public-release-1/undo-latest-author-action.json";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import { ManuscriptEditor } from "../../src/manuscript-editor.tsx";
import type { PendingEditProjection } from "../../src/editor-types.ts";
import { applyImeComposition, applyTrustedInput } from "../support/browser-command-client.ts";
import { focusManuscriptEnd, manuscriptBody } from "../support/manuscript-surface.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import {
  SESSION,
  createBrowserScenario,
  jsonResponse,
  requestHeaders,
  requireRequestBody,
} from "./scenario.ts";

const UNDO_SCHEMA = "storyos.command.undo-latest-author-action.request.v1";

function gate() {
  let open!: () => void;
  const opened = new Promise<void>((resolve) => { open = resolve; });
  return { open, opened };
}

async function openUndoEditor() {
  const test = await openJournalAppendTestWorkspace();
  const scenario = createBrowserScenario();
  const firstChallenge = gate();
  const undoResponse = gate();
  firstChallenge.open();
  undoResponse.open();
  const sessionRefreshRead = gate();
  const firstChallengeRead = gate();
  const refusedChallenges = new Set([1]);
  const gates = { firstChallenge, undoResponse };
  let sessionReads = 0;
  const undoChallengeKeys: string[] = [];
  const undoKeys: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      const request = requireRequestBody(init);
      // Author Edits after an abandoned Undo stay in flight; these tests observe only Undo.
      if (request.command_schema !== UNDO_SCHEMA) return new Promise<Response>(() => {});
      undoChallengeKeys.push(String(request.idempotency_key));
      const attempt = undoChallengeKeys.length;
      if (attempt === 1) await gates.firstChallenge.opened;
      const response = refusedChallenges.has(attempt)
        ? new Response(JSON.stringify({ schema_id: "storyos.problem.v1",
          code: "challenge_rate_limited", message: "The command challenge rate limit is exceeded." }),
        { status: 429, headers: { "content-type": "application/json", "retry-after": "7" } })
        : jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-10-03T08:05:00.000Z",
          limit_profile_revision: "storyos.foundation.absolute.v1" });
      if (attempt !== 1) return response;
      const text = response.text.bind(response);
      response.text = async () => { const body = await text(); firstChallengeRead.open(); return body; };
      return response;
    }
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      sessionReads += 1;
      const response = jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1",
        author_undo_frontier_sequence: "1" });
      if (sessionReads !== 2) return response;
      // The second read is the refresh after a compensated Undo.
      const text = response.text.bind(response);
      response.text = async () => { const body = await text(); sessionRefreshRead.open(); return body; };
      return response;
    }
    if (path.endsWith("/author-actions/undo")) {
      undoKeys.push(requestHeaders(init).get("idempotency-key") ?? "");
      await gates.undoResponse.opened;
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
  const writing = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin, fetchImpl,
    onFailure: (error) => { failures.push(error); } });
  writing.subscribe(() => { projections.push(writing.snapshot().projection); });
  const props: Parameters<typeof ManuscriptEditor>[0] = { blocks: test.workspace.pending.blocks, editable: true,
    persistWorkspace: test.workspace, writing, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
    controllerRef: { current: null },
    onFailure: (error) => { failures.push(error); },
    undoChallengeTimers: {
      setTimeoutImpl: (callback, timeout) => timers.push({ callback, timeout, cleared: false }) - 1,
      clearTimeoutImpl: (timer) => { if (typeof timer === "number" && timers[timer]) timers[timer].cleared = true; },
    } };
  await act(async () => { root.render(createElement(ManuscriptEditor, props)); });
  const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
  surface.focus();
  let unmounted = false;
  return {
    gates, sessionRefreshRead, firstChallengeRead, refusedChallenges, surface, timers, projections, failures, undoChallengeKeys, undoKeys,
    pressUndo() {
      const mac = navigator.platform.includes("Mac");
      surface.dispatchEvent(new KeyboardEvent("keydown", { key: "z", code: "KeyZ", bubbles: true, cancelable: true,
        metaKey: mac, ctrlKey: !mac }));
    },
    // A new Editor Session workspace abandons an Undo of the earlier one.
    async replaceWorkspace() {
      const persistWorkspace = { ...test.workspace, pending: structuredClone(test.workspace.pending) };
      await act(async () => { root.render(createElement(ManuscriptEditor, { ...props, persistWorkspace })); });
    },
    // Composition input abandons an Undo and does not submit an Author Edit.
    async startComposition() {
      focusManuscriptEnd(surface, window);
      const offset = manuscriptBody(surface).length;
      await applyImeComposition({ text: "!", replacementStart: offset, replacementEnd: offset,
        selectionStart: 1, selectionEnd: 1 });
    },
    // Every continuation after the first Challenge response body is read is a microtask.
    async releaseFirstChallenge() {
      await act(async () => { gates.firstChallenge.open(); await firstChallengeRead.opened; await drain(); });
    },
    waitForRetryTimer: () => expect.poll(() => timers.some((timer) => timer.timeout === 7_000)).toBe(true),
    retryTimers: () => timers.filter((timer) => timer.timeout === 7_000),
    // A Journal transaction created now completes after a base install that started before it.
    journalSettled: () => new Promise<void>((resolve) => {
      test.workspace.database.transaction(["metadata"], "readonly").oncomplete = () => resolve();
    }),
    unmount: async () => { unmounted = true; await act(async () => { root.unmount(); }); },
    async close() {
      gates.firstChallenge.open();
      gates.undoResponse.open();
      if (!unmounted) await act(async () => { root.unmount(); });
      writing.close();
      host.remove();
      await test.close();
      Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
    },
  };
}

// One macrotask lets every continuation of an abandoned Undo run.
const drain = () => new Promise((resolve) => { setTimeout(resolve, 0); });

it("keeps the editor editable while an Author Undo waits for Challenge admission and retries the same Undo", async () => {
  const undo = await openUndoEditor();
  try {
    undo.pressUndo();
    await undo.waitForRetryTimer();
    expect({ failures: undo.failures, editable: undo.surface.getAttribute("contenteditable"),
      saveState: undo.projections.at(-1)?.save_state, undoRequests: undo.undoKeys.length })
      .toEqual({ failures: [], editable: "true", saveState: "saving", undoRequests: 0 });

    await act(async () => { undo.retryTimers()[0]!.callback(); });
    await expect.poll(() => undo.projections.at(-1)?.save_state).toBe("saved");

    expect({ failures: undo.failures, challengeKeys: undo.undoChallengeKeys.length,
      undoKeys: undo.undoKeys, editable: undo.surface.getAttribute("contenteditable") })
      .toEqual({ failures: [], challengeKeys: 2, undoKeys: [undo.undoChallengeKeys[0]], editable: "true" });
    expect(undo.undoChallengeKeys[1]).toBe(undo.undoChallengeKeys[0]);
  } finally {
    await undo.close();
  }
});

it("ignores another Ctrl/Cmd+Z while an Author Undo is in progress", async () => {
  const undo = await openUndoEditor();
  try {
    undo.gates.firstChallenge = gate();
    undo.pressUndo();
    await expect.poll(() => undo.undoChallengeKeys.length).toBe(1);
    undo.pressUndo();
    undo.gates.firstChallenge.open();
    await undo.waitForRetryTimer();
    undo.pressUndo();
    await drain();

    expect({ challengeKeys: undo.undoChallengeKeys.length, retryTimers: undo.retryTimers().length,
      failures: undo.failures }).toEqual({ challengeKeys: 1, retryTimers: 1, failures: [] });
  } finally {
    await undo.close();
  }
});

it.each(["input", "unmount"] as const)("abandons an Author Undo Challenge wait at the editor %s boundary", async (boundary) => {
  const undo = await openUndoEditor();
  try {
    undo.pressUndo();
    await undo.waitForRetryTimer();
    if (boundary === "input") await applyTrustedInput({ operation: "insert_text", text: "!" });
    else await undo.unmount();
    await drain();

    expect({ cleared: undo.retryTimers()[0]!.cleared, challengeKeys: undo.undoChallengeKeys.length,
      undoRequests: undo.undoKeys.length, failures: undo.failures })
      .toEqual({ cleared: true, challengeKeys: 1, undoRequests: 0, failures: [] });
  } finally {
    await undo.close();
  }
});

it("keeps new input when the author types while the retried Author Undo request is in flight", async () => {
  const undo = await openUndoEditor();
  try {
    undo.gates.undoResponse = gate();
    undo.pressUndo();
    await undo.waitForRetryTimer();
    await act(async () => { undo.retryTimers()[0]!.callback(); });
    await expect.poll(() => undo.undoKeys.length).toBe(1);
    focusManuscriptEnd(undo.surface, window);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    await act(async () => {
      undo.gates.undoResponse.open();
      await undo.sessionRefreshRead.opened;
      await drain();
      await undo.journalSettled();
      await drain();
    });

    expect({ text: undo.surface.textContent, failures: undo.failures })
      .toEqual({ text: "Base!", failures: [] });
  } finally {
    await undo.close();
  }
});

it("does not send an abandoned Author Undo when its Challenge request completes after new input", async () => {
  const undo = await openUndoEditor();
  try {
    undo.refusedChallenges.clear();
    undo.gates.firstChallenge = gate();
    undo.pressUndo();
    await expect.poll(() => undo.undoChallengeKeys.length).toBe(1);
    await undo.startComposition();
    await undo.releaseFirstChallenge();

    expect({ undoRequests: undo.undoKeys.length, failures: undo.failures })
      .toEqual({ undoRequests: 0, failures: [] });
    await applyImeComposition({ operation: "cancel" });
  } finally {
    await undo.close();
  }
});

it("starts no Retry-After wait for an abandoned Author Undo and accepts the next Ctrl/Cmd+Z at once", async () => {
  const undo = await openUndoEditor();
  try {
    undo.gates.firstChallenge = gate();
    undo.pressUndo();
    await expect.poll(() => undo.undoChallengeKeys.length).toBe(1);
    await undo.replaceWorkspace();
    await undo.releaseFirstChallenge();

    expect({ retryTimers: undo.retryTimers().length, failures: undo.failures,
      saving: undo.projections.filter((projection) => projection.save_state === "saving").length })
      .toEqual({ retryTimers: 0, failures: [], saving: 0 });
    undo.pressUndo();
    await expect.poll(() => undo.undoChallengeKeys.length).toBe(2);
  } finally {
    await undo.close();
  }
});
