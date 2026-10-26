import { expect, it } from "vitest";
import { act, createElement, useLayoutEffect, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";

import { appendAuthorEdit } from "../../src/editor-session.ts";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import { rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import {
  FIRST_APPEND_EDIT,
  SECOND_APPEND_EDIT,
  openJournalAppendTestWorkspace,
  withDigestBudget,
} from "./local-edit-journal-append-fixture.ts";

it("returns the valid append projection without a second Journal reconstruction", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    await appendAuthorEdit(test.workspace, FIRST_APPEND_EDIT);
    test.workspace.cryptoImpl = withDigestBudget(crypto, 2);

    const projection = await appendAuthorEdit(test.workspace, SECOND_APPEND_EDIT);
    test.workspace.cryptoImpl = crypto;

    expect(projection).toEqual({
      body: "Base!?",
      blocks: [{
        ...test.workspace.session.base_snapshot.materialized_revision.blocks[0]!,
        text: "Base!?",
      }],
      save_state: "saving",
      unsettled_intent_count: 2,
      authoritative_revision_id:
        test.workspace.session.base_snapshot.authoritative_head_revision_id,
    });
    expect(await rebuildPendingProjection(test.workspace)).toEqual(projection);
  } finally {
    test.workspace.cryptoImpl = crypto;
    await test.close();
  }
});

it("freezes one Tiptap IME intent over a crossing selection as one Chapter edit after confirmation and ignores cancellation", async () => {
  const test = await openJournalAppendTestWorkspace();
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const id = "018f0000-0000-7001-8000-000000000090";
  const { ManuscriptEditor } = await import("../../src/manuscript-editor.tsx");
  const { applyImeComposition, applyTrustedInput } = await import("../support/browser-command-client.ts");
  const { readJournalSnapshot } = await import("../../src/local-edit-journal.ts");
  let failure: unknown;
  let requests = 0;
  const fetchImpl = () => { requests += 1; return new Promise<Response>(() => {}); };
  const writing = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin, fetchImpl,
    onFailure: (error) => { failure = error; } });
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => { root.render(createElement(ManuscriptEditor, { blocks: test.workspace.openedProjection.blocks,
      proposals: [{ proposalId: id, operationId: id, revisionId: id, blockId: test.workspace.openedProjection.blocks[0]!.manuscript_block_id,
        sourceRunId: id, sourceDecisionId: id, text: "Old", eligible: true, expectedHeads: [id] }],
      editable: true, writing })); });
    await expect.poll(() => host.querySelector(".block-proposal-text")).not.toBeNull();
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    const select = () => {
      surface.focus();
      const first = surface.querySelector("p")!.firstChild!;
      const last = surface.querySelector(".block-proposal-text")!.firstChild!;
      window.getSelection()!.setBaseAndExtent(first, 1, last, 2);
      document.dispatchEvent(new Event("selectionchange"));
    };
    /** The text from the start of the Chapter paragraph to a collapsed caret in it. */
    const caret = () => {
      const selection = window.getSelection()!;
      const chapter = surface.querySelector("p")!;
      if (!selection.isCollapsed || !chapter.contains(selection.anchorNode)) return undefined;
      const range = document.createRange();
      range.setStart(chapter, 0);
      range.setEnd(selection.anchorNode!, selection.anchorOffset);
      return range.toString();
    };
    select();
    const selected = window.getSelection()!.toString();
    await applyImeComposition({ text: "provisional", replacementStart: 1, replacementEnd: 8, selectionStart: 11, selectionEnd: 11 });
    expect((await readJournalSnapshot(test.workspace)).records).toEqual([]);
    expect(requests).toBe(0);
    await applyImeComposition({ operation: "cancel" });
    // A cancelled composition restores the start selection.
    await expect.poll(() => window.getSelection()!.toString()).toBe(selected);
    await expect.poll(() => writing.hasIncompleteInput()).toBe(false);
    expect((await readJournalSnapshot(test.workspace)).records).toEqual([]);
    expect(requests).toBe(0);
    select();
    await applyImeComposition({ text: "provisional", replacementStart: 1, replacementEnd: 8, selectionStart: 11, selectionEnd: 11 });
    expect((await readJournalSnapshot(test.workspace)).records).toEqual([]);
    await applyTrustedInput({ operation: "insert_text", text: "Complete 中文" });
    await expect.poll(async () => (await readJournalSnapshot(test.workspace)).records.length).toBe(1);
    const [record] = (await readJournalSnapshot(test.workspace)).records;
    expect(record!.input_origin).toBe("composition_confirmation");
    // The composition replaces only the Chapter part, the part of the first owner (ADR 0072).
    expect({ unit: record!.author_edit_unit?.normalized_primitives, target: record!.proposal_target }).toEqual({
      unit: [{ kind: "replace_block_selection", text: "Complete 中文", from: 1, to: 4,
        manuscript_block_id: test.workspace.openedProjection.blocks[0]!.manuscript_block_id }], target: undefined });
    expect(surface.querySelector("p")!.textContent).toBe("BComplete 中文");
    expect(surface.querySelector(".block-proposal-text")!.textContent).toBe("Old");
    // The caret stays at the end of the confirmed text, in the first owner.
    await expect.poll(caret).toBe("BComplete 中文");
    expect(failure).toBeUndefined();
  } finally { await act(async () => { root.unmount(); }); writing.close(); host.remove(); await test.close();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct }); }
});


it("keeps newer captured input visible when an earlier Author Edit settles", async () => {
  const test = await openJournalAppendTestWorkspace();
  const { ManuscriptEditor } = await import("../../src/manuscript-editor.tsx");
  const { applyTrustedInput } = await import("../support/browser-command-client.ts");
  const { createBrowserScenario, createAppliedAuthorEditResponse } = await import("./scenario.ts");
  const { digestApplyAuthorEdit } = await import("../../../../generated/typescript/storyos-public-release-1/client.mjs");
  const scenario = createBrowserScenario();
  let canonical = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
  let reached!: () => void, release!: () => void;
  const requested = new Promise<void>((resolve) => { reached = resolve; });
  const responseGate = new Promise<void>((resolve) => { release = resolve; });
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = String(input);
    if (path.endsWith("/anti-forgery-challenges")) return Response.json({ nonce: "a".repeat(64),
      expires_at: new Date(Date.now() + 60_000).toISOString(), limit_profile_revision: "storyos.foundation.absolute.v1" });
    if (path.includes("/editor-sessions/")) return Response.json(canonical);
    if (!path.endsWith("/manuscript/author-edits")) throw new Error(`Unexpected request ${path}`);
    const request = JSON.parse(String(init?.body));
    const response = createAppliedAuthorEditResponse({ request, body: "Base!",
      commandDigest: await digestApplyAuthorEdit(request), idempotencyKey: new Headers(init?.headers).get("idempotency-key")! });
    const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode("Base!"));
    canonical = { ...canonical, base_snapshot: { ...canonical.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000080", project_activity_position: "1",
      authoritative_head_revision_id: response.effect.kind === "authoritative_applied"
        ? response.effect.authoritative_revision.revision_id : "",
      materialized_revision: { ...canonical.base_snapshot.materialized_revision, revision_id: "018f0000-0000-7001-8000-000000000034",
        body: "Base!", blocks: canonical.base_snapshot.materialized_revision.blocks.map((block) => ({ ...block, text: "Base!" })) },
      materialized_payload_digest: { ...canonical.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("") } } };
    reached(); await responseGate;
    return Response.json(response);
  };
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  let failure: unknown, installedBody: string | null | undefined;
  const writing = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin, fetchImpl,
    onFailure: (error) => { failure = error; } });
  function View() {
    const { projection } = useSyncExternalStore(writing.subscribe, writing.snapshot);
    if (projection.authoritative_revision_id !== scenario.session.base_snapshot.authoritative_head_revision_id) {
      queueMicrotask(() => { installedBody ??= host.querySelector("[data-manuscript-editor] p")?.textContent; });
    }
    return createElement(ManuscriptEditor, { blocks: projection.blocks, editable: true, writing });
  }
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => { root.render(createElement(View)); });
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    surface.focus();
    const text = surface.querySelector("p")!.firstChild!;
    window.getSelection()!.setBaseAndExtent(text, 4, text, 4);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    const flushing = writing.flush();
    await requested;
    await applyTrustedInput({ operation: "insert_text", text: "?" });
    expect(surface.querySelector("p")!.textContent).toBe("Base!?");
    await act(async () => { release(); await flushing; await writing.whenIdle(); });
    expect(installedBody).toBe("Base!?");
    expect(surface.querySelector("p")!.textContent).toBe("Base!?");
    expect((await rebuildPendingProjection(test.workspace)).body).toBe("Base!?");
    expect(failure).toBeUndefined();
  } finally { release(); await act(async () => { root.unmount(); }); writing.close(); host.remove(); await test.close();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct }); }
});


it("keeps input that the author types before the effects of a settled projection render occur", async () => {
  const test = await openJournalAppendTestWorkspace();
  const { ManuscriptEditor } = await import("../../src/manuscript-editor.tsx");
  const { applyTrustedInput } = await import("../support/browser-command-client.ts");
  const { createBrowserScenario, createAppliedAuthorEditResponse } = await import("./scenario.ts");
  const { digestApplyAuthorEdit } = await import("../../../../generated/typescript/storyos-public-release-1/client.mjs");
  const scenario = createBrowserScenario();
  let canonical = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
  let submissions = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = String(input);
    if (path.endsWith("/anti-forgery-challenges")) return Response.json({ nonce: "a".repeat(64),
      expires_at: new Date(Date.now() + 60_000).toISOString(), limit_profile_revision: "storyos.foundation.absolute.v1" });
    if (path.includes("/editor-sessions/")) return Response.json(canonical);
    if (!path.endsWith("/manuscript/author-edits")) throw new Error(`No request handler: ${path}`);
    // Subsequent input stays in the Journal. This test monitors only the first settlement.
    if ((submissions += 1) > 1) return new Promise<Response>(() => {});
    const request = JSON.parse(String(init?.body));
    const response = createAppliedAuthorEditResponse({ request, body: "Base!",
      commandDigest: await digestApplyAuthorEdit(request), idempotencyKey: new Headers(init?.headers).get("idempotency-key")! });
    const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode("Base!"));
    canonical = { ...canonical, base_snapshot: { ...canonical.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000080", project_activity_position: "1",
      authoritative_head_revision_id: response.effect.kind === "authoritative_applied"
        ? response.effect.authoritative_revision.revision_id : "",
      materialized_revision: { ...canonical.base_snapshot.materialized_revision, revision_id: "018f0000-0000-7001-8000-000000000034",
        body: "Base!", blocks: canonical.base_snapshot.materialized_revision.blocks.map((block) => ({ ...block, text: "Base!" })) },
      materialized_payload_digest: { ...canonical.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("") } } };
    return Response.json(response);
  };
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  let failure: unknown, typed = false;
  const initialRevision = test.workspace.openedProjection.authoritative_revision_id;
  const writing = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin, fetchImpl,
    onFailure: (error) => { failure = error; } });
  function View() {
    const { projection } = useSyncExternalStore(writing.subscribe, writing.snapshot);
    // The author types after the settled render commits and before its passive effects occur.
    useLayoutEffect(() => {
      if (typed || projection.save_state !== "saved" || projection.authoritative_revision_id === initialRevision) return;
      typed = true;
      queueMicrotask(() => { document.execCommand("insertText", false, "?"); });
    }, [projection]);
    return createElement(ManuscriptEditor, { blocks: projection.blocks, editable: true, writing });
  }
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: false });
  try {
    root.render(createElement(View));
    await expect.poll(() => host.querySelector("[data-manuscript-editor] p")?.textContent).toBe("Base");
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    surface.focus();
    const text = surface.querySelector("p")!.firstChild!;
    window.getSelection()!.setBaseAndExtent(text, 4, text, 4);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    const replaced: (string | null)[] = [];
    const observer = new MutationObserver((records) => {
      for (const record of records) {
        if (record.type === "characterData") replaced.push(record.oldValue);
        else replaced.push(...[...record.removedNodes].map((node) => node.textContent));
      }
    });
    observer.observe(surface, { subtree: true, childList: true, characterData: true, characterDataOldValue: true });
    await writing.flush();
    await expect.poll(() => typed).toBe(true);
    await expect.poll(async () => (await rebuildPendingProjection(test.workspace)).body).toBe("Base!?");
    observer.disconnect();
    expect({ newestReplaced: replaced.includes("Base!?"), text: surface.querySelector("p")!.textContent, failure })
      .toEqual({ newestReplaced: false, text: "Base!?", failure: undefined });
  } finally { root.unmount(); writing.close(); host.remove(); await test.close();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct }); }
});


it("keeps author input when a Journal projection read before that input completes after it", async () => {
  const test = await openJournalAppendTestWorkspace();
  const { ManuscriptEditor } = await import("../../src/manuscript-editor.tsx");
  const { applyTrustedInput } = await import("../support/browser-command-client.ts");
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  let failure: unknown;
  // The submission stays unsettled, so the Journal keeps the input as local work.
  const writing = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin,
    fetchImpl: () => new Promise<Response>(() => {}), onFailure: (error) => { failure = error; } });
  function View() {
    const { projection } = useSyncExternalStore(writing.subscribe, writing.snapshot);
    return createElement(ManuscriptEditor, { blocks: projection.blocks, editable: true, writing });
  }
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => { root.render(createElement(View)); });
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    // A background reader, for example a refresh after a settlement, reads the Journal before the input.
    const earlier = writing.refresh();
    surface.focus();
    const text = surface.querySelector("p")!.firstChild!;
    window.getSelection()!.setBaseAndExtent(text, 4, text, 4);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    await act(async () => { await earlier; await writing.whenIdle(); });
    const journal = await rebuildPendingProjection(test.workspace);
    expect({ text: surface.querySelector("p")!.textContent, installed: writing.snapshot().projection, failure })
      .toEqual({ text: "Base!", installed: journal, failure: undefined });
    expect(journal.save_state).toBe("saving");
  } finally { await act(async () => { root.unmount(); }); writing.close(); host.remove(); await test.close();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct }); }
});


it("shows a changed Journal projection before its install completes", async () => {
  const test = await openJournalAppendTestWorkspace();
  const { ManuscriptEditor } = await import("../../src/manuscript-editor.tsx");
  const { submitOnePendingAuthorEdit } = await import("../../src/author-edit-submission.ts");
  const { createBrowserScenario, createAppliedAuthorEditResponse } = await import("./scenario.ts");
  const { digestApplyAuthorEdit } = await import("../../../../generated/typescript/storyos-public-release-1/client.mjs");
  const scenario = createBrowserScenario();
  let canonical = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = String(input);
    if (path.endsWith("/anti-forgery-challenges")) return Response.json({ nonce: "a".repeat(64),
      expires_at: new Date(Date.now() + 60_000).toISOString(), limit_profile_revision: "storyos.foundation.absolute.v1" });
    if (path.includes("/editor-sessions/")) return Response.json(canonical);
    if (!path.endsWith("/manuscript/author-edits")) throw new Error(`Unexpected request ${path}`);
    const request = JSON.parse(String(init?.body));
    const response = createAppliedAuthorEditResponse({ request, body: "Base!",
      commandDigest: await digestApplyAuthorEdit(request), idempotencyKey: new Headers(init?.headers).get("idempotency-key")! });
    const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode("Base!"));
    canonical = { ...canonical, base_snapshot: { ...canonical.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000080", project_activity_position: "1",
      authoritative_head_revision_id: response.effect.kind === "authoritative_applied"
        ? response.effect.authoritative_revision.revision_id : "",
      materialized_revision: { ...canonical.base_snapshot.materialized_revision, revision_id: "018f0000-0000-7001-8000-000000000034",
        body: "Base!", blocks: canonical.base_snapshot.materialized_revision.blocks.map((block) => ({ ...block, text: "Base!" })) },
      materialized_payload_digest: { ...canonical.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("") } } };
    return Response.json(response);
  };
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  let failure: unknown;
  const writing = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin, fetchImpl,
    onFailure: (error) => { failure = error; } });
  function View() {
    const { projection } = useSyncExternalStore(writing.subscribe, writing.snapshot);
    return createElement(ManuscriptEditor, { blocks: projection.blocks, editable: true, writing });
  }
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => { root.render(createElement(View)); });
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    // An Author Edit settles outside the editor.
    await appendAuthorEdit(test.workspace, FIRST_APPEND_EDIT);
    const settled = await submitOnePendingAuthorEdit({ workspace: test.workspace, baseUrl: location.origin, fetchImpl });
    let shown: string | null = null;
    await act(async () => {
      await writing.refresh();
      shown = surface.querySelector("p")!.textContent;
    });
    expect({ shown, installed: writing.snapshot().projection, failure })
      .toEqual({ shown: "Base!", installed: settled, failure: undefined });
  } finally { await act(async () => { root.unmount(); }); writing.close(); host.remove(); await test.close();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct }); }
});
