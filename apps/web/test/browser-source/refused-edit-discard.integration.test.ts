import { expect, it, vi } from "vitest";
import { discardRefusedEdit, observeDiscard, readDiscardJournal, reconcileDiscard } from "../../src/refused-edit-discard.ts";
import { rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import type { CloseEditorFlowDraftRequest, EditorFlowDraftClosed, RefusedEditDraftInspect }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";

it("persists the complete immutable explicit Discard before Admission and only reconciles its exact public event", async () => {
  const test = await openJournalAppendTestWorkspace();
  const id = "018f0000-0000-7001-8000-000000000090";
  const draft = { draft_id: id, draft_revision_id: id, payload_digest: "a".repeat(64),
    closure: "open", retention_state: "retained", kind: "refused_edit" } as RefusedEditDraftInspect;
  let request: CloseEditorFlowDraftRequest | undefined;
  const routes: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    routes.push(String(input));
    if (String(input).includes("/editor-sessions/")) return Response.json(test.workspace.session);
    if (String(input).endsWith("/anti-forgery-challenges")) {
      const record = (await readDiscardJournal(test.workspace))[0]!.record;
      expect(record!.group).toMatchObject({ ordered_coverage: [{
        local_intent_sequence: record!.local_intent_sequence,
        intent_record_ref: record!.explicit_command_record_id, payload_digest: record!.group.frozen_request_digest,
      }] });
      expect(record!.exact_target_head_anchor_bindings).toEqual({ draft_id: id, draft_revision_id: id,
        payload_digest: "a".repeat(64), expected_closure: "open" });
      expect(JSON.stringify(record)).not.toContain("nonce");
      return Response.json({ nonce: "b".repeat(64), expires_at: new Date(Date.now() + 60000).toISOString(),
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    request = JSON.parse(String(init?.body)) as CloseEditorFlowDraftRequest;
    throw new TypeError("Controlled response loss");
  };
  try {
    await expect(discardRefusedEdit({ workspace: test.workspace, draft, baseUrl: location.origin, fetchImpl }))
      .rejects.toThrow("Controlled response loss");
    const [original] = await readDiscardJournal(test.workspace);
    expect(original!.observation).toMatchObject({ kind: "unresolved" });
    expect(original!.record.group.frozen_request_body).toEqual(request);
    expect(await rebuildPendingProjection(test.workspace)).toMatchObject({ save_state: "needs_attention", unsettled_intent_count: 1 });
    const event: EditorFlowDraftClosed = { schema_id: "storyos.event.editor-flow-draft-closed.v1",
      event_kind: "editor_flow_draft_closed", event_id: id, project_scope: test.workspace.partition.project_scope,
      draft_id: id, draft_revision_id: id, payload_digest: draft.payload_digest, prior_closure: "open",
      closure: "closed", close_reason: "abandoned", source: { command_id: id, author_command_admission_id: id,
        receipt_id: id, idempotency_key: original!.record.group.idempotency_key,
        command_digest: original!.record.group.frozen_request_digest }, author_action_sequence: "1",
      created_at: "2026-09-26T12:00:00.000Z" };
    expect(await reconcileDiscard(test.workspace, { ...draft, closure: "closed", closure_event: {
      ...event, source: { ...event.source, idempotency_key: id } } })).toMatchObject({ kind: "unresolved" });
    expect(await reconcileDiscard(test.workspace, draft)).toMatchObject({ kind: "unresolved" });
    const postsBeforeReload = routes.length;
    expect(await reconcileDiscard(test.workspace, { ...draft, closure: "closed", closure_event: event }))
      .toMatchObject({ kind: "settled_closed", event });
    expect(await rebuildPendingProjection(test.workspace)).toMatchObject({ save_state: "saved", unsettled_intent_count: 0 });
    const clock = vi.spyOn(Date, "now").mockReturnValue(Date.parse("2030-01-01T00:00:00.000Z"));
    try { await observeDiscard(test.workspace, original!.record, { kind: "unresolved" }); }
    finally { clock.mockRestore(); }
    expect(await rebuildPendingProjection(test.workspace)).toMatchObject({ save_state: "saved", unsettled_intent_count: 0 });
    expect((await readDiscardJournal(test.workspace))[0]!.record).toEqual(original!.record);
    expect(routes.length).toBe(postsBeforeReload);
    test.workspace.partition = { ...test.workspace.partition, client_session_generation: "2" };
    await expect(discardRefusedEdit({ workspace: test.workspace, draft, baseUrl: location.origin, fetchImpl }))
      .rejects.toThrow("settled current writer");
    expect(routes.filter((route) => route.endsWith("/closures"))).toHaveLength(1);
    const transaction = test.workspace.database.transaction("metadata", "readwrite");
    transaction.objectStore("metadata").put({ ...original!.record, nonce: "secret" });
    await new Promise<void>((resolve) => { transaction.oncomplete = () => resolve(); });
    await expect(readDiscardJournal(test.workspace)).rejects.toThrow("Journal unavailable");
  } finally { await test.close(); }
});

it("keeps exact Receipt-backed refusal and conflict distinct from an infrastructure outcome", async () => {
  for (const kind of ["refused", "conflicted"] as const) {
    const test = await openJournalAppendTestWorkspace();
    const id = "018f0000-0000-7001-8000-000000000090";
    const draft = { draft_id: id, draft_revision_id: id, payload_digest: "a".repeat(64),
      closure: "open", retention_state: "retained", kind: "refused_edit" } as RefusedEditDraftInspect;
    let expected: import("../../../../generated/typescript/storyos-public-release-1/client.mjs").CloseEditorFlowDraftResponse | undefined;
    const fetchImpl: typeof fetch = async (input, init) => {
      if (String(input).includes("/editor-sessions/")) return Response.json(test.workspace.session);
      if (String(input).endsWith("/anti-forgery-challenges")) return Response.json({ nonce: "b".repeat(64),
        expires_at: new Date(Date.now() + 60000).toISOString(), limit_profile_revision: "storyos.foundation.absolute.v1" });
      const request = JSON.parse(String(init?.body)) as CloseEditorFlowDraftRequest;
      const record = (await readDiscardJournal(test.workspace))[0]!.record;
      const scope = test.workspace.partition.project_scope;
      expected = { schema_id: "storyos.command.close-editor-flow-draft.response.v1",
        correlation_id: request.close_editor_flow_draft_input.correlation_id, project_scope: scope,
        command_id: id, author_command_admission_id: id, effect: kind === "refused"
          ? { kind, reason: "source_unavailable", current_closure: "open" }
          : { kind, current_revision_id: id, current_digest: "0".repeat(64), current_closure: "open" },
        receipt: { receipt_id: id, project_scope: scope, command_kind: "closeEditorFlowDraft",
          command_digest: record.group.frozen_request_digest, idempotency_key: record.group.idempotency_key,
          producer_cause: "author_command_admission", author_command_admission_id: id,
          expected_heads: [], prior_heads: [], resulting_heads: [], authoritative_revision_ids: [],
          proposal_revision_ids: [], authoritative_commit_ids: [], author_action_sequence: null,
          draft_artifact_refs: [id], artifact_lifecycle_event_refs: [], condition_refs: [], result: kind,
          created_at: "2026-09-26T12:00:00.000Z" } };
      return Response.json(expected);
    };
    try {
      await discardRefusedEdit({ workspace: test.workspace, draft, baseUrl: location.origin, fetchImpl });
      expect(await reconcileDiscard(test.workspace, draft)).toMatchObject({ kind: "settled", response: expected });
      expect(await rebuildPendingProjection(test.workspace)).toMatchObject({ save_state: "saved", unsettled_intent_count: 0 });
    } finally { await test.close(); }
  }
});
