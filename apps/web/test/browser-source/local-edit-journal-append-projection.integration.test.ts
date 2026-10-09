import { expect, it, vi } from "vitest";
import { act, createElement, useLayoutEffect, useState, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { RefusedEditDraftDisplay } from "../../src/refused-edit-draft-display.tsx";

import { persistReplaceSelection } from "../../src/editor-session.ts";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import { rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import {
  FIRST_APPEND_EDIT,
  SECOND_APPEND_EDIT,
  openJournalAppendTestWorkspace,
  createPausedDigestCrypto,
  withDigestBudget,
} from "./local-edit-journal-append-fixture.ts";

it("returns the valid append projection without a second Journal reconstruction", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    await persistReplaceSelection(test.workspace, FIRST_APPEND_EDIT);
    test.workspace.cryptoImpl = withDigestBudget(crypto, 2);

    const projection = await persistReplaceSelection(test.workspace, SECOND_APPEND_EDIT);
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

it("keeps a complete mixed intent in the immutable Journal without projecting it as manuscript truth", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    const unit = { normalized_primitives: [{ kind: "replace_structured_selection", replacement: [
      { block_kind: "heading", text: "New heading" }, { block_kind: "paragraph", text: "New paragraph" },
    ] }], selection_snapshot: { coordinate_profile: "storyos.editor.ordered-source.v1", from: 0, to: 3,
      ordered_selection: { sources: [{ owner: { kind: "manuscript", manuscript_block_id: test.workspace.pending.blocks[0]!.manuscript_block_id },
        coordinate_profile: "prosemirror-token-utf16.v1", from: 0, to: 4, block_kind: "paragraph", source_text: "Base" },
        { owner: { kind: "proposal", proposal_id: "018f0000-0000-7001-8000-000000000090",
          operation_id: "018f0000-0000-7001-8000-000000000091", revision_id: "018f0000-0000-7001-8000-000000000092",
          manuscript_block_id: test.workspace.pending.blocks[0]!.manuscript_block_id },
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 3, block_kind: "paragraph", source_text: "Old" }],
        anchor: { source_index: 0, source_offset: 0 }, head: { source_index: 1, source_offset: 3 } } } };
    let failure: unknown;
    const idle = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: "http://localhost",
      onFailure: (error) => { failure = error; }, setTimeoutImpl: () => 1, clearTimeoutImpl: () => {} });
    idle.setHoldSubmission(true);
    Reflect.apply(idle.capture, idle, [{ kind: "structured_selection", authorEditUnit: unit,
      expectedProposalHeads: ["018f0000-0000-7001-8000-000000000092"] }, "typing"]);
    await idle.whenIdle();
    idle.close();
    expect(failure).toBeUndefined();
    const { readJournalSnapshot } = await import("../../src/local-edit-journal.ts");
    expect((await readJournalSnapshot(test.workspace)).records[0]?.author_edit_unit).toEqual(unit);
    expect(await rebuildPendingProjection(test.workspace)).toEqual({ ...test.workspace.pending,
      body: "Base", save_state: "saving", unsettled_intent_count: 1 });
    const { submitOnePendingAuthorEdit } = await import("../../src/author-edit-submission.ts");
    const { digestApplyAuthorEdit } = await import("../../../../generated/typescript/storyos-public-release-1/client.mjs");
    const ids = ["018f0000-0000-7001-8000-000000000093", "018f0000-0000-7001-8000-000000000094",
      "018f0000-0000-7001-8000-000000000095", "018f0000-0000-7001-8000-000000000096",
      "018f0000-0000-7001-8000-000000000097", "018f0000-0000-7001-8000-000000000098"];
    const responseFetch: typeof fetch = async (input, init) => {
      const path = String(input);
      if (path.endsWith("/anti-forgery-challenges")) return Response.json({ nonce: "a".repeat(64),
        expires_at: new Date(Date.now() + 60000).toISOString(), limit_profile_revision: "storyos.foundation.absolute.v1" });
      if (!path.endsWith("/manuscript/author-edits")) throw new Error(`Unexpected request ${path}`);
      const request = JSON.parse(String(init?.body));
      const group = (await readJournalSnapshot(test.workspace)).groups[0]!;
      return Response.json({ schema_id: "storyos.command.apply-author-edit.response.v2",
        correlation_id: request.correlation_id, project_scope: group.project_scope,
        command_id: ids[0], author_command_admission_id: ids[1],
        completed_intent_record_id: request.completed_intent_record_id, local_intent_sequence: request.local_intent_sequence,
        effect: { kind: "refused_to_draft", refusal_origin: "fresh_editor_intent",
          draft_id: ids[3], draft_revision_id: ids[4], creation_event_id: ids[5] },
        receipt: { receipt_id: ids[2], project_scope: group.project_scope, command_kind: "applyAuthorEdit",
          command_digest: await digestApplyAuthorEdit(request), idempotency_key: group.idempotency_key,
          producer_cause: "author_command_admission", author_command_admission_id: ids[1],
          expected_heads: [request.expected_authoritative_revision_id], prior_heads: [request.expected_authoritative_revision_id],
          resulting_heads: [request.expected_authoritative_revision_id], authoritative_revision_ids: [], proposal_revision_ids: [],
          authoritative_commit_ids: [], author_action_sequence: null, draft_artifact_refs: [ids[3]],
          artifact_lifecycle_event_refs: [ids[5]], condition_refs: [], result: "refused_to_draft", created_at: new Date().toISOString() } });
    };
    expect(await submitOnePendingAuthorEdit({ workspace: test.workspace, baseUrl: location.origin,
      fetchImpl: responseFetch })).toMatchObject({ body: "Base", save_state: "saved", unsettled_intent_count: 0 });
    expect((await readJournalSnapshot(test.workspace)).records[0]?.author_edit_unit).toEqual(unit);
    const group = (await readJournalSnapshot(test.workspace)).groups[0]!;
    const request = group.frozen_request_body;
    const payload = { schema_revision: "storyos.refused-edit-payload.v1", chapter_id: request.chapter_id,
      expected_authoritative_revision_id: request.expected_authoritative_revision_id,
      expected_proposal_head_revision_ids: request.expected_proposal_head_revision_ids,
      target_refs: request.target_refs, author_edit_units: [unit], undo_group_id: request.undo_group_id,
      completed_intent_record_id: request.completed_intent_record_id, local_intent_sequence: request.local_intent_sequence };
    const keys = new Set<string>();
    JSON.stringify(payload, (key, value: unknown) => { keys.add(key); return value; });
    const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(JSON.stringify(payload, [...keys].sort())));
    const query = { schema_id: "storyos.query.refused-edit-draft.response.v1", project_scope: group.project_scope,
      draft: { draft_id: ids[3], draft_revision_id: ids[4], kind: "refused_edit", closure: "open", retention_state: "retained",
        payload, payload_digest: [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join(""),
        payload_digest_profile: "storyos.refused-edit-payload.jcs.v1", creation: {
          event_kind: "refused_edit_draft_created", schema_id: "storyos.event.refused-edit-draft-created.v1",
          project_scope: group.project_scope, draft_id: ids[3], draft_revision_id: ids[4], creation_event_id: ids[5],
          creator: { kind: "core_transition", receipt_id: ids[2] }, source: { command_id: ids[0],
            author_command_admission_id: ids[1], receipt_id: ids[2], idempotency_key: group.idempotency_key,
            command_digest: group.frozen_request_digest } } } };
    for (const action of ["discard", "copy"]) {
    let release!: (response: Response) => void;
    let started!: () => void;
    const pending = new Promise<Response>((resolve) => { release = resolve; });
    const copyStarted = new Promise<void>((resolve) => { started = resolve; });
    let calls = 0;
    const queryFetch: typeof fetch = () => { calls += 1;
      if (calls === 2) { started(); return pending; }
      return Promise.resolve(Response.json(query)); };
    const host = document.createElement("div");
    document.body.append(host);
    const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
    const root = createRoot(host);
    const clipboard = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
    try {
      const props = { workspace: test.workspace, scope: group.project_scope, baseUrl: location.origin,
        fetchImpl: queryFetch, refreshKey: "0" };
      await act(async () => { root.render(createElement(RefusedEditDraftDisplay, props)); });
      await expect.poll(() => host.querySelector("button[data-draft-copy]")).not.toBeNull();
      expect(host.querySelector("button[data-draft-discard]")).not.toBeNull();
      const button = host.querySelector<HTMLButtonElement>(`button[data-draft-${action}]`)!;
      await act(async () => { button.click(); });
      await copyStarted;
      await act(async () => { root.render(createElement(RefusedEditDraftDisplay, { ...props,
        scope: { ...group.project_scope, project_id: ids[0]! } })); });
      await act(async () => { release(Response.json(query)); });
      await expect.poll(() => host.querySelector("[data-draft-unavailable]")).not.toBeNull();
      expect(clipboard).not.toHaveBeenCalled();
      await expect((await import("../../src/refused-edit-discard.ts")).readDiscardJournal(test.workspace)).resolves.toEqual([]);
      expect(host.querySelector("[data-draft-replacement]")).toBeNull();
    } finally { await act(async () => { root.unmount(); }); clipboard.mockRestore(); host.remove();
      Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct }); }
    }
  } finally { await test.close(); }
});

it("freezes one complete mixed Tiptap IME intent only after confirmation and ignores cancellation", async () => {
  const test = await openJournalAppendTestWorkspace();
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const controller = { current: null as import("../../src/manual-input.ts").ManualInputController | null };
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
    await act(async () => { root.render(createElement(ManuscriptEditor, { blocks: test.workspace.pending.blocks,
      proposals: [{ proposalId: id, operationId: id, revisionId: id, blockId: test.workspace.pending.blocks[0]!.manuscript_block_id,
        sourceRunId: id, sourceDecisionId: id, text: "Old", eligible: true, expectedHeads: [id] }],
      editable: true, persistWorkspace: test.workspace, writing, baseUrl: location.origin,
      fetchImpl, cryptoImpl: crypto, controllerRef: controller, onFailure: (error) => { failure = error; } })); });
    await expect.poll(() => host.querySelector(".block-proposal-text")).not.toBeNull();
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    const select = () => {
      surface.focus();
      const first = surface.querySelector("p")!.firstChild!;
      const last = surface.querySelector(".block-proposal-text")!.firstChild!;
      window.getSelection()!.setBaseAndExtent(first, 1, last, 2);
      document.dispatchEvent(new Event("selectionchange"));
    };
    select();
    await applyImeComposition({ text: "provisional", replacementStart: 1, replacementEnd: 8, selectionStart: 11, selectionEnd: 11 });
    expect((await readJournalSnapshot(test.workspace)).records).toEqual([]);
    expect(requests).toBe(0);
    await applyImeComposition({ operation: "cancel" });
    await expect.poll(() => controller.current!.hasIncompleteSemanticIntent()).toBe(false);
    expect((await readJournalSnapshot(test.workspace)).records).toEqual([]);
    expect(requests).toBe(0);
    select();
    await applyImeComposition({ text: "provisional", replacementStart: 1, replacementEnd: 8, selectionStart: 11, selectionEnd: 11 });
    expect((await readJournalSnapshot(test.workspace)).records).toEqual([]);
    await applyTrustedInput({ operation: "insert_text", text: "Complete 中文" });
    await expect.poll(async () => (await readJournalSnapshot(test.workspace)).records.length).toBe(1);
    const [record] = (await readJournalSnapshot(test.workspace)).records;
    expect(record!.input_origin).toBe("composition_confirmation");
    expect(record!.author_edit_unit).toEqual({ normalized_primitives: [{ kind: "replace_structured_selection",
      replacement: [{ block_kind: "paragraph", text: "Complete 中文" }] }],
      selection_snapshot: { coordinate_profile: "storyos.editor.ordered-source.v1", from: 1, to: 2,
        ordered_selection: { anchor: { source_index: 0, source_offset: 1 }, head: { source_index: 1, source_offset: 2 }, sources: [
          { owner: { kind: "manuscript", manuscript_block_id: test.workspace.pending.blocks[0]!.manuscript_block_id },
            coordinate_profile: "prosemirror-token-utf16.v1", from: 1, to: 4, block_kind: "paragraph", source_text: "Base" },
          { owner: { kind: "proposal", proposal_id: id, operation_id: id, revision_id: id,
            manuscript_block_id: test.workspace.pending.blocks[0]!.manuscript_block_id },
            coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 2, block_kind: "paragraph", source_text: "Old" },
        ] } } });
    expect(surface.querySelector("p")!.textContent).toBe("Base");
    expect(surface.querySelector(".block-proposal-text")!.textContent).toBe("Old");
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
  const controller = { current: null as import("../../src/manual-input.ts").ManualInputController | null };
  let failure: unknown, installedBody: string | null | undefined;
  const writing = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin, fetchImpl,
    onFailure: (error) => { failure = error; } });
  function View() {
    const { projection } = useSyncExternalStore(writing.subscribe, writing.snapshot);
    if (projection.authoritative_revision_id !== scenario.session.base_snapshot.authoritative_head_revision_id) {
      queueMicrotask(() => { installedBody ??= host.querySelector("[data-manuscript-editor] p")?.textContent; });
    }
    return createElement(ManuscriptEditor, { blocks: projection.blocks, editable: true, persistWorkspace: test.workspace,
      writing, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto, controllerRef: controller,
      onFailure: (error) => { failure = error; } });
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
    const flushing = controller.current!.flush();
    await requested;
    await applyTrustedInput({ operation: "insert_text", text: "?" });
    expect(surface.querySelector("p")!.textContent).toBe("Base!?");
    await act(async () => { release(); await flushing; await controller.current!.whenIdle(); });
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
  const controller = { current: null as import("../../src/manual-input.ts").ManualInputController | null };
  let failure: unknown, typed = false;
  const initialRevision = test.workspace.pending.authoritative_revision_id;
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
    return createElement(ManuscriptEditor, { blocks: projection.blocks, editable: true,
      persistWorkspace: test.workspace, writing, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
      controllerRef: controller, onFailure: (error) => { failure = error; } });
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
    await controller.current!.flush();
    await expect.poll(() => typed).toBe(true);
    await expect.poll(async () => (await rebuildPendingProjection(test.workspace)).body).toBe("Base!?");
    observer.disconnect();
    expect({ newestReplaced: replaced.includes("Base!?"), text: surface.querySelector("p")!.textContent, failure })
      .toEqual({ newestReplaced: false, text: "Base!?", failure: undefined });
  } finally { root.unmount(); writing.close(); host.remove(); await test.close();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct }); }
});


it("hydrates new blocks when a new inline Proposal projects in the same render", async () => {
  const test = await openJournalAppendTestWorkspace();
  const { ManuscriptEditor } = await import("../../src/manuscript-editor.tsx");
  const block = test.workspace.pending.blocks[0]!;
  const id = "018f0000-0000-7001-8000-000000000090";
  const candidate = (sourceText: string, revisionId: string) => ({ proposalId: id, operationId: id, revisionId,
    blockId: block.manuscript_block_id, sourceRunId: id, sourceDecisionId: id, text: "X", eligible: true,
    expectedHeads: [revisionId], inlineProposal: true, inlineAnchor: { from: 0, to: 3, sourceText } });
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  let failure: unknown;
  const props = { editable: true, persistWorkspace: test.workspace, baseUrl: location.origin,
    fetchImpl: () => new Promise<Response>(() => {}), cryptoImpl: crypto, controllerRef: { current: null },
    onFailure: (error: unknown) => { failure = error; } };
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => { root.render(createElement(ManuscriptEditor, { ...props,
      blocks: [{ ...block, text: "Old base tail" }], proposals: [candidate("Old", "018f0000-0000-7001-8000-000000000091")] })); });
    const paragraph = () => host.querySelector("[data-manuscript-editor] p")!.textContent;
    await expect.poll(paragraph).toBe("X base tail");
    await act(async () => { root.render(createElement(ManuscriptEditor, { ...props,
      blocks: [{ ...block, text: "New base ending" }], proposals: [candidate("New", "018f0000-0000-7001-8000-000000000092")] })); });
    expect({ text: paragraph(), failure }).toEqual({ text: "X base ending", failure: undefined });
  } finally { await act(async () => { root.unmount(); }); host.remove(); await test.close();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct }); }
});


it("keeps captured manuscript input during a background Draft refresh", async () => {
  const test = await openJournalAppendTestWorkspace();
  const { BlockProposalDisplay } = await import("../../src/block-proposal-display.tsx");
  const { applyTrustedInput } = await import("../support/browser-command-client.ts");
  const paused = createPausedDigestCrypto(crypto);
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  const controller = { current: null as import("../../src/manual-input.ts").ManualInputController | null };
  let refresh!: () => void, observed!: () => void;
  const refreshed = new Promise<void>((resolve) => { observed = resolve; });
  let phase = "open", failure: unknown;
  const fetchImpl: typeof fetch = () => { throw new Error("No command is issued before the append completes"); };
  const controlled = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin,
    fetchImpl, cryptoImpl: paused.cryptoImpl, onFailure: (error) => { failure = error; } });
  // The Draft display refreshes the projection through the controller.
  const writing = { ...controlled, async refresh() {
    const projection = await controlled.refresh();
    if (phase === "refresh") observed();
    return projection;
  } };
  function View() {
    const { projection } = useSyncExternalStore(writing.subscribe, writing.snapshot);
    const [refreshKey, setRefreshKey] = useState(0);
    refresh = () => { setRefreshKey(1); };
    return createElement(BlockProposalDisplay, { blocks: projection.blocks, editable: true,
      persistWorkspace: test.workspace, writing, baseUrl: location.origin, cryptoImpl: paused.cryptoImpl,
      fetchImpl, controllerRef: controller,
      scope: test.workspace.session.project_scope, chapterId: test.workspace.session.base_snapshot.chapter_id,
      authoritativeRevisionId: projection.authoritative_revision_id, locators: [], refreshKey,
      safeToProject: true, onAccepted: async () => {}, onFailure: (error) => { failure = error; } });
  }
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => { root.render(createElement(View)); });
    await expect.poll(() => host.querySelector<HTMLElement>("[data-manuscript-editor]")?.contentEditable).toBe("true");
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    surface.focus();
    const text = surface.querySelector("p")!.firstChild!;
    window.getSelection()!.setBaseAndExtent(text, 4, text, 4);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    await paused.reached;
    phase = "refresh";
    await act(async () => { refresh(); });
    await refreshed;
    expect(surface.querySelector("p")!.textContent).toBe("Base!");
    expect(writing.snapshot().projection).toMatchObject({ body: "Base!", save_state: "saving" });
    phase = "append";
    await act(async () => { paused.release(); await controller.current!.whenIdle(); });
    expect((await rebuildPendingProjection(test.workspace)).body).toBe("Base!");
    expect(failure).toBeUndefined();
  } finally { paused.release(); await act(async () => { root.unmount(); }); writing.close(); host.remove(); await test.close();
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
    return createElement(ManuscriptEditor, { blocks: projection.blocks, editable: true, persistWorkspace: test.workspace,
      writing, baseUrl: location.origin, fetchImpl: () => new Promise<Response>(() => {}), cryptoImpl: crypto,
      controllerRef: { current: null }, onFailure: (error) => { failure = error; } });
  }
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => { root.render(createElement(View)); });
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    // A background reader, for example the Draft refresh after a settlement, reads the Journal before the input.
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
    return createElement(ManuscriptEditor, { blocks: projection.blocks, editable: true, persistWorkspace: test.workspace,
      writing, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto, controllerRef: { current: null },
      onFailure: (error) => { failure = error; } });
  }
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => { root.render(createElement(View)); });
    const surface = host.querySelector<HTMLElement>("[data-manuscript-editor]")!;
    // A Draft retry settles outside the editor. The Draft display keeps its hold until this install completes.
    await persistReplaceSelection(test.workspace, FIRST_APPEND_EDIT);
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
