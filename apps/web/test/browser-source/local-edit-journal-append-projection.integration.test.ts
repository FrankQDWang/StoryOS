import { expect, it, vi } from "vitest";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { RefusedEditDraftDisplay } from "../../src/refused-edit-draft-display.tsx";

import { persistReplaceSelection } from "../../src/editor-session.ts";
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
    const { createAuthorEditIdleController } = await import("../../src/author-edit-idle.ts");
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
    const idle = createAuthorEditIdleController({ workspace: test.workspace, baseUrl: "http://localhost",
      onProjection: () => {}, onFailure: (error) => { failure = error; },
      setTimeoutImpl: () => 1, clearTimeoutImpl: () => {} });
    idle.setHoldSubmission(true);
    await Reflect.apply(idle.persist, idle, [{ kind: "structured_selection", authorEditUnit: unit,
      expectedProposalHeads: ["018f0000-0000-7001-8000-000000000092"] }, "typing", new Date().toISOString()]);
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
      const button = host.querySelector<HTMLButtonElement>("button[data-draft-copy]")!;
      await act(async () => { button.click(); });
      await copyStarted;
      await act(async () => { root.render(createElement(RefusedEditDraftDisplay, { ...props,
        scope: { ...group.project_scope, project_id: ids[0]! } })); });
      await act(async () => { release(Response.json(query)); });
      await expect.poll(() => host.querySelector("[data-draft-unavailable]")).not.toBeNull();
      expect(clipboard).not.toHaveBeenCalled();
      expect(host.querySelector("[data-draft-replacement]")).toBeNull();
    } finally { await act(async () => { root.unmount(); }); clipboard.mockRestore(); host.remove();
      Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct }); }
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
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => { root.render(createElement(ManuscriptEditor, { blocks: test.workspace.pending.blocks,
      proposals: [{ proposalId: id, operationId: id, revisionId: id, blockId: test.workspace.pending.blocks[0]!.manuscript_block_id,
        sourceRunId: id, sourceDecisionId: id, text: "Old", eligible: true, expectedHeads: [id] }],
      editable: true, persistWorkspace: test.workspace, baseUrl: location.origin,
      fetchImpl: () => { requests += 1; return new Promise<Response>(() => {}); }, cryptoImpl: crypto,
      controllerRef: controller, onProjection: () => {}, onFailure: (error) => { failure = error; } })); });
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
  } finally { await act(async () => { root.unmount(); }); host.remove(); await test.close();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct }); }
});
