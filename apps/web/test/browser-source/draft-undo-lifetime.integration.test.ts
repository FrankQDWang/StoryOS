import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import closedFixture from "../../../../generated/golden-wire/storyos-public-release-1/close-editor-flow-draft.json";
import draftFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-refused-edit-draft.json";
import { digestCloseEditorFlowDraft } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CloseEditorFlowDraftRequest, RefusedEditDraftInspect }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { ManuscriptEditor } from "../../src/manuscript-editor.tsx";
import { discardRefusedEdit } from "../../src/refused-edit-discard.ts";
import { createBrowserScenario, jsonResponse, requestResult } from "./scenario.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";

it.each(["unmount", "replace", "secret", "schema"] as const)("Draft Undo respects the editor %s boundary", async (boundary) => {
  const test = await openJournalAppendTestWorkspace();
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  const scenario = createBrowserScenario();
  const draft = structuredClone(draftFixture.draft) as RefusedEditDraftInspect;
  const closed = structuredClone(closedFixture);
  closed.project_scope = scenario.project.project_scope;
  closed.receipt.project_scope = scenario.project.project_scope;
  closed.effect.event.project_scope = scenario.project.project_scope;
  let entered!: () => void, release!: () => void;
  const sourceReached = new Promise<void>((resolve) => { entered = resolve; });
  const sourceGate = new Promise<void>((resolve) => { release = resolve; });
  let completed!: () => void;
  const oldTransaction = new Promise<void>((resolve) => { completed = resolve; });
  let released = false, observing = false;
  let failed!: () => void;
  let failure = new Promise<void>((resolve) => { failed = resolve; });
  const mutations: string[] = [], callbacks: string[] = [];
  const database = test.workspace.database;
  test.workspace.database = new Proxy(database, { get(target, key) {
    if (key === "transaction") return (...args: Parameters<IDBDatabase["transaction"]>) => {
      const transaction = target.transaction(...args);
      if (observing && released && args[1] === "readwrite") {
        transaction.addEventListener("complete", completed, { once: true });
        transaction.addEventListener("abort", completed, { once: true });
      }
      return transaction;
    };
    const value: unknown = Reflect.get(target, key, target);
    return typeof value === "function" ? value.bind(target) : value;
  } });
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (observing && init?.method === "POST") { mutations.push(path); return jsonResponse({
      schema_id: "storyos.problem.v1", code: "service_unavailable", message: "Controlled unavailable transport" }, 503); }
    if (path.endsWith("/anti-forgery-challenges")) return jsonResponse({ nonce: "a".repeat(64),
      expires_at: "2026-09-27T00:00:00.000Z", limit_profile_revision: "storyos.foundation.absolute.v1" });
    if (path.includes("/editor-sessions/")) return jsonResponse({ ...scenario.session,
      schema_id: "storyos.query.editor-session.response.v1", author_undo_frontier_sequence: "4" });
    if (path.endsWith("/closures")) {
      const request = JSON.parse(String(init?.body)) as CloseEditorFlowDraftRequest;
      const digest = await digestCloseEditorFlowDraft(request);
      const key = new Headers(init?.headers).get("idempotency-key")!;
      closed.correlation_id = request.close_editor_flow_draft_input.correlation_id;
      Object.assign(closed.effect.event, { draft_id: draft.draft_id, draft_revision_id: draft.draft_revision_id,
        payload_digest: draft.payload_digest, author_action_sequence: "4" });
      Object.assign(closed.receipt, { command_digest: digest, idempotency_key: key,
        draft_artifact_refs: [draft.draft_id], author_action_sequence: "4" });
      Object.assign(closed.effect.event.source, { command_digest: digest, idempotency_key: key });
      return jsonResponse(closed);
    }
    if (path.includes("/refused-edit-drafts/")) {
      entered(); await sourceGate; released = true;
      return jsonResponse({ ...draftFixture, project_scope: scenario.project.project_scope, draft: { ...draft, closure: "closed", closure_event: closed.effect.event } });
    }
    throw new Error(`Unexpected request ${path}`);
  };
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  const replacement = { ...test.workspace, pending: structuredClone(test.workspace.pending) };
  const props = { blocks: test.workspace.pending.blocks, editable: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
    controllerRef: { current: null }, onProjection: () => { callbacks.push("projection"); },
    onFailure: () => { callbacks.push("failure"); failed(); }, onCandidateSettled: () => { callbacks.push("settled"); } };
  try {
    await discardRefusedEdit({ workspace: test.workspace, draft, baseUrl: location.origin, fetchImpl });
    observing = true;
    await act(async () => { root.render(createElement(ManuscriptEditor, { ...props, persistWorkspace: test.workspace })); });
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!; surface.focus();
    surface.dispatchEvent(new KeyboardEvent("keydown", { key: "z", code: "KeyZ", bubbles: true, cancelable: true,
      metaKey: navigator.platform.includes("Mac"), ctrlKey: !navigator.platform.includes("Mac") }));
    await sourceReached;
    await act(async () => { if (boundary === "unmount") root.unmount();
      else if (boundary === "replace") root.render(createElement(ManuscriptEditor, { ...props, persistWorkspace: replacement })); });
    if (boundary === "schema") {
      const transaction = database.transaction("metadata", "readwrite");
      const done = new Promise<void>((resolve) => { transaction.oncomplete = () => resolve(); });
      transaction.objectStore("metadata").put({ key: "schema", version: 0 }); await done;
    }
    const pending = structuredClone(replacement.pending);
    callbacks.length = 0;
    await act(async () => { release(); await (boundary === "schema" ? failure : oldTransaction); });
    const rows = await requestResult(database.transaction("metadata").objectStore("metadata").getAll());
    if (boundary === "secret") {
      await failure;
      const record = (rows as { key: string }[]).find((row) => row.key.startsWith("draft-undo:"))!;
      const corrupted = { ...record, anti_forgery_nonce: "secret-must-not-enter-replay" };
      const transaction = database.transaction("metadata", "readwrite");
      const done = new Promise<void>((resolve) => { transaction.oncomplete = () => resolve(); });
      transaction.objectStore("metadata").put(corrupted); await done;
      const submitted = [...mutations];
      failure = new Promise<void>((resolve) => { failed = resolve; });
      surface.dispatchEvent(new KeyboardEvent("keydown", { key: "z", bubbles: true, cancelable: true, metaKey: navigator.platform.includes("Mac"), ctrlKey: !navigator.platform.includes("Mac") }));
      await failure;
      expect(mutations).toEqual(submitted);
      expect(await requestResult(database.transaction("metadata").objectStore("metadata").get(record.key))).toEqual(corrupted);
      return;
    }
    expect((rows as { key: string }[]).filter((row) => row.key.startsWith("draft-undo:"))).toEqual([]);
    expect(mutations).toEqual([]); expect(callbacks).toEqual(boundary === "schema" ? ["failure"] : []); expect(replacement.pending).toEqual(pending);
  } finally {
    release(); if (boundary !== "unmount") await act(async () => { root.unmount(); });
    host.remove(); await test.close(); Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});
