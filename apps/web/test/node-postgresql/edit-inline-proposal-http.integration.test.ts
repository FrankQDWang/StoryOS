// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/edit-proposal-candidate-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "vitest";
import {
  StoryOSProtocolError, closeEditorFlowDraft, digestCloseEditorFlowDraft, undoLatestAuthorAction, digestUndoLatestAuthorAction,
  expandRefusedEditDraftToProposal, digestExpandRefusedEditDraft,
  takeOverProjectWriter, digestTakeOverProjectWriter,
  acceptProposal, applyAuthorEdit, archiveProject, createAgentRun, createEditorSession,
  digestAcceptProposal, digestApplyAuthorEdit, digestArchiveProject, digestCreateAgentRun,
  digestCreateEditorSession, digestExportProjectArchive, digestRejectProposalOperations,
  exportProjectArchive, getAgentRun, getChapter, getExportOperation, getProposal, getRefusedEditDraft, getApplyAuthorEditOutcome,
  rejectProposalOperations,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CloseEditorFlowDraftRequest, CloseEditorFlowDraftResponse, AcceptProposalRequest, ApplyAuthorEditRequest, CreateAgentRunRequest,
  CreateEditorSessionRequest, RejectProposalOperationsRequest, SelectedEditSource,
  ExpandRefusedEditDraftRequest, ReplacementBlock,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { requireStoryOSProtocolError, sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer, queryStoryOSPostgres as queryPostgres } from "../support/node-integration.ts";
import { retainRefusedEditRecoveryExpectation } from "../support/refused-edit-recovery.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { BINDING, PROSE, USER_A, UUID_V7, challenged, drainLeftoverWork, id,
  prepare, settleOnce, startRealServer } from "../support/acceptance.ts";

const INLINE_CANDIDATE = "narrator tone";
const SOURCE_SLICE = "narrator voice";
const GOLDEN_DIGEST =
  "sha256:bc395ed925fa201c3c0ecd550ed362f89e6069001d6c23af2ce214b107d9ce40";

function sliceDigest(manuscriptBlockId: string): string {
  return `sha256:${createHash("sha256").update(JSON.stringify({
    base_slice: SOURCE_SLICE,
    block_kind: "paragraph",
    coordinate_profile: "prosemirror-token-utf16.v1",
    from: 10,
    manuscript_block_id: manuscriptBlockId,
    manuscript_schema_version: 1,
    to: 24,
  })).digest("hex")}`;
}

const INTERIOR_CANDIDATE = "narrxxr tone";
const ACCEPTED_BODY = "Guard the narrxxr tone in this passage.";
const EDGE_BODY = "Keep the narrator voice in this passage.";

function phraseRequest(chapterId: string, correlationId: string): CreateAgentRunRequest {
  return {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation: { kind: "new" },
      author_message: { text: "Revise this phrase: keep the voice." },
      working_target: { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

async function writePassage(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  ns: string,
  text = PROSE,
) {
  const sessionRequest: CreateEditorSessionRequest = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}1`),
  };
  const session = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/editor-sessions",
    sessionRequest.command_schema, await digestCreateEditorSession(sessionRequest),
    id(`${ns}2`),
    (antiForgery) => createEditorSession({
      baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}2`), antiForgery,
      request: sessionRequest,
    }),
  );
  if (session.writer.kind !== "current_writer") throw new Error("expected current writer");
  const editRequest: ApplyAuthorEditRequest = {
    command_schema: "storyos.command.apply-author-edit.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}3`),
    editor_session_id: session.editor_session.editor_session_id,
    writer_generation: session.writer.writer_generation,
    chapter_id: session.base_snapshot.chapter_id,
    expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
    expected_proposal_head_revision_ids: [],
    target_refs: session.base_snapshot.target_refs,
    observed_ownership_partition: "authoritative",
    editor_contract_revision: "storyos.editor-contract.release-1.v3",
    undo_group_id: id(`${ns}4`),
    completed_intent_record_id: id(`${ns}5`),
    local_intent_sequence: "1",
    author_edit_units: [{
      normalized_primitives: [{ kind: "replace_selection", from: 0, to: 0, text }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 0,
      },
    }],
  };
  const written = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/manuscript/author-edits",
    editRequest.command_schema, await digestApplyAuthorEdit(editRequest), id(`${ns}6`),
    (antiForgery) => applyAuthorEdit({
      baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}6`), antiForgery,
      request: editRequest,
    }),
  );
  if (written.effect.kind !== "authoritative_applied") {
    throw new Error("expected written prose");
  }
  return {
    session,
    writerGeneration: session.writer.writer_generation,
    authoritativeRevisionId: written.effect.authoritative_revision.revision_id,
    nextSequence: "2",
  };
}

async function admitPhrase(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  key: string,
) {
  const request = phraseRequest(chapterId, id(key.slice(-4)));
  const created = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/agent-runs",
    request.command_schema, await digestCreateAgentRun(request), key,
    (antiForgery) => createAgentRun({
      baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery, request,
    }),
  );
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  await settleOnce();
  return getAgentRun({ baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
}

async function openInline(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  ns: string,
) {
  const writer = await writePassage(baseUrl, fetchImpl, projectId, `${ns}0`);
  const queried = await admitPhrase(baseUrl, fetchImpl, projectId, chapterId, id(`${ns}1`));
  assert.equal(queried.status, "completed");
  assert.equal(queried.decision.kind, "prose_change");
  if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
    throw new Error("expected opened inline");
  }
  const opened = await getProposal({
    baseUrl, projectId, proposalId: queried.decision.opened_proposal.proposal_id, fetchImpl,
  });
  assert.equal(opened.proposal.kind, "inline_edit");
  assert.equal(opened.proposal.candidate_text, INLINE_CANDIDATE);
  assert.equal(opened.proposal.validation, "valid");
  assert.equal(opened.proposal.validation_receipt.kind, "present");
  assert.equal(opened.proposal.anchors.length, 1);
  const anchor = opened.proposal.anchors[0];
  if (anchor === undefined) throw new Error("expected one Anchor");
  assert.equal(anchor.manuscript_block_id, opened.proposal.manuscript_block_id);
  assert.equal(anchor.coordinate_profile, "prosemirror-token-utf16.v1");
  assert.equal(anchor.boundary_profile, "exclusive-authoritative-edges.v1");
  assert.equal(anchor.from, 10);
  assert.equal(anchor.to, 24);
  assert.equal(PROSE.slice(anchor.from, anchor.to), SOURCE_SLICE);
  assert.equal(sliceDigest("block-a"), GOLDEN_DIGEST);
  assert.equal(anchor.base_slice_digest, sliceDigest(anchor.manuscript_block_id));
  return { queried, opened, writer };
}

function replaceUnit(
  from: number,
  to: number,
  text: string,
  writer: Awaited<ReturnType<typeof writePassage>>,
  ns: string,
  proposalRevisionId: string,
): ApplyAuthorEditRequest {
  return {
    command_schema: "storyos.command.apply-author-edit.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}3`),
    editor_session_id: writer.session.editor_session.editor_session_id,
    writer_generation: writer.writerGeneration,
    chapter_id: writer.session.base_snapshot.chapter_id,
    expected_authoritative_revision_id: writer.authoritativeRevisionId,
    expected_proposal_head_revision_ids: [proposalRevisionId],
    target_refs: writer.session.base_snapshot.target_refs,
    observed_ownership_partition: "mixed",
    editor_contract_revision: "storyos.editor-contract.release-1.v3",
    undo_group_id: id(`${ns}4`),
    completed_intent_record_id: id(`${ns}5`),
    local_intent_sequence: writer.nextSequence,
    author_edit_units: [{
      normalized_primitives: [{ kind: "replace_selection", from, to, text }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to,
      },
    }],
  };
}

function mixedRequest(
  opened: Awaited<ReturnType<typeof getProposal>>,
  writer: Awaited<ReturnType<typeof writePassage>>,
  ns: string,
): ApplyAuthorEditRequest {
  const request = replaceUnit(8, 26, "New passage", writer, ns, opened.proposal.revision_id);
  request.author_edit_units = [{ normalized_primitives: [{ kind: "replace_structured_selection",
    replacement: [{ block_kind: "paragraph", text: "New passage" }] }],
    selection_snapshot: { coordinate_profile: "storyos.editor.ordered-source.v1", from: 8, to: 26,
      ordered_selection: { anchor: { source_index: 0, source_offset: 8 },
        head: { source_index: 2, source_offset: 26 }, sources: [
          { owner: { kind: "manuscript", manuscript_block_id: opened.proposal.manuscript_block_id },
            coordinate_profile: "prosemirror-token-utf16.v1", from: 8, to: 10,
            block_kind: "paragraph", source_text: PROSE },
          { owner: { kind: "proposal", proposal_id: opened.proposal.proposal_id,
            operation_id: opened.proposal.operation_id, revision_id: opened.proposal.revision_id,
            manuscript_block_id: opened.proposal.manuscript_block_id },
            coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: INLINE_CANDIDATE.length,
            block_kind: "paragraph", source_text: INLINE_CANDIDATE },
          { owner: { kind: "manuscript", manuscript_block_id: opened.proposal.manuscript_block_id },
            coordinate_profile: "prosemirror-token-utf16.v1", from: 24, to: 26,
            block_kind: "paragraph", source_text: PROSE },
        ] } } }];
  return request;
}

async function retainedState(projectId: string) {
  const tables = ["authoritative_heads", "authoritative_revisions", "authoritative_commits",
    "author_action_entries", "project_activity_events", "scope_counters", "proposals",
    "proposal_heads", "proposal_revisions", "proposal_operations", "draft_artifacts",
    "draft_artifact_revisions", "draft_lifecycle_events", "draft_close_events", "draft_reopen_events", "draft_reopen_receipts"];
  return JSON.parse(await queryPostgres(`SELECT jsonb_build_object(${tables.map((table) =>
    `'${table}', (SELECT coalesce(jsonb_agg(to_jsonb(record) ORDER BY to_jsonb(record)::text), '[]'::jsonb)
      FROM storyos.${table} AS record WHERE record.owner_user_id = '${USER_A}'::uuid
      AND record.project_id = '${projectId}'::uuid)`).join(",")})::text`));
}

async function sendMixed(baseUrl: string, prepared: Awaited<ReturnType<typeof prepare>>,
  request: ApplyAuthorEditRequest, key: string) {
  return challenged(baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
    "/api/v1/projects/{project_id}/manuscript/author-edits", request.command_schema,
    await digestApplyAuthorEdit(request), key, (antiForgery) => applyAuthorEdit({ baseUrl,
      projectId: prepared.projectId, fetchImpl: prepared.fetchImpl, request,
      idempotencyKey: key, antiForgery }));
}

test("whole Draft expansion preserves structured content in a fresh pending Proposal and supersedes its source", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e0fb11"), "Whole Draft Expansion", "e0fb2");
    const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, prepared.chapterId, "e0fb3");
    const original = mixedRequest(opened, writer, "e0fb4");
    const replacement: ReplacementBlock[] = [{ block_kind: "heading", text: "完整标题🙂" },
      { block_kind: "paragraph", text: "First line\n第二行" }];
    original.author_edit_units[0]!.normalized_primitives = [{ kind: "replace_structured_selection", replacement }];
    const created = await sendMixed(started.baseUrl, prepared, original, id("e0fb46"));
    if (created.effect.kind !== "refused_to_draft") throw new Error("expected complete source Draft");
    const source = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: created.effect.draft_id, fetchImpl: prepared.fetchImpl })).draft;
    const guardCreated = await sendMixed(started.baseUrl, prepared, { ...original, correlation_id: id("e0fb901"),
      undo_group_id: id("e0fb902"), completed_intent_record_id: id("e0fb903"),
      local_intent_sequence: String(BigInt(original.local_intent_sequence) + 1n) }, id("e0fb904"));
    if (guardCreated.effect.kind !== "refused_to_draft") throw new Error("expected retained guard Draft");
    const guardSource = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: guardCreated.effect.draft_id, fetchImpl: prepared.fetchImpl })).draft;
    const rejection: RejectProposalOperationsRequest = {
      command_schema: "storyos.command.reject-proposal-operations.request.v1",
      reject_proposal_operations_input: { ...BINDING, correlation_id: id("e0fb51"),
        editor_session_id: writer.session.editor_session.editor_session_id,
        proposal_revision_id: opened.proposal.revision_id,
        selected_pending_operation_ids: [opened.proposal.operation_id],
        expected_target_revisions: [writer.authoritativeRevisionId],
        rejection_reason: { kind: "author_declined", note: { kind: "omitted" } } },
    };
    await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections", rejection.command_schema,
      await digestRejectProposalOperations(rejection), id("e0fb52"), (antiForgery) => rejectProposalOperations({
        baseUrl: started.baseUrl, projectId: prepared.projectId, proposalId: opened.proposal.proposal_id,
        request: rejection, fetchImpl: prepared.fetchImpl, antiForgery, idempotencyKey: id("e0fb52") }));
    const request: ExpandRefusedEditDraftRequest = { command_schema: "storyos.command.expand-refused-edit-draft-to-proposal.request.v1",
      expand_refused_edit_draft_to_proposal_input: { ...BINDING, correlation_id: id("e0fb61"),
        draft_id: source.draft_id, source_current_draft_revision_id: source.draft_revision_id,
        source_draft_payload_digest: source.payload_digest, expected_source_draft_closure: "open",
        selected_payload_range: { kind: "whole_draft_payload" }, proposal_kind: "inline_edit",
        chapter_id: prepared.chapterId, target_refs: [opened.proposal.manuscript_block_id],
        expected_target_revisions: [writer.authoritativeRevisionId], anchors: opened.proposal.anchors,
        editor_session_id: writer.session.editor_session.editor_session_id, writer_generation: writer.writerGeneration } };
    const before = await retainedState(prepared.projectId);
    const sendExpansion = async (body: ExpandRefusedEditDraftRequest, key: string, fetchImpl = prepared.fetchImpl) =>
      challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
        "/api/v1/projects/{project_id}/drafts/{draft_id}/proposal-expansions", body.command_schema,
        await digestExpandRefusedEditDraft(body), key, (antiForgery) => expandRefusedEditDraftToProposal({
          baseUrl: started.baseUrl, projectId: prepared.projectId, draftId: body.expand_refused_edit_draft_to_proposal_input.draft_id,
          fetchImpl, request: body, antiForgery, idempotencyKey: key }));
    for (const [index, patch] of [{ source_draft_payload_digest: "0".repeat(64) },
      { expected_target_revisions: [id("e0fb899")] }, { writer_generation: "999" }].entries()) {
      const changed = { ...request, expand_refused_edit_draft_to_proposal_input: { ...request.expand_refused_edit_draft_to_proposal_input, ...patch,
        ...(index === 1 ? { anchors: [{ ...request.expand_refused_edit_draft_to_proposal_input.anchors[0]!, base_authoritative_revision_id: id("e0fb899") }] } : {}) } };
      if (index === 2) await assert.rejects(() => sendExpansion(changed, id(`e0fb8${index}`)), (error) => requireStoryOSProtocolError(error).status === 409);
      else assert.equal((await sendExpansion(changed, id(`e0fb8${index}`))).effect.kind, "conflicted");
      assert.deepEqual(await retainedState(prepared.projectId), before);
    }
    await assert.rejects(() => sendExpansion(request, id("e0fb83"), browserFetch(started.baseUrl, "session-b")),
      (error) => requireStoryOSProtocolError(error).status === 422);
    await assert.rejects(() => getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: source.draft_id, fetchImpl: browserFetch(started.baseUrl, "session-b") }), (error) => requireStoryOSProtocolError(error).status === 404);
    await queryPostgres(`CREATE FUNCTION storyos.fail_expand_385() RETURNS trigger LANGUAGE plpgsql AS $fault$
      BEGIN IF NEW.project_id='${prepared.projectId}'::uuid THEN RAISE EXCEPTION 'Controlled expansion failure'; END IF; RETURN NULL; END $fault$;
      CREATE CONSTRAINT TRIGGER fail_expand_385 AFTER INSERT ON storyos.draft_close_events DEFERRABLE INITIALLY DEFERRED
      FOR EACH ROW EXECUTE FUNCTION storyos.fail_expand_385();`);
    try { await assert.rejects(() => sendExpansion(request, id("e0fb84")), (error) => requireStoryOSProtocolError(error).status === 503);
      assert.deepEqual(await retainedState(prepared.projectId), before); }
    finally { await queryPostgres("DROP TRIGGER fail_expand_385 ON storyos.draft_close_events; DROP FUNCTION storyos.fail_expand_385()"); }
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    let arrivals = 0;
    const concurrentFetch: typeof fetch = async (url, init) => {
      if (++arrivals === 2) release();
      await gate;
      return prepared.fetchImpl(url, init);
    };
    const attempts = await Promise.allSettled([0, 1].map(() => sendExpansion(request, id("e0fb62"), concurrentFetch)));
    const responses = attempts.flatMap((attempt) => attempt.status === "fulfilled" ? [attempt.value] : []);
    assert.ok(responses.length > 0);
    for (const attempt of attempts) if (attempt.status === "rejected") assert.ok([409, 503].some((status) => status === requireStoryOSProtocolError(attempt.reason).status));
    const response = responses[0]!;
    for (const observed of responses) assert.deepEqual(observed, response);
    if (response.effect.kind !== "proposal_created_from_draft") throw new Error("expected expanded Proposal");
    const proposal = (await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: response.effect.proposal_id, fetchImpl: prepared.fetchImpl })).proposal;
    assert.equal(proposal.validation, "pending");
    assert.deepEqual(proposal.candidate_blocks, replacement);
    assert.deepEqual(proposal.operations.map((operation) => operation.candidate_blocks), [replacement]);
    assert.equal(proposal.candidate_text, "完整标题🙂\nFirst line\n第二行");
    assert.deepEqual(proposal.source, { kind: "refused_edit_draft", draft_id: source.draft_id,
      draft_revision_id: source.draft_revision_id, payload_digest: source.payload_digest, payload: source.payload });
    const closed = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: source.draft_id, fetchImpl: prepared.fetchImpl })).draft;
    assert.deepEqual(closed.payload, source.payload);
    assert.equal(closed.closure_event?.close_reason, "superseded");
    assert.deepEqual(response.receipt.authoritative_commit_ids, []);
    const afterExpansion = await retainedState(prepared.projectId);
    assert.deepEqual(afterExpansion.authoritative_heads, before.authoritative_heads);
    assert.deepEqual(afterExpansion.authoritative_commits, before.authoritative_commits);
    assert.equal((await sendExpansion(request, id("e0fb85"))).effect.kind, "refused");
    assert.deepEqual(await retainedState(prepared.projectId), afterExpansion);
    assert.deepEqual(await sendExpansion(request, id("e0fb62")), response);
    const undo = { command_schema: "storyos.command.undo-latest-author-action.request.v1",
      undo_latest_author_action_input: { ...BINDING, correlation_id: id("e0fb71"),
        editor_session_id: writer.session.editor_session.editor_session_id,
        expected_authoritative_revision_id: writer.authoritativeRevisionId,
        expected_author_undo_frontier_sequence: response.receipt.author_action_sequence! } };
    const sendUndo = async (body: typeof undo, key: string) => challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/author-actions/undo", body.command_schema, await digestUndoLatestAuthorAction(body), key,
      (antiForgery) => undoLatestAuthorAction({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl, request: body, antiForgery, idempotencyKey: key }));
    const undone = await sendUndo(undo, id("e0fb72"));
    assert.equal(undone.effect.kind, "draft_compensated");
    const reopened = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: source.draft_id, fetchImpl: prepared.fetchImpl })).draft;
    assert.equal(reopened.closure, "open");
    assert.deepEqual(reopened.payload, source.payload);
    const withdrawn = (await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: proposal.proposal_id, fetchImpl: prepared.fetchImpl })).proposal;
    assert.equal(withdrawn.closure, "withdrawn");
    assert.deepEqual(withdrawn.candidate_blocks, replacement);
    assert.equal(withdrawn.reservation_state, "resolved");
    assert.deepEqual(await sendUndo(undo, id("e0fb72")), undone);
    const guardExpansion = await sendExpansion({ ...request, expand_refused_edit_draft_to_proposal_input: {
      ...request.expand_refused_edit_draft_to_proposal_input, draft_id: guardSource.draft_id,
      source_current_draft_revision_id: guardSource.draft_revision_id, source_draft_payload_digest: guardSource.payload_digest } }, id("e0fb905"));
    if (guardExpansion.effect.kind !== "proposal_created_from_draft") throw new Error("expected guard expansion");
    const guardProposal = (await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: guardExpansion.effect.proposal_id, fetchImpl: prepared.fetchImpl })).proposal;
    assert.equal(await queryPostgres(`WITH advanced AS (INSERT INTO storyos.proposal_revisions
      (owner_user_id,project_id,proposal_id,revision_id,generation,validation,closure,candidate_text,base_authoritative_revision_id,parent_revision_id,candidate_blocks)
      SELECT owner_user_id,project_id,proposal_id,'${id("e0fb906")}'::uuid,generation,validation,closure,candidate_text,base_authoritative_revision_id,revision_id,candidate_blocks
      FROM storyos.proposal_revisions WHERE owner_user_id='${USER_A}'::uuid AND project_id='${prepared.projectId}'::uuid
        AND proposal_id='${guardProposal.proposal_id}'::uuid AND revision_id='${guardProposal.revision_id}'::uuid RETURNING *), moved AS (
      UPDATE storyos.proposal_heads AS head SET current_revision_id=advanced.revision_id FROM advanced
      WHERE (head.owner_user_id,head.project_id,head.proposal_id)=(advanced.owner_user_id,advanced.project_id,advanced.proposal_id)
        AND head.current_revision_id='${guardProposal.revision_id}'::uuid RETURNING 1) SELECT count(*)::text FROM moved`), "1");
    const advancedRevisionId = id("e0fb906");
    const guardComparison = guardProposal.revision_comparison;
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: guardProposal.proposal_id, fetchImpl: prepared.fetchImpl })).proposal, {
      ...guardProposal,
      revision_id: advancedRevisionId,
      revision_comparison: guardComparison.kind === "present"
        ? { ...guardComparison, candidate_revision_id: advancedRevisionId }
        : guardComparison,
    });
    const guardUndo = { ...undo, undo_latest_author_action_input: { ...undo.undo_latest_author_action_input,
      expected_author_undo_frontier_sequence: guardExpansion.receipt.author_action_sequence! } };
    const beforeDriftUndo = await retainedState(prepared.projectId);
    assert.deepEqual((await sendUndo(guardUndo, id("e0fb907"))).effect, { kind: "conflicted", reason: "source_binding_changed" });
    assert.deepEqual(await retainedState(prepared.projectId), beforeDriftUndo);
    await queryPostgres(`UPDATE storyos.draft_artifacts SET retention_state='tombstoned' WHERE owner_user_id='${USER_A}'::uuid
      AND project_id='${prepared.projectId}'::uuid AND draft_id='${guardSource.draft_id}'::uuid`);
    const beforeUnavailableUndo = await retainedState(prepared.projectId);
    assert.deepEqual((await sendUndo(guardUndo, id("e0fb908"))).effect, { kind: "unavailable", reason: "source_unavailable" });
    assert.deepEqual(await retainedState(prepared.projectId), beforeUnavailableUndo);
    await retainRefusedEditRecoveryExpectation(prepared.projectId, [{ draft: reopened, available: true },
      { draft: guardSource, available: false }], [], [withdrawn]);
  } finally { await stopRealServer(started.server); }
});

test("an explicit narrowed public Draft retry changes content and supersedes only its exact source", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e0c9111"), "Narrow Draft Retry", "e0c92");
    const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, prepared.chapterId, "e0c93");
    const created = await sendMixed(started.baseUrl, prepared,
      mixedRequest(opened, writer, "e0c94"), id("e0c946"));
    if (created.effect.kind !== "refused_to_draft") throw new Error("expected complete public Draft");
    const source = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: created.effect.draft_id, fetchImpl: prepared.fetchImpl })).draft;
    const selected = [{ block_kind: "paragraph", text: "New" }];
    const request: ApplyAuthorEditRequest = { ...replaceUnit(0, 5, "New", writer, "e0c95", opened.proposal.revision_id),
      local_intent_sequence: String(BigInt(writer.nextSequence) + 1n),
      retry_source: { kind: "draft_retry", source_draft_kind: "refused_edit",
        source_draft_id: source.draft_id, source_current_draft_revision_id: source.draft_revision_id,
        source_draft_payload_digest: source.payload_digest, expected_source_draft_closure: "open",
        selected_payload_range: { kind: "exact_structured_range",
          coordinate_profile: "storyos.draft-replacement.block-utf16.v1",
          from: { block_index: 0, offset: 0 }, to: { block_index: 0, offset: 3 },
          slice_digest: createHash("sha256").update(JSON.stringify(selected)).digest("hex") } } };
    const retried = await sendMixed(started.baseUrl, prepared, request, id("e0c956")).catch((error: unknown) => {
      if (error instanceof StoryOSProtocolError) throw new Error(error.responseBody, { cause: error });
      throw error;
    });
    assert.equal(retried.effect.kind, "authoritative_applied");
    const closed = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: source.draft_id, fetchImpl: prepared.fetchImpl })).draft;
    assert.deepEqual({ ...closed, closure: source.closure, closure_event: undefined },
      { ...source, closure_event: undefined });
    assert.equal(closed.closure, "closed");
    assert.equal(closed.closure_event?.close_reason, "superseded");
    assert.equal(closed.closure_event?.source.receipt_id, retried.receipt.receipt_id);
    assert.deepEqual(await sendMixed(started.baseUrl, prepared, request, id("e0c956")), retried);
    if (retried.effect.kind !== "authoritative_applied") throw new Error("expected content retry");
    const undo = { command_schema: "storyos.command.undo-latest-author-action.request.v1",
      undo_latest_author_action_input: { ...BINDING, correlation_id: id("e0c961"),
        editor_session_id: writer.session.editor_session.editor_session_id,
        expected_author_undo_frontier_sequence: retried.effect.author_action_sequence,
        expected_authoritative_revision_id: retried.effect.authoritative_revision.revision_id } };
    const compensate = async () => challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/author-actions/undo", undo.command_schema,
      await digestUndoLatestAuthorAction(undo), id("e0c962"), (antiForgery) => undoLatestAuthorAction({
        baseUrl: started.baseUrl, projectId: prepared.projectId, request: undo,
        idempotencyKey: id("e0c962"), antiForgery, fetchImpl: prepared.fetchImpl }));
    const undone = await compensate();
    assert.equal(undone.effect.kind, "compensated");
    assert.equal(undone.source_reopen_event?.source_close_event_id, closed.closure_event?.event_id);
    assert.deepEqual(undone.receipt.artifact_lifecycle_event_refs, [undone.source_reopen_event?.event_id]);
    assert.deepEqual(await compensate(), undone);
    const reopened = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: source.draft_id, fetchImpl: prepared.fetchImpl })).draft;
    assert.deepEqual(reopened, { ...source, closure_event: closed.closure_event, reopen_event: undone.source_reopen_event });
    await retainRefusedEditRecoveryExpectation(prepared.projectId, [{ draft: reopened, available: true }]);
  } finally { await stopRealServer(started.server); }
});

test("narrow Draft retry preserves candidate, refusal, no-effect and conflict outcomes with exact source replay", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    for (const [index, outcome] of ["proposal_revised", "refused_to_draft", "no_effect", "conflicted", "source_conflict", "split_surrogate", "changed_content", "reserved_block"].entries()) {
      const ns = `e0c97${index}`;
      const prepared = await prepare(started.baseUrl, id(`${ns}11`), "Draft Retry Outcomes", `${ns}2`);
      const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
        prepared.projectId, prepared.chapterId, `${ns}3`);
      const original = mixedRequest(opened, writer, `${ns}4`);
      original.author_edit_units[0]!.normalized_primitives = [{ kind: "replace_structured_selection",
        replacement: [{ block_kind: "paragraph", text: "Guard🙂New passage" }] }];
      const created = await sendMixed(started.baseUrl, prepared, original, id(`${ns}46`));
      if (created.effect.kind !== "refused_to_draft") throw new Error("expected complete source Draft");
      const source = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        draftId: created.effect.draft_id, fetchImpl: prepared.fetchImpl })).draft;
      const unchanged = { kind: "unchanged", source_draft_kind: "refused_edit", source_draft_id: source.draft_id,
        requested_source_draft_revision_id: source.draft_revision_id,
        current_source_draft_revision_id: source.draft_revision_id,
        current_source_draft_payload_digest: source.payload_digest, current_closure: { kind: "open" } };
      const text = outcome === "no_effect" ? "Guard" : "New";
      const request = outcome === "refused_to_draft" ? mixedRequest(opened, writer, `${ns}5`)
        : replaceUnit(outcome === "proposal_revised" ? 14 : 0, outcome === "proposal_revised" ? 17 : 5,
          text, writer, `${ns}5`, opened.proposal.revision_id);
      if (outcome === "refused_to_draft") request.author_edit_units[0]!.normalized_primitives = [
        { kind: "replace_structured_selection", replacement: [{ block_kind: "paragraph", text }] }];
      if (outcome === "conflicted") request.expected_proposal_head_revision_ids = [id(`${ns}58`)];
      if (outcome === "reserved_block") request.author_edit_units[0]!.normalized_primitives = [
        { kind: "replace_block_selection", manuscript_block_id: opened.proposal.manuscript_block_id,
          from: 0, to: 5, text }];
      request.local_intent_sequence = String(BigInt(writer.nextSequence) + 1n);
      request.retry_source = { kind: "draft_retry", source_draft_kind: "refused_edit",
        source_draft_id: source.draft_id, source_current_draft_revision_id: source.draft_revision_id,
        source_draft_payload_digest: outcome === "source_conflict" ? "0".repeat(64) : source.payload_digest,
        expected_source_draft_closure: "open", selected_payload_range: { kind: "exact_structured_range",
          coordinate_profile: "storyos.draft-replacement.block-utf16.v1",
          from: { block_index: 0, offset: outcome === "no_effect" ? 0 : 7 },
          to: { block_index: 0, offset: outcome === "no_effect" ? 5 : 10 },
          slice_digest: createHash("sha256").update(JSON.stringify([{ block_kind: "paragraph", text }])).digest("hex") } };
      if (outcome === "split_surrogate") request.retry_source.selected_payload_range.from.offset = 6;
      if (outcome === "changed_content") request.author_edit_units[0]!.normalized_primitives = [
        { kind: "replace_selection", from: 0, to: 5, text: "NewX" }];
      const before = await retainedState(prepared.projectId);
      const retried = await sendMixed(started.baseUrl, prepared, request, id(`${ns}56`)).catch((error: unknown) => {
        if (error instanceof StoryOSProtocolError) throw new Error(`${outcome}: ${error.responseBody}`, { cause: error });
        throw error;
      });
      assert.equal(retried.effect.kind, ["source_conflict", "split_surrogate", "changed_content", "reserved_block"].includes(outcome) ? "conflicted" : outcome);
      const after = await retainedState(prepared.projectId);
      const observed = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        draftId: source.draft_id, fetchImpl: prepared.fetchImpl })).draft;
      if (["no_effect", "conflicted", "source_conflict", "split_surrogate", "changed_content", "reserved_block"].includes(outcome)) {
        assert.deepEqual(retried.source_draft_disposition, unchanged);
        assert.deepEqual(observed, source);
        assert.deepEqual(after, before);
        assert.deepEqual([retried.receipt.draft_artifact_refs, retried.receipt.artifact_lifecycle_event_refs], [[], []]);
      } else {
        const closed = observed.closure_event!;
        assert.deepEqual(retried.source_draft_disposition, { kind: "closed_superseded", source_draft_kind: "refused_edit",
          source_draft_id: source.draft_id, source_draft_revision_id: source.draft_revision_id,
          source_draft_payload_digest: source.payload_digest, prior_closure: "open", resulting_closure: "closed",
          close_reason: "superseded", closure_event_ref: closed.event_id });
        assert.deepEqual({ ...observed, closure: "open", closure_event: undefined }, { ...source, closure_event: undefined });
        if (retried.effect.kind === "refused_to_draft") {
          const provenance = { source_draft_id: source.draft_id, source_draft_revision_id: source.draft_revision_id,
            source_draft_payload_digest: source.payload_digest, closure_event_ref: closed.event_id,
            selected_payload_range: request.retry_source.selected_payload_range };
          assert.equal(retried.effect.refusal_origin, "draft_retry_replacement");
          assert.deepEqual(retried.effect.replacement_provenance, provenance);
          const replacement = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
            draftId: retried.effect.draft_id, fetchImpl: prepared.fetchImpl })).draft;
          assert.deepEqual(replacement.payload.author_edit_units, request.author_edit_units);
          assert.deepEqual(replacement.replacement_provenance, provenance);
          assert.equal(replacement.closure, "open");
          assert.equal(closed.author_action_sequence, null);
          assert.deepEqual(retried.receipt.draft_artifact_refs, [replacement.draft_id, source.draft_id]);
          assert.deepEqual(retried.receipt.artifact_lifecycle_event_refs, [replacement.creation.creation_event_id, closed.event_id]);
          assert.deepEqual(after.author_action_entries, before.author_action_entries);
        } else {
          assert.equal(closed.author_action_sequence, retried.receipt.author_action_sequence);
          assert.deepEqual(retried.receipt.draft_artifact_refs, [source.draft_id]);
          assert.deepEqual(retried.receipt.artifact_lifecycle_event_refs, [closed.event_id]);
          const proposal = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
            proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl });
          assert.equal(proposal.proposal.candidate_text, "narrNewr tone");
          if (retried.effect.kind !== "proposal_revised") throw new Error("expected candidate retry");
          assert.equal(proposal.proposal.revision_id, retried.effect.proposal_revision_id);
        }
      }
      assert.deepEqual(await sendMixed(started.baseUrl, prepared, request, id(`${ns}56`)), retried);
      assert.deepEqual(await retainedState(prepared.projectId), after);
      await assert.rejects(() => sendMixed(started.baseUrl, prepared,
        { ...request, correlation_id: id(`${ns}57`) }, id(`${ns}56`)),
        (error) => requireStoryOSProtocolError(error).status === 409);
      if (retried.effect.kind === "proposal_revised") {
        const undo = { command_schema: "storyos.command.undo-latest-author-action.request.v1",
          undo_latest_author_action_input: { ...BINDING, correlation_id: id(`${ns}61`),
            editor_session_id: writer.session.editor_session.editor_session_id,
            expected_author_undo_frontier_sequence: retried.effect.author_action_sequence,
            expected_authoritative_revision_id: writer.authoritativeRevisionId } };
        const compensate = async () => challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
          "/api/v1/projects/{project_id}/author-actions/undo", undo.command_schema, await digestUndoLatestAuthorAction(undo),
          id(`${ns}62`), (antiForgery) => undoLatestAuthorAction({ baseUrl: started.baseUrl, projectId: prepared.projectId,
            request: undo, idempotencyKey: id(`${ns}62`), antiForgery, fetchImpl: prepared.fetchImpl }));
        const beforeUndo = await retainedState(prepared.projectId);
        for (const invalid of ["'{}'::jsonb", "NEW.result_payload || jsonb_build_object('source_proposal_revision_id', '018f0000-0000-7001-8000-000000000001')",
          "NEW.result_payload || jsonb_build_object('project_activity_position', NULL)",
          "NEW.result_payload || jsonb_build_object('project_activity_position', '01')",
          "NEW.result_payload || jsonb_build_object('project_activity_position', '18446744073709551616')"]) {
          await queryPostgres(`CREATE FUNCTION storyos.damage_retry_compensation() RETURNS trigger LANGUAGE plpgsql AS $fault$
            BEGIN IF NEW.project_id='${prepared.projectId}'::uuid AND NEW.command_kind='undoLatestAuthorAction' THEN
              NEW.result_payload := ${invalid}; END IF; RETURN NEW; END $fault$;
            CREATE TRIGGER damage_retry_compensation BEFORE INSERT ON storyos.domain_receipts
              FOR EACH ROW EXECUTE FUNCTION storyos.damage_retry_compensation();`);
          try { await assert.rejects(() => compensate(), (error) => requireStoryOSProtocolError(error).status === 503); }
          finally { await queryPostgres("DROP TRIGGER damage_retry_compensation ON storyos.domain_receipts; DROP FUNCTION storyos.damage_retry_compensation();"); }
          assert.deepEqual(await retainedState(prepared.projectId), beforeUndo);
          assert.deepEqual((await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
            draftId: source.draft_id, fetchImpl: prepared.fetchImpl })).draft, observed);
        }
        const undone = await compensate();
        assert.equal(undone.effect.kind, "compensated");
        const compensated = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
          proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl });
        assert.equal(compensated.proposal.candidate_text, INLINE_CANDIDATE);
        assert.equal(Reflect.get(undone, "proposal_revision_id"), compensated.proposal.revision_id);
        assert.equal(undone.source_reopen_event?.source_close_event_id, observed.closure_event?.event_id);
        assert.deepEqual(await compensate(), undone);
        assert.deepEqual((await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
          draftId: source.draft_id, fetchImpl: prepared.fetchImpl })).draft,
          { ...source, closure_event: observed.closure_event, reopen_event: undone.source_reopen_event });
        assert.deepEqual(await sendMixed(started.baseUrl, prepared, request, id(`${ns}56`)), retried);
        await stopRealServer(started.server); started = await startRealServer();
        prepared.fetchImpl = browserFetch(started.baseUrl, "session-a");
        assert.deepEqual(await compensate(), undone);
        const resumedProposal = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
          proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl });
        assert.deepEqual({ ...resumedProposal, correlation_id: compensated.correlation_id }, compensated);
        assert.deepEqual((await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
          draftId: source.draft_id, fetchImpl: prepared.fetchImpl })).draft,
          { ...source, closure_event: observed.closure_event, reopen_event: undone.source_reopen_event });
      }
      const recovered = [(await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        draftId: source.draft_id, fetchImpl: prepared.fetchImpl })).draft];
      if (retried.effect.kind === "refused_to_draft") recovered.push((await getRefusedEditDraft({
        baseUrl: started.baseUrl, projectId: prepared.projectId, draftId: retried.effect.draft_id,
        fetchImpl: prepared.fetchImpl })).draft);
      await retainRefusedEditRecoveryExpectation(prepared.projectId, recovered.map((draft) => ({ draft, available: true })));
    }
  } finally { await stopRealServer(started.server); }
});

test("a cross-block Unicode Draft retry rolls back atomically and recovers one exact replacement after restart", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e0c9811"), "Structured Draft Retry", "e0c982");
    const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, prepared.chapterId, "e0c983");
    const original = mixedRequest(opened, writer, "e0c984");
    original.author_edit_units[0]!.normalized_primitives = [{ kind: "replace_structured_selection", replacement: [
      { block_kind: "heading", text: "标题🙂" }, { block_kind: "paragraph", text: "GuardNew" }] }];
    const created = await sendMixed(started.baseUrl, prepared, original, id("e0c9846"));
    if (created.effect.kind !== "refused_to_draft") throw new Error("expected complete structured source");
    const source = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: created.effect.draft_id, fetchImpl: prepared.fetchImpl })).draft;
    const selected = [{ block_kind: "heading", text: "🙂" }, { block_kind: "paragraph", text: "Guard" }];
    const request = mixedRequest(opened, writer, "e0c985");
    request.author_edit_units[0]!.normalized_primitives = [{ kind: "replace_structured_selection",
      replacement: selected as [{ block_kind: "heading"; text: string }, { block_kind: "paragraph"; text: string }] }];
    request.local_intent_sequence = String(BigInt(writer.nextSequence) + 1n);
    request.retry_source = { kind: "draft_retry", source_draft_kind: "refused_edit", source_draft_id: source.draft_id,
      source_current_draft_revision_id: source.draft_revision_id, source_draft_payload_digest: source.payload_digest,
      expected_source_draft_closure: "open", selected_payload_range: { kind: "exact_structured_range",
        coordinate_profile: "storyos.draft-replacement.block-utf16.v1", from: { block_index: 0, offset: 2 },
        to: { block_index: 1, offset: 5 }, slice_digest: createHash("sha256").update(JSON.stringify(selected)).digest("hex") } };
    let retryNonce = "";
    const sendRetry = async () => challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits", request.command_schema,
      await digestApplyAuthorEdit(request), id("e0c9856"), (antiForgery) => {
        retryNonce = antiForgery;
        return applyAuthorEdit({ baseUrl: started.baseUrl, projectId: prepared.projectId, request,
          idempotencyKey: id("e0c9856"), antiForgery, fetchImpl: prepared.fetchImpl });
      });
    const before = await retainedState(prepared.projectId);
    await queryPostgres(`CREATE FUNCTION storyos.fail_narrow_retry() RETURNS trigger LANGUAGE plpgsql AS $fault$
      BEGIN IF NEW.project_id='${prepared.projectId}'::uuid AND NEW.source_draft_disposition IS NOT NULL THEN
        RAISE EXCEPTION 'Controlled narrow retry failure' USING ERRCODE='23514'; END IF; RETURN NEW; END $fault$;
      CREATE TRIGGER fail_narrow_retry BEFORE INSERT ON storyos.domain_receipts
        FOR EACH ROW EXECUTE FUNCTION storyos.fail_narrow_retry();`);
    try { await assert.rejects(() => sendRetry(),
      (error) => requireStoryOSProtocolError(error).status === 503); }
    finally { await queryPostgres("DROP TRIGGER fail_narrow_retry ON storyos.domain_receipts; DROP FUNCTION storyos.fail_narrow_retry();"); }
    assert.deepEqual(await retainedState(prepared.projectId), before);
    assert.deepEqual((await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: source.draft_id, fetchImpl: prepared.fetchImpl })).draft, source);
    const reconciled = await getApplyAuthorEditOutcome({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      idempotencyKey: id("e0c9856"), antiForgery: retryNonce, fetchImpl: prepared.fetchImpl });
    if (reconciled.outcome.outcome_kind !== "committed") throw new Error(`expected original Admission recovery: ${JSON.stringify(reconciled.outcome)}`);
    const retried = reconciled.outcome.response;
    if (retried.effect.kind !== "refused_to_draft") throw new Error("expected exact replacement refusal");
    const replacementId = retried.effect.draft_id;
    const after = await retainedState(prepared.projectId);
    await stopRealServer(started.server);
    started = await startRealServer();
    const resumed = { ...prepared, fetchImpl: browserFetch(started.baseUrl, "session-a") };
    const recovered = await getApplyAuthorEditOutcome({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      idempotencyKey: id("e0c9856"), antiForgery: retryNonce, fetchImpl: resumed.fetchImpl });
    if (recovered.outcome.outcome_kind !== "committed") throw new Error("expected retained retry outcome");
    assert.deepEqual(recovered.outcome.response, retried);
    assert.deepEqual(await sendMixed(started.baseUrl, resumed, request, id("e0c9856")), retried);
    const replacement = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: replacementId, fetchImpl: resumed.fetchImpl })).draft;
    assert.deepEqual(replacement.payload.author_edit_units, request.author_edit_units);
    assert.deepEqual(replacement.replacement_provenance, retried.effect.replacement_provenance);
    assert.equal(replacement.closure, "open");
    assert.deepEqual(await retainedState(prepared.projectId), after);
    const archive = async (suffix: string) => {
      const request = { command_schema: "storyos.command.export-project-archive.request.v1",
        export_project_archive_input: { ...BINDING, correlation_id: id(`${suffix}1`), archive_profile: "storyos.project-export.v1",
          archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" } };
      const admitted = await challenged(started.baseUrl, resumed.fetchImpl, prepared.projectId, "POST",
        "/api/v1/projects/{project_id}/exports", request.command_schema, await digestExportProjectArchive(request), id(`${suffix}2`),
        (antiForgery) => exportProjectArchive({ baseUrl: started.baseUrl, projectId: prepared.projectId,
          fetchImpl: resumed.fetchImpl, request, idempotencyKey: id(`${suffix}2`), antiForgery }));
      if (admitted.effect.kind !== "admitted") throw new Error("expected retry Archive admission");
      await settleOnce();
      const ready = await getExportOperation({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        exportId: admitted.effect.export_id, fetchImpl: resumed.fetchImpl });
      assert.equal(ready.status, "ready", JSON.stringify(ready));
      if (ready.status !== "ready") throw new Error("expected complete retry Archive");
      return ready;
    };
    const download = (exportId: string) => resumed.fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${exportId}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
    const prior = await archive("e0c986");
    assert.equal((await download(prior.export_id)).status, 200);
    await queryPostgres(`UPDATE storyos.draft_artifacts SET retention_state='tombstoned'
      WHERE owner_user_id='${USER_A}'::uuid AND project_id='${prepared.projectId}'::uuid AND draft_id='${replacementId}'::uuid;`);
    await assert.rejects(() => getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: replacementId, fetchImpl: resumed.fetchImpl }), (error) => requireStoryOSProtocolError(error).status === 404);
    const closedSource = (await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: source.draft_id, fetchImpl: resumed.fetchImpl })).draft;
    assert.deepEqual(closedSource.payload, source.payload);
    assert.equal((await download(prior.export_id)).status, 422);
    const priorRead = await getExportOperation({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      exportId: prior.export_id, fetchImpl: resumed.fetchImpl });
    assert.deepEqual({ ...priorRead, correlation_id: prior.correlation_id, query_id: prior.query_id }, prior);
    const current = await archive("e0c987");
    const exported = await download(current.export_id); assert.equal(exported.status, 200);
    const bytes = new Uint8Array(await exported.arrayBuffer()), files = zipStoreFiles(bytes);
    const rows = (table: string) => JSON.parse(new TextDecoder().decode(files.get(`canonical/${table}.json`))) as Record<string, unknown>[];
    const revisionGap = { kind: "refused_edit_revision_payload", reason: "withheld_due_to_tombstone",
      entry_path: "canonical/draft_artifact_revisions.json", record_id: replacement.draft_revision_id,
      payload_field: "payload", draft_id: replacementId, retention_state: "tombstoned",
      payload_digest: replacement.payload_digest, payload_digest_profile: "storyos.refused-edit-payload.jcs.v1" };
    const admissionGap = { kind: "refused_edit_admission_payload", reason: "withheld_due_to_tombstone",
      entry_path: "canonical/author_command_admissions.json", record_id: retried.author_command_admission_id,
      payload_field: "command_payload", draft_id: replacementId, retention_state: "tombstoned", command_id: retried.command_id,
      canonical_command_digest: `sha256:${retried.receipt.command_digest.profile}:${retried.receipt.command_digest.value_hex_lowercase}` };
    assert.deepEqual(rows("draft_artifact_revisions"), after.draft_artifact_revisions.map((row: Record<string, unknown>) => {
      if (row.draft_id !== replacementId) return row;
      const { payload: _withheld, ...metadata } = row; return { ...metadata, payload_availability: revisionGap };
    }));
    const admissions = JSON.parse(await queryPostgres(`SELECT jsonb_agg(to_jsonb(record) ORDER BY to_jsonb(record)::text)
      FROM storyos.author_command_admissions AS record WHERE owner_user_id='${USER_A}'::uuid
        AND project_id='${prepared.projectId}'::uuid AND command_kind='applyAuthorEdit'`));
    assert.deepEqual(rows("author_command_admissions").filter((row) => row.command_kind === "applyAuthorEdit"),
      admissions.map((row: Record<string, unknown>) => {
        if (row.author_command_admission_id !== retried.author_command_admission_id) return row;
        const { command_payload: _withheld, ...metadata } = row;
        return { ...metadata, payload_availability: admissionGap, draft_retry_source: request.retry_source };
      }));
    for (const table of ["draft_lifecycle_events", "draft_close_events", "domain_receipts", "author_action_entries"]) {
      const expected = JSON.parse(await queryPostgres(`SELECT coalesce(jsonb_agg(to_jsonb(record) ORDER BY to_jsonb(record)::text), '[]')
        FROM storyos.${table} AS record WHERE owner_user_id='${USER_A}'::uuid AND project_id='${prepared.projectId}'::uuid`));
      const key = table === "domain_receipts" ? "receipt_id" : table === "draft_lifecycle_events"
        ? "creation_event_id" : table === "author_action_entries" ? "author_action_sequence" : "event_id";
      const compare = (a: Record<string, unknown>, b: Record<string, unknown>) => String(a[key]).localeCompare(String(b[key]));
      assert.deepEqual(rows(table).sort(compare), expected.filter((row: Record<string, unknown>) =>
        table !== "domain_receipts" || row.idempotency_key !== id("e0c9872")).sort(compare));
    }
    const pin = rows("pinned_export_sources").find((row) => row.export_id === prior.export_id)!;
    assert.equal(Object.hasOwn(pin, "facts"), false);
    assert.deepEqual(pin.payload_availability, { kind: "refused_edit_pinned_export_source_facts", reason: "withheld_due_to_tombstone",
      entry_path: "canonical/pinned_export_sources.json", record_id: prior.export_id, payload_field: "facts",
      restricted_draft_ids: [replacementId], facts_sha256: pin.facts_sha256 });
    for (const content of files.values()) assert.ok(!new TextDecoder().decode(content).includes(retryNonce));
    for (const [index, mutation] of ["entry - 'draft_retry_source'",
      "jsonb_set(entry, '{draft_retry_source,source_draft_payload_digest}', to_jsonb(repeat('0',64)))"].entries()) {
      await queryPostgres(`CREATE FUNCTION storyos.damage_retry_archive() RETURNS trigger LANGUAGE plpgsql AS $fault$
        DECLARE damaged jsonb;
        BEGIN
          IF NEW.owner_user_id='${USER_A}'::uuid AND NEW.project_id='${prepared.projectId}'::uuid
            AND NEW.path='canonical/author_command_admissions.json' THEN
            SELECT jsonb_agg(CASE WHEN entry->>'author_command_admission_id'='${retried.author_command_admission_id}'
              THEN ${mutation} ELSE entry END ORDER BY position) INTO damaged
              FROM jsonb_array_elements(convert_from(NEW.payload,'UTF8')::jsonb) WITH ORDINALITY AS rows(entry,position);
            NEW.payload:=convert_to(damaged::text,'UTF8'); NEW.byte_length:=octet_length(NEW.payload);
            NEW.digest:='sha256:'||encode(sha256(NEW.payload),'hex');
          END IF; RETURN NEW;
        END $fault$;
        CREATE TRIGGER damage_retry_archive BEFORE INSERT ON storyos.project_export_entries
          FOR EACH ROW EXECUTE FUNCTION storyos.damage_retry_archive();`);
      try {
        const damaged = await archive(`e0c988${index}`), refused = await download(damaged.export_id);
        assert.equal(refused.status, 422);
        assert.equal(refused.headers.get("content-type"), "application/json");
        assert.deepEqual(await refused.json(), { schema_id: "storyos.problem.v1", code: "invalid_provenance",
          message: "The Project Export Archive did not complete." });
      } finally {
        await queryPostgres("DROP TRIGGER damage_retry_archive ON storyos.project_export_entries; DROP FUNCTION storyos.damage_retry_archive();");
      }
    }
    await retainRefusedEditRecoveryExpectation(prepared.projectId,
      [{ draft: closedSource, available: true }, { draft: { ...replacement, retention_state: "tombstoned" }, available: false }],
      [{ exportId: prior.export_id, root: prior.immutable_root, status: 422 },
        { exportId: current.export_id, root: current.immutable_root, status: 200,
          bytesSha256: createHash("sha256").update(bytes).digest("hex") }]);
  } finally { await stopRealServer(started.server); }
});

test("a protected mixed replacement retains complete content after response loss, replay, and restart", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e09111"), "Refused Edit Novel", "e092");
    const { opened, writer } = await openInline(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, "e093",
    );
    const request = replaceUnit(8, 26, "New passage", writer, "e094", opened.proposal.revision_id);
    const sources: SelectedEditSource[] = [
      { owner: { kind: "manuscript", manuscript_block_id: opened.proposal.manuscript_block_id },
        coordinate_profile: "prosemirror-token-utf16.v1", from: 8, to: 10,
        block_kind: "paragraph", source_text: "Guard the narrator voice in this passage." },
      { owner: { kind: "proposal", proposal_id: opened.proposal.proposal_id,
        operation_id: opened.proposal.operation_id, revision_id: opened.proposal.revision_id,
        manuscript_block_id: opened.proposal.manuscript_block_id },
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 13,
        block_kind: "paragraph", source_text: "narrator tone" },
      { owner: { kind: "manuscript", manuscript_block_id: opened.proposal.manuscript_block_id },
        coordinate_profile: "prosemirror-token-utf16.v1", from: 24, to: 26,
        block_kind: "paragraph", source_text: "Guard the narrator voice in this passage." },
    ];
    request.author_edit_units = [{
      normalized_primitives: [{ kind: "replace_structured_selection",
        replacement: [{ block_kind: "paragraph", text: "New passage" }] }],
      selection_snapshot: { coordinate_profile: "storyos.editor.ordered-source.v1", from: 8, to: 26,
        ordered_selection: { sources, anchor: { source_index: 0, source_offset: 8 },
          head: { source_index: 2, source_offset: 26 } } },
    }];
    const before = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    const key = id("e0946");
    const digest = await digestApplyAuthorEdit(request);
    let originalNonce = "";
    let loseResponse = true;
    const lossyFetch: typeof fetch = async (input, init) => {
      const response = await prepared.fetchImpl(input, init);
      if (loseResponse && init?.method === "POST" && String(input).endsWith("/manuscript/author-edits")) {
        assert.equal(response.status, 200, await response.clone().text());
        loseResponse = false;
        await stopRealServer(started.server);
        throw new Error("Controlled response loss after durable HTTP settlement");
      }
      return response;
    };
    await assert.rejects(() => challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits", request.command_schema,
      digest, key, (antiForgery) => {
        originalNonce = antiForgery;
        return applyAuthorEdit({ baseUrl: started.baseUrl, projectId: prepared.projectId,
          fetchImpl: lossyFetch, idempotencyKey: key, antiForgery, request });
      }), /Controlled response loss/);
    const originalOrigin = new URL(started.baseUrl);
    started = await startRealServer(`${originalOrigin.hostname}:${originalOrigin.port}`);
    prepared.fetchImpl = browserFetch(started.baseUrl, "session-a");
    const recovered = await getApplyAuthorEditOutcome({ baseUrl: started.baseUrl,
      projectId: prepared.projectId, idempotencyKey: key, antiForgery: originalNonce,
      fetchImpl: prepared.fetchImpl });
    if (recovered.outcome.outcome_kind !== "committed") throw new Error("expected committed outcome");
    const result = recovered.outcome.response;
    if (result.effect.kind !== "refused_to_draft") throw new Error("expected preserved Draft");
    assert.deepEqual(result.effect, { kind: "refused_to_draft", refusal_origin: "fresh_editor_intent",
      draft_id: result.effect.draft_id, draft_revision_id: result.effect.draft_revision_id,
      creation_event_id: result.effect.creation_event_id });
    for (const value of [result.command_id, result.author_command_admission_id,
      result.receipt.receipt_id, result.effect.draft_id, result.effect.draft_revision_id,
      result.effect.creation_event_id]) assert.match(value, UUID_V7);
    assert.deepEqual(result.receipt, {
      receipt_id: result.receipt.receipt_id, project_scope: { owner_user_id: USER_A, project_id: prepared.projectId },
      command_kind: "applyAuthorEdit", command_digest: digest, idempotency_key: key,
      producer_cause: "author_command_admission", author_command_admission_id: result.author_command_admission_id,
      expected_heads: [writer.authoritativeRevisionId], prior_heads: [writer.authoritativeRevisionId],
      resulting_heads: [writer.authoritativeRevisionId], authoritative_revision_ids: [], proposal_revision_ids: [],
      authoritative_commit_ids: [], author_action_sequence: null, draft_artifact_refs: [result.effect.draft_id],
      artifact_lifecycle_event_refs: [result.effect.creation_event_id], condition_refs: [],
      result: "refused_to_draft", created_at: result.receipt.created_at,
    });
    const expectedPayload = {
      schema_revision: "storyos.refused-edit-payload.v1", chapter_id: prepared.chapterId,
      expected_authoritative_revision_id: writer.authoritativeRevisionId,
      expected_proposal_head_revision_ids: [opened.proposal.revision_id],
      target_refs: [`manuscript:${prepared.chapterId}`], author_edit_units: request.author_edit_units,
      undo_group_id: id("e0944"), completed_intent_record_id: id("e0945"), local_intent_sequence: "2",
    };
    const canonicalPayload = JSON.stringify(expectedPayload, (_key, value: unknown) =>
      value !== null && typeof value === "object" && !Array.isArray(value)
        ? Object.fromEntries(Object.entries(value).sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0))
        : value);
    const queried = await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: result.effect.draft_id, fetchImpl: prepared.fetchImpl });
    assert.ok(Number.isFinite(Date.parse(queried.draft.creation.created_at)));
    assert.ok(Date.parse(queried.draft.creation.created_at) <= Date.parse(result.receipt.created_at));
    assert.deepEqual(queried.draft, {
      draft_id: result.effect.draft_id, draft_revision_id: result.effect.draft_revision_id,
      kind: "refused_edit", closure: "open", retention_state: "retained", payload: expectedPayload,
      payload_digest: createHash("sha256").update(canonicalPayload).digest("hex"),
      payload_digest_profile: "storyos.refused-edit-payload.jcs.v1", creation: {
        schema_id: "storyos.event.refused-edit-draft-created.v1", event_kind: "refused_edit_draft_created",
        project_scope: { owner_user_id: USER_A, project_id: prepared.projectId },
        creator: { kind: "core_transition", receipt_id: result.receipt.receipt_id }, creation_event_id: result.effect.creation_event_id,
        draft_id: result.effect.draft_id, draft_revision_id: result.effect.draft_revision_id,
        created_at: queried.draft.creation.created_at, source: { command_id: result.command_id,
          author_command_admission_id: result.author_command_admission_id, receipt_id: result.receipt.receipt_id,
          idempotency_key: key, command_digest: digest },
      },
    });
    const replay = () => applyAuthorEdit({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl, idempotencyKey: key, antiForgery: originalNonce, request });
    assert.deepEqual(await replay(), result);
    assert.deepEqual(await Promise.all([replay(), replay()]), [result, result]);
    assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl })).chapter, before.chapter);
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl })).proposal, opened.proposal);
    await assert.rejects(() => getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: queried.draft.draft_id, fetchImpl: browserFetch(started.baseUrl, "session-b") }), (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes("New passage");
      });
    const archiveRequest = { command_schema: "storyos.command.export-project-archive.request.v1" as const,
      export_project_archive_input: { ...BINDING, correlation_id: id("e0951"),
        archive_profile: "storyos.project-export.v1", archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" } };
    const archive = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/exports", archiveRequest.command_schema,
      await digestExportProjectArchive(archiveRequest), id("e0952"),
      (antiForgery) => exportProjectArchive({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl, idempotencyKey: id("e0952"), antiForgery, request: archiveRequest }));
    if (archive.effect.kind !== "admitted") throw new Error("expected admitted archive");
    await settleOnce();
    assert.equal((await getExportOperation({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      exportId: archive.effect.export_id, fetchImpl: prepared.fetchImpl })).status, "ready");
    const download = await prepared.fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${archive.effect.export_id}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
    assert.equal(download.status, 200);
    const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
    const drafts = JSON.parse(new TextDecoder().decode(files.get("canonical/draft_artifacts.json")));
    const revisions = JSON.parse(new TextDecoder().decode(files.get("canonical/draft_artifact_revisions.json")));
    const events = JSON.parse(new TextDecoder().decode(files.get("canonical/draft_lifecycle_events.json")));
    const rowScope = { owner_user_id: USER_A, project_id: prepared.projectId };
    assert.deepEqual(drafts, [{ ...rowScope, draft_id: queried.draft.draft_id, draft_kind: "refused_edit",
      current_revision_id: queried.draft.draft_revision_id, closure: "open", retention_state: "retained", close_event_id: null, reopen_event_id: null }]);
    const revisionCreatedAt = revisions[0].created_at;
    assert.equal(new Date(revisionCreatedAt).toISOString(), queried.draft.creation.created_at);
    assert.deepEqual(revisions, [{ ...rowScope, draft_id: queried.draft.draft_id,
      revision_id: queried.draft.draft_revision_id, payload: expectedPayload,
      payload_digest: queried.draft.payload_digest, payload_digest_profile: "storyos.refused-edit-payload.jcs.v1",
      created_at: revisionCreatedAt }]);
    assert.deepEqual(events, [{ ...rowScope, creation_event_id: queried.draft.creation.creation_event_id,
      event_kind: "refused_edit_draft_created", draft_id: queried.draft.draft_id,
      revision_id: queried.draft.draft_revision_id, receipt_id: result.receipt.receipt_id,
      author_command_admission_id: result.author_command_admission_id, command_id: result.command_id,
      receipt_result_kind: "refused_to_draft", created_at: revisionCreatedAt }]);
    for (const content of files.values()) assert.ok(!new TextDecoder().decode(content).includes(originalNonce));
    const readyExport = await getExportOperation({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      exportId: archive.effect.export_id, fetchImpl: prepared.fetchImpl });
    if (readyExport.status !== "ready") throw new Error("expected ready recovery archive");
    const recoveryDownload = await prepared.fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${readyExport.export_id}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
    assert.equal(recoveryDownload.status, 200);
    await retainRefusedEditRecoveryExpectation(prepared.projectId, [{ draft: queried.draft, available: true }],
      [{ exportId: readyExport.export_id, root: readyExport.immutable_root, status: 200,
        bytesSha256: createHash("sha256").update(new Uint8Array(await recoveryDownload.arrayBuffer())).digest("hex") }]);
  } finally { await stopRealServer(started.server); }
});

test("inline Proposal uses exact Anchors, keeps source and candidate distinct, and Accepts a splice", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e08111"), "Inline Proposal Novel", "e082");
    const { opened, writer } = await openInline(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, "e083",
    );
    const before = await getChapter({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(before.chapter.current_revision.body, PROSE);
    const reloaded = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(reloaded.proposal, opened.proposal);
    const interior = replaceUnit(14, 17, "xx", writer, "e084", opened.proposal.revision_id);
    const edited = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits",
      interior.command_schema, await digestApplyAuthorEdit(interior), id("e0846"),
      (antiForgery) => applyAuthorEdit({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl, idempotencyKey: id("e0846"), antiForgery,
        request: interior,
      }),
    );
    assert.equal(edited.effect.kind, "proposal_revised");
    const revised = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(revised.proposal.candidate_text, INTERIOR_CANDIDATE);
    assert.equal(revised.proposal.validation, "valid");
    if (opened.proposal.validation_receipt.kind !== "present"
      || revised.proposal.validation_receipt.kind !== "present") {
      throw new Error("expected receipts");
    }
    assert.notEqual(
      revised.proposal.validation_receipt.validation_receipt_id,
      opened.proposal.validation_receipt.validation_receipt_id,
    );
    const stillSource = await getChapter({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(stillSource.chapter.current_revision.body, PROSE);
    assert.notEqual(stillSource.chapter.current_revision.body, revised.proposal.candidate_text);
    const acceptRequest: AcceptProposalRequest = {
      command_schema: "storyos.command.accept-proposal.request.v1",
      accept_proposal_input: {
        proposal_revision_id: revised.proposal.revision_id,
        validation_receipt_id: revised.proposal.validation_receipt.validation_receipt_id,
        selected_operation_ids: [revised.proposal.operation_id],
        expected_authoritative_revision_id: stillSource.chapter.current_revision.revision_id,
        editor_session_id: writer.session.editor_session.editor_session_id,
        ...BINDING, correlation_id: id("e0851"),
      },
    };
    const accepted = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      acceptRequest.command_schema, await digestAcceptProposal(acceptRequest), id("e0852"),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: revised.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("e0852"), antiForgery, request: acceptRequest,
      }),
    );
    assert.equal(accepted.effect.kind, "applied");
    const after = await getChapter({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(after.chapter.current_revision.body, ACCEPTED_BODY);
    await assert.rejects(
      () => getProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id,
        fetchImpl: browserFetch(started.baseUrl, "session-b"),
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
  } finally {
    await stopRealServer(started.server);
  }
});

test.each(["paragraph", "heading"] as const)("inline Acceptance preserves complete two-Block state with a %s sibling after response loss and restart", async (siblingKind) => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = siblingKind === "paragraph" ? "f865a" : "f865b";
    const prepared = await prepare(started.baseUrl, id(`${ns}11`), "Canonical Inline Acceptance", `${ns}2`);
    const siblingText = "Second paragraph. 😀";
    const writer = await writePassage(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, `${ns}3`, PROSE + siblingText);
    const options = () => ({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    const initial = await getChapter(options());
    const firstBlock = initial.chapter.current_revision.blocks[0]!;
    const siblingId = id(`${ns}41`);
    const split = replaceUnit(0, 0, "", writer, `${ns}4`, id(`${ns}ff1`));
    split.expected_proposal_head_revision_ids = [];
    split.observed_ownership_partition = "authoritative";
    split.author_edit_units = [{ normalized_primitives: [
      { kind: "split_block", manuscript_block_id: firstBlock.manuscript_block_id,
        offset: PROSE.length, new_manuscript_block_id: siblingId },
      { kind: "retype_block", manuscript_block_id: siblingId, block_kind: siblingKind },
    ], selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1",
      from: PROSE.length, to: PROSE.length } }];
    const divided = await sendMixed(started.baseUrl, prepared, split, id(`${ns}42`));
    if (divided.effect.kind !== "authoritative_applied") throw new Error("expected split");
    const before = await getChapter(options());
    assert.deepEqual(before.chapter.current_revision.blocks, [
      { ...firstBlock, text: PROSE },
      { manuscript_block_id: siblingId, block_kind: siblingKind, text: siblingText },
    ]);
    const run = await admitPhrase(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, prepared.chapterId, id(`${ns}51`));
    if (run.decision.kind !== "prose_change" || run.decision.opened_proposal.kind !== "present")
      throw new Error("expected inline Proposal");
    const proposalId = run.decision.opened_proposal.proposal_id;
    const opened = await getProposal({ ...options(), proposalId });
    assert.equal(opened.proposal.manuscript_block_id, firstBlock.manuscript_block_id);
    assert.equal(opened.proposal.validation, "valid");
    if (opened.proposal.validation_receipt.kind !== "present") throw new Error("expected receipt");
    const request: AcceptProposalRequest = {
      command_schema: "storyos.command.accept-proposal.request.v1",
      accept_proposal_input: { ...BINDING, correlation_id: id(`${ns}61`),
        proposal_revision_id: opened.proposal.revision_id,
        validation_receipt_id: opened.proposal.validation_receipt.validation_receipt_id,
        selected_operation_ids: [opened.proposal.operation_id],
        expected_authoritative_revision_id: before.chapter.current_revision.revision_id,
        editor_session_id: writer.session.editor_session.editor_session_id },
    };
    const digest = await digestAcceptProposal(request);
    let originalNonce = "";
    let original: Awaited<ReturnType<typeof acceptProposal>> | undefined;
    const lossyFetch: typeof fetch = async (input, init) => {
      const response = await prepared.fetchImpl(input, init);
      if (init?.method === "POST" && String(input).endsWith("/acceptances")) {
        assert.equal(response.status, 200, await response.clone().text());
        original = await response.clone().json();
        await stopRealServer(started.server);
        throw new Error("Controlled Acceptance response loss");
      }
      return response;
    };
    await assert.rejects(() => challenged(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, "POST", "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      request.command_schema, digest, id(`${ns}62`), (antiForgery) => {
        originalNonce = antiForgery;
        return acceptProposal({ ...options(), proposalId, fetchImpl: lossyFetch,
          idempotencyKey: id(`${ns}62`), antiForgery, request });
      }), /Controlled Acceptance response loss/);
    assert.equal(original?.effect.kind, "applied", "valid canonical two-Block inline Acceptance must apply");
    if (original?.effect.kind !== "applied") throw new Error("expected applied");
    const origin = new URL(started.baseUrl);
    started = await startRealServer(`${origin.hostname}:${origin.port}`);
    prepared.fetchImpl = browserFetch(started.baseUrl, "session-a");
    const replay = () => acceptProposal({ ...options(), proposalId,
      idempotencyKey: id(`${ns}62`), antiForgery: originalNonce, request });
    assert.deepEqual(await replay(), original);
    const after = await getChapter(options());
    assert.deepEqual(after.chapter, { ...before.chapter, current_revision: {
      ...original.effect.authoritative_revision,
      body: "Guard the narrator tone in this passage.\nSecond paragraph. 😀",
      blocks: [{ ...firstBlock, text: "Guard the narrator tone in this passage." },
        { manuscript_block_id: siblingId, block_kind: siblingKind, text: siblingText }],
    } });
    assert.deepEqual(await Promise.all([replay(), replay()]), [original, original]);
    assert.deepEqual((await getChapter(options())).chapter, after.chapter);
    const settled = await getProposal({ ...options(), proposalId });
    assert.equal(settled.proposal.operation_resolution, "applied");
    assert.equal(settled.proposal.reservation_state, "resolved");
    assert.equal(settled.proposal.revision_id, opened.proposal.revision_id);
    await assert.rejects(() => getProposal({ ...options(), proposalId,
      fetchImpl: browserFetch(started.baseUrl, "session-b") }),
      (error) => requireStoryOSProtocolError(error).status === 404);
  } finally {
    if (started.server.exitCode === null && started.server.signalCode === null)
      await stopRealServer(started.server);
  }
});

test("edge input stays authoritative, reserved-block change conflicts, and Reject leaves prose", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e08611"), "Inline Edge Novel", "e087");
    const { opened, writer } = await openInline(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, "e088",
    );
    const edge = replaceUnit(0, 5, "Keep", writer, "e089", opened.proposal.revision_id);
    const applied = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits",
      edge.command_schema, await digestApplyAuthorEdit(edge), id("e0896"),
      (antiForgery) => applyAuthorEdit({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl, idempotencyKey: id("e0896"), antiForgery,
        request: edge,
      }),
    );
    assert.equal(applied.effect.kind, "authoritative_applied");
    const chapter = await getChapter({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(chapter.chapter.current_revision.body, EDGE_BODY);
    const conflicted = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(conflicted.proposal.validation, "conflicted");
    assert.equal(conflicted.proposal.candidate_text, INLINE_CANDIDATE);
    const second = await prepare(started.baseUrl, id("e08a11"), "Inline Reject Novel", "e08b");
    const openedSecond = await openInline(
      started.baseUrl, second.fetchImpl, second.projectId, second.chapterId, "e08c",
    );
    const rejectBefore = await getChapter({
      baseUrl: started.baseUrl, projectId: second.projectId,
      chapterId: second.chapterId, fetchImpl: second.fetchImpl,
    });
    const rejectRequest: RejectProposalOperationsRequest = {
      command_schema: "storyos.command.reject-proposal-operations.request.v1",
      reject_proposal_operations_input: {
        proposal_revision_id: openedSecond.opened.proposal.revision_id,
        selected_pending_operation_ids: [openedSecond.opened.proposal.operation_id],
        expected_target_revisions: [rejectBefore.chapter.current_revision.revision_id],
        rejection_reason: { kind: "author_declined", note: { kind: "omitted" } },
        editor_session_id: openedSecond.writer.session.editor_session.editor_session_id,
        ...BINDING, correlation_id: id("e08d3"),
      },
    };
    const rejected = await challenged(
      started.baseUrl, second.fetchImpl, second.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
      rejectRequest.command_schema, await digestRejectProposalOperations(rejectRequest),
      id("e08d4"),
      (antiForgery) => rejectProposalOperations({
        baseUrl: started.baseUrl, projectId: second.projectId,
        proposalId: openedSecond.opened.proposal.proposal_id, fetchImpl: second.fetchImpl,
        idempotencyKey: id("e08d4"), antiForgery, request: rejectRequest,
      }),
    );
    assert.equal(rejected.effect.kind, "resolved");
    const rejectAfter = await getChapter({
      baseUrl: started.baseUrl, projectId: second.projectId,
      chapterId: second.chapterId, fetchImpl: second.fetchImpl,
    });
    assert.deepEqual(rejectAfter.chapter, rejectBefore.chapter);
    const archiveRequest = {
      command_schema: "storyos.command.export-project-archive.request.v1" as const,
      export_project_archive_input: {
        ...BINDING, correlation_id: id("e08e1"),
        archive_profile: "storyos.project-export.v1",
        archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
      },
    };
    const archive = await challenged(
      started.baseUrl, second.fetchImpl, second.projectId, "POST",
      "/api/v1/projects/{project_id}/exports",
      archiveRequest.command_schema, await digestExportProjectArchive(archiveRequest),
      id("e08e2"),
      (antiForgery) => exportProjectArchive({
        baseUrl: started.baseUrl, projectId: second.projectId, fetchImpl: second.fetchImpl,
        idempotencyKey: id("e08e2"), antiForgery, request: archiveRequest,
      }),
    );
    if (archive.effect.kind !== "admitted") throw new Error("expected admitted archive");
    await settleOnce();
    assert.equal((await getExportOperation({
      baseUrl: started.baseUrl, projectId: second.projectId,
      exportId: archive.effect.export_id, fetchImpl: second.fetchImpl,
    })).status, "ready");
    const download = await second.fetchImpl(
      `${started.baseUrl}/api/v1/projects/${second.projectId}/exports/${archive.effect.export_id}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } },
    );
    assert.equal(download.status, 200);
    const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
    const anchors = JSON.parse(new TextDecoder().decode(files.get("canonical/proposal_anchors.json")));
    assert.equal(anchors.length, 1);
    assert.equal(anchors[0].proposal_id, openedSecond.opened.proposal.proposal_id);
    assert.equal(anchors[0].owner_user_id, USER_A);
    assert.match(anchors[0].proposal_id, UUID_V7);
    assert.equal(anchors[0].range_from, 10);
    assert.equal(anchors[0].range_to, 24);
    assert.equal(
      anchors[0].base_slice_digest,
      sliceDigest(openedSecond.opened.proposal.manuscript_block_id),
    );
  } finally {
    await stopRealServer(started.server);
  }
});


test("changed Heads, source identities, order, text, and ranges create no Draft or authority change", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e0a111"), "Invalid Mixed Proof", "e0a2");
    const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, prepared.chapterId, "e0a3");
    const before = await retainedState(prepared.projectId);
    const mutations: ((request: ApplyAuthorEditRequest) => void)[] = [
      (request) => { request.expected_authoritative_revision_id = id("e0aff1"); },
      (request) => { request.expected_proposal_head_revision_ids = []; },
      (request) => { request.author_edit_units[0]!.selection_snapshot.ordered_selection!.sources[1]!.source_text = "altered"; },
      (request) => { request.author_edit_units[0]!.selection_snapshot.ordered_selection!.sources.reverse(); },
      (request) => { request.author_edit_units[0]!.selection_snapshot.ordered_selection!.sources.splice(1, 1); },
      (request) => { const owner = request.author_edit_units[0]!.selection_snapshot.ordered_selection!.sources[1]!.owner;
        if (owner.kind === "proposal") owner.revision_id = id("e0aff2"); },
      (request) => { request.author_edit_units[0]!.selection_snapshot.ordered_selection!.sources[1]!.to += 1; },
    ];
    for (const [index, mutate] of mutations.entries()) {
      const request = mixedRequest(opened, writer, `e0a4${index}`);
      request.local_intent_sequence = String(index + 2);
      mutate(request);
      if (index <= 1) {
        await assert.rejects(() => sendMixed(started.baseUrl, prepared, request, id(`e0a5${index}`)),
          (error) => requireStoryOSProtocolError(error).status === (index === 0 ? 409 : 422));
      } else {
        const result = await sendMixed(started.baseUrl, prepared, request, id(`e0a5${index}`));
        assert.equal(result.effect.kind, index === 3 ? "refused" : "conflicted");
      }
      assert.deepEqual(await retainedState(prepared.projectId), before);
    }
    const staleWriter = mixedRequest(opened, writer, "e0a60");
    staleWriter.writer_generation = "2";
    staleWriter.local_intent_sequence = "9";
    await assert.rejects(() => sendMixed(started.baseUrl, prepared, staleWriter, id("e0a61")),
      (error) => requireStoryOSProtocolError(error).status === 412);
    const known = mixedRequest(opened, writer, "e0a70");
    known.local_intent_sequence = "10";
    const untrusted = { ...known, author_edit_units: known.author_edit_units.map((unit) => ({ ...unit,
      normalized_primitives: unit.normalized_primitives.map((primitive) => ({ ...primitive,
        raw_steps: [{ arbitrary: "This is not typed domain content" }] })) })) };
    const extraFieldFetch: typeof fetch = (input, init) => prepared.fetchImpl(input,
      init?.method === "POST" && String(input).endsWith("/manuscript/author-edits")
        ? { ...init, body: JSON.stringify(untrusted) } : init);
    const knownDigest = await digestApplyAuthorEdit(known);
    await assert.rejects(() => challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits", known.command_schema,
      knownDigest, id("e0a71"), (antiForgery) => applyAuthorEdit({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: extraFieldFetch,
        request: known, antiForgery, idempotencyKey: id("e0a71") })),
      (error) => requireStoryOSProtocolError(error).status === 422);
    assert.equal(await queryPostgres(`SELECT count(*) FROM storyos.author_command_admissions
      WHERE project_id='${prepared.projectId}'::uuid AND idempotency_key='${id("e0a71")}'::uuid`), "0");
    assert.deepEqual(await retainedState(prepared.projectId), before);
  } finally { await stopRealServer(started.server); }
});


test("a pre-commit database fault rolls back all Draft records and GET settles only the retained Admission", async () => {
  const started = await startRealServer();
  const functionName = "refused_edit_test_precommit_fault";
  let faultInstalled = false;
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e0b111"), "Refused Draft Atomic Fault", "e0b2");
    const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, prepared.chapterId, "e0b3");
    const before = await retainedState(prepared.projectId);
    await queryPostgres(`CREATE FUNCTION storyos.${functionName}() RETURNS trigger LANGUAGE plpgsql AS $fault$
      BEGIN IF NEW.project_id = '${prepared.projectId}'::uuid THEN
        RAISE EXCEPTION 'Controlled failure before Refused Draft commit'; END IF; RETURN NULL; END $fault$;
      CREATE CONSTRAINT TRIGGER ${functionName} AFTER INSERT ON storyos.draft_lifecycle_events
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.${functionName}();`);
    faultInstalled = true;
    const request = mixedRequest(opened, writer, "e0b4");
    const key = id("e0b46");
    let nonce = "";
    const digest = await digestApplyAuthorEdit(request);
    await assert.rejects(() => challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits", request.command_schema,
      digest, key, (antiForgery) => { nonce = antiForgery; return applyAuthorEdit({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
        request, idempotencyKey: key, antiForgery }); }),
      (error) => requireStoryOSProtocolError(error).status === 503);
    assert.deepEqual(await retainedState(prepared.projectId), before);
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.domain_receipts
      WHERE project_id = '${prepared.projectId}'::uuid AND idempotency_key = '${key}'::uuid`), "0");
    await queryPostgres(`DROP TRIGGER ${functionName} ON storyos.draft_lifecycle_events;
      DROP FUNCTION storyos.${functionName}();`);
    faultInstalled = false;
    const recovered = await getApplyAuthorEditOutcome({ baseUrl: started.baseUrl,
      projectId: prepared.projectId, idempotencyKey: key, antiForgery: nonce, fetchImpl: prepared.fetchImpl });
    if (recovered.outcome.outcome_kind !== "committed") throw new Error("expected committed outcome");
    const result = recovered.outcome.response;
    if (result.effect.kind !== "refused_to_draft")
      throw new Error("expected explicit GET reconciliation to settle the original retained intent");
    const draft = await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: result.effect.draft_id, fetchImpl: prepared.fetchImpl });
    assert.deepEqual(draft.draft.payload.author_edit_units, request.author_edit_units);
    assert.equal(draft.draft.creation.source.author_command_admission_id, result.author_command_admission_id);
    const after = await retainedState(prepared.projectId);
    assert.deepEqual({ ...after, draft_artifacts: [], draft_artifact_revisions: [], draft_lifecycle_events: [] }, before);
    assert.equal(after.draft_artifacts.length, 1);
    assert.equal(after.draft_artifact_revisions.length, 1);
    assert.equal(after.draft_lifecycle_events.length, 1);
  } finally {
    if (faultInstalled) await queryPostgres(`DROP TRIGGER ${functionName} ON storyos.draft_lifecycle_events;
      DROP FUNCTION storyos.${functionName}();`);
    await stopRealServer(started.server);
  }
});

test("243 ordered sources and two distinct Proposal owners retain a near-1-MiB structured request", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e0c111"), "Bounded Refused Draft", "e0c2");
    const writer = await writePassage(started.baseUrl, prepared.fetchImpl, prepared.projectId, "e0c3");
    const initial = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    const firstBlock = initial.chapter.current_revision.blocks[0]!;
    for (const [batch, count] of [240, 2].entries()) {
      const request = replaceUnit(0, 0, "", writer, `e0c4${batch}`, id("e0cff1"));
      request.expected_proposal_head_revision_ids = [];
      request.observed_ownership_partition = "authoritative";
      request.author_edit_units = [{ normalized_primitives: Array.from({ length: count }, (_, index) => ({
        kind: "split_block" as const, manuscript_block_id: firstBlock.manuscript_block_id,
        offset: PROSE.length, new_manuscript_block_id: id(`e0c${batch}${index.toString(16).padStart(3, "0")}`),
      })), selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 0 } }];
      const result = await sendMixed(started.baseUrl, prepared, request, id(`e0c5${batch}`));
      if (result.effect.kind !== "authoritative_applied") throw new Error("expected lawful Block splits");
      writer.authoritativeRevisionId = result.effect.authoritative_revision.revision_id;
      writer.nextSequence = String(batch + 3);
    }
    const before = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    assert.equal(before.chapter.current_revision.blocks.length, 243);
    const proposals: Awaited<ReturnType<typeof getProposal>>[] = [];
    for (const index of [0, 1]) {
      const request = phraseRequest(prepared.chapterId, id(`e0c6${index}`));
      request.create_agent_run_input.author_message.text = "Revise this passage: keep the voice.";
      const key = id(`e0c7${index}`);
      const admitted = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
        "/api/v1/projects/{project_id}/agent-runs", request.command_schema,
        await digestCreateAgentRun(request), key, (antiForgery) => createAgentRun({ baseUrl: started.baseUrl,
          projectId: prepared.projectId, fetchImpl: prepared.fetchImpl, request, idempotencyKey: key, antiForgery }));
      if (admitted.effect.kind !== "admitted") throw new Error("expected admitted run");
      await settleOnce();
      const queried = await getAgentRun({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        runId: admitted.effect.run_id, fetchImpl: prepared.fetchImpl });
      if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present")
        throw new Error("expected public pending Proposal");
      proposals.push(await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: queried.decision.opened_proposal.proposal_id, fetchImpl: prepared.fetchImpl }));
    }
    assert.notEqual(proposals[0]!.proposal.proposal_id, proposals[1]!.proposal.proposal_id);
    const sources: SelectedEditSource[] = before.chapter.current_revision.blocks.map((block) => {
      const proposal = proposals.find((item) => item.proposal.manuscript_block_id === block.manuscript_block_id)?.proposal;
      return proposal === undefined
        ? { owner: { kind: "manuscript", manuscript_block_id: block.manuscript_block_id },
          coordinate_profile: "prosemirror-token-utf16.v1", from: 0, to: block.text.length,
          block_kind: block.block_kind, source_text: block.text }
        : { owner: { kind: "proposal", proposal_id: proposal.proposal_id, operation_id: proposal.operation_id,
          revision_id: proposal.revision_id, manuscript_block_id: block.manuscript_block_id },
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: proposal.candidate_text.length,
          block_kind: block.block_kind, source_text: proposal.candidate_text };
    });
    const request = replaceUnit(0, 0, "", writer, "e0c8", proposals[0]!.proposal.revision_id);
    request.expected_proposal_head_revision_ids = proposals.map((item) => item.proposal.revision_id).sort();
    const replacement = [{ block_kind: "paragraph" as const, text: "" },
      { block_kind: "heading" as const, text: "A retained alternative ✨" }];
    request.author_edit_units = [{ normalized_primitives: [{ kind: "replace_structured_selection", replacement }],
      selection_snapshot: { coordinate_profile: "storyos.editor.ordered-source.v1", from: 0, to: 0,
        ordered_selection: { sources, anchor: { source_index: 0, source_offset: 0 },
          head: { source_index: sources.length - 1, source_offset: 0 } } } }];
    const wholeBodyCeiling = 1_048_576;
    const remaining = wholeBodyCeiling - Buffer.byteLength(JSON.stringify(request));
    replacement[0]!.text = "🙂".repeat(Math.floor(remaining / 4)) + "x".repeat(remaining % 4);
    assert.equal(Buffer.byteLength(JSON.stringify(request)), wholeBodyCeiling);
    const stateBefore = await retainedState(prepared.projectId);
    const result = await sendMixed(started.baseUrl, prepared, request, id("e0c86"));
    if (result.effect.kind !== "refused_to_draft") throw new Error("expected complete bounded Draft");
    const queried = await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: result.effect.draft_id, fetchImpl: prepared.fetchImpl });
    const expectedPayload = { schema_revision: "storyos.refused-edit-payload.v1", chapter_id: prepared.chapterId,
      expected_authoritative_revision_id: writer.authoritativeRevisionId,
      expected_proposal_head_revision_ids: request.expected_proposal_head_revision_ids,
      target_refs: [`manuscript:${prepared.chapterId}`], author_edit_units: request.author_edit_units,
      undo_group_id: id("e0c84"), completed_intent_record_id: id("e0c85"), local_intent_sequence: "4" };
    assert.deepEqual(queried.draft.payload, expectedPayload);
    const canonicalPayload = JSON.stringify(expectedPayload, (_key, value: unknown) =>
      value !== null && typeof value === "object" && !Array.isArray(value)
        ? Object.fromEntries(Object.entries(value).sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)) : value);
    assert.equal(queried.draft.payload_digest, createHash("sha256").update(canonicalPayload).digest("hex"));
    const stateAfter = await retainedState(prepared.projectId);
    assert.deepEqual({ ...stateAfter, draft_artifacts: [], draft_artifact_revisions: [], draft_lifecycle_events: [] }, stateBefore);
    const oversized = structuredClone(request);
    oversized.correlation_id = id("e0c91");
    oversized.completed_intent_record_id = id("e0c92");
    oversized.local_intent_sequence = "5";
    const primitive = oversized.author_edit_units[0]!.normalized_primitives[0]!;
    if (primitive.kind !== "replace_structured_selection") throw new Error("expected structured replacement");
    primitive.replacement[0]!.text += "x";
    assert.equal(Buffer.byteLength(JSON.stringify(oversized)), wholeBodyCeiling + 1);
    await assert.rejects(() => sendMixed(started.baseUrl, prepared, oversized, id("e0c93")),
      (error) => requireStoryOSProtocolError(error).status === 413);
    assert.deepEqual(await retainedState(prepared.projectId), stateAfter);
    await retainRefusedEditRecoveryExpectation(prepared.projectId, [{ draft: queried.draft, available: true }]);
  } finally { await stopRealServer(started.server); }
});

test("closed and archived Drafts keep their lifecycle, while tombstoned content fences old packages and leaves exact archive gaps", async () => {
  let started = await startRealServer();
  const faultName = "draft_close_test_precommit_fault";
  let faultInstalled = false;
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e0d111"), "Draft Lifecycle Export", "e0d2");
    const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, prepared.chapterId, "e0d3");
    const drafts: Awaited<ReturnType<typeof getRefusedEditDraft>>[] = [];
    const results: Awaited<ReturnType<typeof applyAuthorEdit>>[] = [];
    for (const [index, text] of ["Closed alternate", "Archived alternate ✨", "Restricted erased alternative 🌘"].entries()) {
      const request = mixedRequest(opened, writer, `e0d4${index}`);
      request.local_intent_sequence = String(index + 2);
      const primitive = request.author_edit_units[0]!.normalized_primitives[0]!;
      if (primitive.kind !== "replace_structured_selection") throw new Error("expected structured replacement");
      primitive.replacement[0]!.text = text;
      const result = await sendMixed(started.baseUrl, prepared, request, id(`e0d5${index}`));
      if (result.effect.kind !== "refused_to_draft") throw new Error("expected fresh lifecycle Draft");
      const queried = await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        draftId: result.effect.draft_id, fetchImpl: prepared.fetchImpl });
      assert.deepEqual(queried.draft.payload.author_edit_units, request.author_edit_units);
      drafts.push(queried);
      results.push(result);
    }
    const [closed, archived, tombstoned] = drafts;
    if (!closed || !archived || !tombstoned) throw new Error("expected three retained Drafts");
    const closeRequest: CloseEditorFlowDraftRequest = { command_schema: "storyos.command.close-editor-flow-draft.request.v1",
      close_editor_flow_draft_input: { ...BINDING, correlation_id: id("e0db1"),
        editor_session_id: writer.session.editor_session.editor_session_id, writer_generation: writer.writerGeneration,
        draft_kind: "refused_edit", draft_id: closed.draft.draft_id,
        source_current_draft_revision_id: closed.draft.draft_revision_id,
        source_draft_payload_digest: closed.draft.payload_digest, expected_closure: "open", close_reason: "abandoned" } };
    await queryPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count=0
      WHERE owner_user_id='${USER_A}'::uuid AND project_id='${prepared.projectId}'::uuid`);
    const secondaryRequest: CreateEditorSessionRequest = { command_schema: "storyos.command.create-editor-session.request.v1",
      ...BINDING, correlation_id: id("e0db91") };
    const secondary = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/editor-sessions", secondaryRequest.command_schema,
      await digestCreateEditorSession(secondaryRequest), id("e0db92"), (antiForgery) => createEditorSession({
        baseUrl: started.baseUrl, projectId: prepared.projectId, request: secondaryRequest,
        idempotencyKey: id("e0db92"), antiForgery, fetchImpl: prepared.fetchImpl }));
    assert.deepEqual(secondary.writer, { kind: "read_only", reason: "secondary_session", observed_writer_generation: writer.writerGeneration });
    let afterTakeovers: Awaited<ReturnType<typeof retainedState>> | undefined;
    await assert.rejects(async () => challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/drafts/{draft_id}/closures", closeRequest.command_schema,
      await digestCloseEditorFlowDraft(closeRequest), id("e0db94"), async (antiForgery) => {
        for (const [index, sessionId] of [secondary.editor_session.editor_session_id,
          writer.session.editor_session.editor_session_id].entries()) {
          const takeoverRequest = { command_schema: "storyos.command.take-over-project-writer.request.v1", ...BINDING,
            correlation_id: id(`e0db95${index}`), editor_session_id: sessionId,
            observed_writer_generation: writer.writerGeneration, editor_contract_revision: "storyos.editor-contract.release-1.v3" };
          const takeoverKey = id(`e0db96${index}`);
          const takeover = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
            "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers", takeoverRequest.command_schema,
            await digestTakeOverProjectWriter(takeoverRequest), takeoverKey, (nonce) => takeOverProjectWriter({
              baseUrl: started.baseUrl, projectId: prepared.projectId, editorSessionId: sessionId, request: takeoverRequest,
              idempotencyKey: takeoverKey, antiForgery: nonce, fetchImpl: prepared.fetchImpl }));
          if (takeover.result.kind !== "takeover_applied") throw new Error("expected writer generation advance");
          writer.writerGeneration = takeover.result.resulting_writer_generation;
        }
        afterTakeovers = await retainedState(prepared.projectId);
        return closeEditorFlowDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
          draftId: closed.draft.draft_id, request: closeRequest, idempotencyKey: id("e0db94"), antiForgery,
          fetchImpl: prepared.fetchImpl });
      }), (error) => requireStoryOSProtocolError(error).status === 409);
    assert.deepEqual(await retainedState(prepared.projectId), afterTakeovers);
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.author_command_admissions
      WHERE project_id='${prepared.projectId}'::uuid AND idempotency_key='${id("e0db94")}'::uuid`), "0");
    closeRequest.close_editor_flow_draft_input.writer_generation = writer.writerGeneration;
    const beforeClose = await retainedState(prepared.projectId);
    const key = id("e0db2");
    const digest = await digestCloseEditorFlowDraft(closeRequest);
    let nonce = "";
    const sendClose = (request = closeRequest, antiForgery = nonce, idempotencyKey = key,
      draftId = closed.draft.draft_id, fetchImpl = prepared.fetchImpl) => closeEditorFlowDraft({
      baseUrl: started.baseUrl, projectId: prepared.projectId, draftId, fetchImpl, request, idempotencyKey, antiForgery });
    await queryPostgres(`CREATE FUNCTION storyos.${faultName}() RETURNS trigger LANGUAGE plpgsql AS $fault$
      BEGIN IF NEW.project_id='${prepared.projectId}'::uuid THEN RAISE EXCEPTION 'Controlled Draft close precommit failure';
      END IF; RETURN NULL; END $fault$;
      CREATE CONSTRAINT TRIGGER ${faultName} AFTER INSERT ON storyos.draft_close_events
      DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.${faultName}();`);
    faultInstalled = true;
    await assert.rejects(() => challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/drafts/{draft_id}/closures", closeRequest.command_schema, digest, key,
      (antiForgery) => { nonce = antiForgery; return sendClose(); }),
      (error) => requireStoryOSProtocolError(error).status === 503);
    assert.deepEqual(await retainedState(prepared.projectId), beforeClose);
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.domain_receipts
      WHERE project_id='${prepared.projectId}'::uuid AND idempotency_key='${key}'::uuid`), "0");
    await queryPostgres(`DROP TRIGGER ${faultName} ON storyos.draft_close_events; DROP FUNCTION storyos.${faultName}();`);
    faultInstalled = false;
    let lost: CloseEditorFlowDraftResponse | undefined;
    const lossyFetch: typeof fetch = async (input, init) => {
      const response = await prepared.fetchImpl(input, init);
      assert.equal(response.status, 200, await response.clone().text());
      lost = await response.clone().json() as CloseEditorFlowDraftResponse;
      await stopRealServer(started.server);
      throw new Error("Controlled Discard response loss after durable settlement");
    };
    await assert.rejects(() => sendClose(closeRequest, nonce, key, closed.draft.draft_id, lossyFetch), /Controlled Discard response loss/);
    const origin = new URL(started.baseUrl);
    started = await startRealServer(`${origin.hostname}:${origin.port}`);
    prepared.fetchImpl = browserFetch(started.baseUrl, "session-a");
    const [closeResponse, concurrentReplay] = await Promise.all([sendClose(), sendClose()]);
    assert.deepEqual(closeResponse, lost);
    assert.deepEqual(concurrentReplay, closeResponse);
    if (closeResponse.effect.kind !== "draft_closure_changed") throw new Error("expected complete Discard");
    const closeEvent = closeResponse.effect.event;
    const scope = { owner_user_id: USER_A, project_id: prepared.projectId };
    for (const value of [closeResponse.command_id, closeResponse.author_command_admission_id,
      closeResponse.receipt.receipt_id, closeEvent.event_id]) assert.match(value, UUID_V7);
    assert.deepEqual(closeEvent, { schema_id: "storyos.event.editor-flow-draft-closed.v1", event_kind: "editor_flow_draft_closed",
      event_id: closeEvent.event_id, project_scope: scope, draft_id: closed.draft.draft_id,
      draft_revision_id: closed.draft.draft_revision_id, payload_digest: closed.draft.payload_digest,
      prior_closure: "open", closure: "closed", close_reason: "abandoned",
      source: { command_id: closeResponse.command_id, author_command_admission_id: closeResponse.author_command_admission_id,
        receipt_id: closeResponse.receipt.receipt_id, idempotency_key: key, command_digest: digest },
      author_action_sequence: String(Number(beforeClose.scope_counters[0].author_action_sequence) + 1),
      created_at: closeResponse.receipt.created_at });
    assert.deepEqual(closeResponse.receipt, { receipt_id: closeResponse.receipt.receipt_id, project_scope: scope,
      command_kind: "closeEditorFlowDraft", command_digest: digest, idempotency_key: key,
      producer_cause: "author_command_admission", author_command_admission_id: closeResponse.author_command_admission_id,
      expected_heads: [], prior_heads: [], resulting_heads: [], authoritative_revision_ids: [], proposal_revision_ids: [],
      authoritative_commit_ids: [], author_action_sequence: closeEvent.author_action_sequence,
      draft_artifact_refs: [closed.draft.draft_id], artifact_lifecycle_event_refs: [closeEvent.event_id], condition_refs: [],
      result: "draft_closure_changed", created_at: closeEvent.created_at });
    const closedDraft = { ...closed.draft, closure: "closed", closure_event: closeEvent };
    assert.deepEqual((await getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: closed.draft.draft_id, fetchImpl: prepared.fetchImpl })).draft, closedDraft);
    const afterClose = await retainedState(prepared.projectId);
    await assert.rejects(() => sendClose({ ...closeRequest, close_editor_flow_draft_input: {
      ...closeRequest.close_editor_flow_draft_input, draft_id: archived.draft.draft_id } }),
      (error) => requireStoryOSProtocolError(error).status === 400);
    assert.deepEqual(await retainedState(prepared.projectId), afterClose);
    for (const table of ["authoritative_heads", "authoritative_revisions", "authoritative_commits", "project_activity_events",
      "proposals", "proposal_heads", "proposal_revisions", "proposal_operations", "draft_artifact_revisions", "draft_lifecycle_events"])
      assert.deepEqual(afterClose[table], beforeClose[table]);
    assert.equal(afterClose.draft_close_events.length, 1);
    assert.equal(afterClose.author_action_entries.length, beforeClose.author_action_entries.length + 1);
    assert.deepEqual(await sendClose(), closeResponse);
    assert.deepEqual(await retainedState(prepared.projectId), afterClose);
    const settleFresh = async (request: CloseEditorFlowDraftRequest, keySuffix: string, draftId = closed.draft.draft_id) =>
      challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
        "/api/v1/projects/{project_id}/drafts/{draft_id}/closures", request.command_schema,
        await digestCloseEditorFlowDraft(request), id(keySuffix), (antiForgery) => sendClose(request, antiForgery, id(keySuffix), draftId));
    for (const [suffix, input, expected] of [
      ["e0db3", { ...closeRequest.close_editor_flow_draft_input, source_current_draft_revision_id: id("e0dff4"), source_draft_payload_digest: "0".repeat(64) },
        { kind: "conflicted", current_revision_id: closed.draft.draft_revision_id, current_digest: closed.draft.payload_digest, current_closure: "closed" }],
      ["e0db4", closeRequest.close_editor_flow_draft_input,
        { kind: "refused", reason: "source_draft_not_open", current_closure: "closed" }],
    ] as const) {
      const refused = await settleFresh({ ...closeRequest, close_editor_flow_draft_input: input }, suffix);
      assert.deepEqual(refused.effect, expected);
      assert.deepEqual(refused.receipt.authoritative_commit_ids, []);
      assert.deepEqual(refused.receipt.artifact_lifecycle_event_refs, []);
      assert.equal(refused.receipt.author_action_sequence, null);
      assert.deepEqual(await retainedState(prepared.projectId), afterClose);
    }
    await assert.rejects(() => settleFresh({ ...closeRequest, close_editor_flow_draft_input: {
      ...closeRequest.close_editor_flow_draft_input, editor_session_id: secondary.editor_session.editor_session_id } }, "e0db93"),
      (error) => requireStoryOSProtocolError(error).status === 409);
    await assert.rejects(() => sendClose(closeRequest, nonce, key, closed.draft.draft_id, browserFetch(started.baseUrl, "session-b")),
      (error) => [404, 422].includes(requireStoryOSProtocolError(error).status ?? 0));
    await assert.rejects(() => getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: closed.draft.draft_id, fetchImpl: browserFetch(started.baseUrl, "session-b") }),
      (error) => requireStoryOSProtocolError(error).status === 404);
    assert.deepEqual(await retainedState(prepared.projectId), afterClose);
    const undoRequest = { command_schema: "storyos.command.undo-latest-author-action.request.v1",
      undo_latest_author_action_input: { ...BINDING, correlation_id: id("e0db6"),
        editor_session_id: writer.session.editor_session.editor_session_id,
        expected_author_undo_frontier_sequence: String(Number(closeEvent.author_action_sequence) - 1),
        expected_authoritative_revision_id: writer.authoritativeRevisionId } };
    const undo = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/author-actions/undo", undoRequest.command_schema,
      await digestUndoLatestAuthorAction(undoRequest), id("e0db7"), (antiForgery) => undoLatestAuthorAction({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
        request: undoRequest, idempotencyKey: id("e0db7"), antiForgery }));
    assert.deepEqual(undo.effect, { kind: "conflicted", reason: "frontier_mismatch",
      current_author_undo_frontier_sequence: closeEvent.author_action_sequence });
    assert.deepEqual(await retainedState(prepared.projectId), afterClose);
    await assert.rejects(() => queryPostgres(`UPDATE storyos.draft_artifacts SET closure='closed'
      WHERE project_id='${prepared.projectId}'::uuid AND draft_id='${archived.draft.draft_id}'::uuid`), /Incomplete Draft Discard settlement/);
    assert.deepEqual(await retainedState(prepared.projectId), afterClose);
    for (const mutation of [
      `UPDATE storyos.draft_artifacts SET closure='open',close_event_id=NULL
        WHERE project_id='${prepared.projectId}'::uuid AND draft_id='${closed.draft.draft_id}'::uuid`,
      `UPDATE storyos.draft_artifacts SET closure='closed',close_event_id='${closeEvent.event_id}'::uuid
        WHERE project_id='${prepared.projectId}'::uuid AND draft_id='${archived.draft.draft_id}'::uuid`,
    ]) {
      await assert.rejects(() => queryPostgres(mutation), /Draft Discard/);
      assert.deepEqual(await retainedState(prepared.projectId), afterClose);
    }
    const priorExports: { exportId: string; root: string }[] = [];
    for (const index of [0, 1]) {
      const request = { command_schema: "storyos.command.export-project-archive.request.v1" as const,
        export_project_archive_input: { ...BINDING, correlation_id: id(`e0da${index}1`), archive_profile: "storyos.project-export.v1",
          archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" } };
      const key = id(`e0da${index}2`);
      const admitted = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
        "/api/v1/projects/{project_id}/exports", request.command_schema, await digestExportProjectArchive(request), key,
        (antiForgery) => exportProjectArchive({ baseUrl: started.baseUrl, projectId: prepared.projectId,
          fetchImpl: prepared.fetchImpl, request, idempotencyKey: key, antiForgery }));
      if (admitted.effect.kind !== "admitted") throw new Error("expected retained source export");
      await settleOnce();
      const ready = await getExportOperation({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        exportId: admitted.effect.export_id, fetchImpl: prepared.fetchImpl });
      if (ready.status !== "ready") throw new Error("expected completed prior export");
      priorExports.push({ exportId: ready.export_id, root: ready.immutable_root });
      const download = await prepared.fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${ready.export_id}`,
        { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
      assert.equal(download.status, 200);
      const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
      assert.ok([...files.values()].some((content) => new TextDecoder().decode(content).includes("Restricted erased alternative")));
    }
    const pinnedSources = JSON.parse(await queryPostgres(`SELECT jsonb_agg(to_jsonb(source) ORDER BY source.export_id)
      FROM storyos.pinned_export_sources AS source WHERE project_id='${prepared.projectId}'::uuid`));
    const archivedClose = await settleFresh({ ...closeRequest, close_editor_flow_draft_input: {
      ...closeRequest.close_editor_flow_draft_input, draft_id: archived.draft.draft_id,
      source_current_draft_revision_id: archived.draft.draft_revision_id, source_draft_payload_digest: archived.draft.payload_digest } }, "e0db81", archived.draft.draft_id);
    if (archivedClose.effect.kind !== "draft_closure_changed") throw new Error("expected archived source Discard");
    await queryPostgres(`UPDATE storyos.draft_artifacts SET retention_state='archived'
      WHERE project_id='${prepared.projectId}'::uuid AND draft_id='${archived.draft.draft_id}'::uuid;
      UPDATE storyos.draft_artifacts SET retention_state='tombstoned'
      WHERE project_id='${prepared.projectId}'::uuid AND draft_id='${tombstoned.draft.draft_id}'::uuid;`);
    assert.ok(archivedClose.effect.event.author_action_sequence);
    const unavailableUndoRequest = { ...undoRequest, undo_latest_author_action_input: { ...undoRequest.undo_latest_author_action_input,
      expected_author_undo_frontier_sequence: archivedClose.effect.event.author_action_sequence } };
    const retainedBeforeUnavailable = await retainedState(prepared.projectId);
    const unavailableUndo = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/author-actions/undo", undoRequest.command_schema, await digestUndoLatestAuthorAction(unavailableUndoRequest),
      id("e0db82"), (antiForgery) => undoLatestAuthorAction({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        request: unavailableUndoRequest, idempotencyKey: id("e0db82"), antiForgery, fetchImpl: prepared.fetchImpl }));
    assert.deepEqual(unavailableUndo.effect, { kind: "unavailable", reason: "source_unavailable" });
    assert.deepEqual(await retainedState(prepared.projectId), retainedBeforeUnavailable);
    const unavailable = await settleFresh({ ...closeRequest, close_editor_flow_draft_input: {
      ...closeRequest.close_editor_flow_draft_input, draft_id: tombstoned.draft.draft_id, source_current_draft_revision_id: tombstoned.draft.draft_revision_id,
      source_draft_payload_digest: tombstoned.draft.payload_digest } }, "e0db8", tombstoned.draft.draft_id);
    assert.deepEqual(unavailable.effect, { kind: "refused", reason: "source_unavailable", current_closure: "open" });
    const read = (draftId: string, projectId = prepared.projectId) => getRefusedEditDraft({ baseUrl: started.baseUrl,
      projectId, draftId, fetchImpl: prepared.fetchImpl });
    assert.deepEqual((await read(closed.draft.draft_id)).draft, closedDraft);
    for (const draft of [archived, tombstoned]) await assert.rejects(() => read(draft.draft.draft_id),
      (error) => requireStoryOSProtocolError(error).status === 404);
    await assert.rejects(() => read(closed.draft.draft_id, id("e0dff1")),
      (error) => requireStoryOSProtocolError(error).status === 404);
    for (const prior of priorExports) {
      const download = await prepared.fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${prior.exportId}`,
        { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
      assert.equal(download.status, 422);
      assert.deepEqual(await download.json(), { schema_id: "storyos.problem.v1", code: "ineligible_lifecycle",
        message: "The Project Export Archive did not complete." });
      const ready = await getExportOperation({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        exportId: prior.exportId, fetchImpl: prepared.fetchImpl });
      if (ready.status !== "ready") throw new Error("eligibility must not rewrite the historical export");
      assert.equal(ready.immutable_root, prior.root);
    }
    const before = await retainedState(prepared.projectId);
    for (const table of ["draft_artifact_revisions", "draft_lifecycle_events", "draft_close_events"]) {
      await assert.rejects(() => queryPostgres(`DELETE FROM storyos.${table}
        WHERE project_id='${prepared.projectId}'::uuid`), /immutable/);
    }
    await assert.rejects(() => queryPostgres(`BEGIN;
      INSERT INTO storyos.draft_artifacts(owner_user_id,project_id,draft_id,current_revision_id)
      VALUES('${USER_A}','${prepared.projectId}','${id("e0dff2")}','${id("e0dff3")}');
      INSERT INTO storyos.draft_artifact_revisions(owner_user_id,project_id,draft_id,revision_id,payload,payload_digest)
      SELECT owner_user_id,project_id,'${id("e0dff2")}'::uuid,'${id("e0dff3")}'::uuid,payload,payload_digest
      FROM storyos.draft_artifact_revisions WHERE draft_id='${closed.draft.draft_id}'::uuid; COMMIT;`),
      /Incomplete Refused Edit Draft settlement/);
    assert.deepEqual(await retainedState(prepared.projectId), before);
    const admissionRows = JSON.parse(await queryPostgres(`SELECT jsonb_agg(to_jsonb(admission) ORDER BY to_jsonb(admission)::text)
      FROM storyos.author_command_admissions AS admission WHERE command_kind='applyAuthorEdit' AND project_id='${prepared.projectId}'::uuid`));
    const request = { command_schema: "storyos.command.export-project-archive.request.v1" as const,
      export_project_archive_input: { ...BINDING, correlation_id: id("e0d61"), archive_profile: "storyos.project-export.v1",
        archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" } };
    const admitted = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/exports", request.command_schema, await digestExportProjectArchive(request),
      id("e0d62"), (antiForgery) => exportProjectArchive({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl, request, idempotencyKey: id("e0d62"), antiForgery }));
    if (admitted.effect.kind !== "admitted") throw new Error("expected authorized lifecycle archive");
    await settleOnce();
    const download = await prepared.fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${admitted.effect.export_id}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
    assert.equal(download.status, 200);
    const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
    const revisionGap = { kind: "refused_edit_revision_payload", reason: "withheld_due_to_tombstone",
      entry_path: "canonical/draft_artifact_revisions.json", record_id: tombstoned.draft.draft_revision_id,
      payload_field: "payload", draft_id: tombstoned.draft.draft_id, retention_state: "tombstoned",
      payload_digest: tombstoned.draft.payload_digest, payload_digest_profile: "storyos.refused-edit-payload.jcs.v1" };
    const admissionGap = { kind: "refused_edit_admission_payload", reason: "withheld_due_to_tombstone",
      entry_path: "canonical/author_command_admissions.json", record_id: results[2]!.author_command_admission_id,
      payload_field: "command_payload", draft_id: tombstoned.draft.draft_id, retention_state: "tombstoned",
      command_id: results[2]!.command_id,
      canonical_command_digest: `sha256:${results[2]!.receipt.command_digest.profile}:${results[2]!.receipt.command_digest.value_hex_lowercase}` };
    const expectedRevisions = before.draft_artifact_revisions.map((row: Record<string, unknown>) => {
      if (row.draft_id !== tombstoned.draft.draft_id) return row;
      const { payload: _withheld, ...metadata } = row;
      return { ...metadata, payload_availability: revisionGap };
    });
    const expectedAdmissions = admissionRows.map((row: Record<string, unknown>) => {
      if (row.author_command_admission_id !== results[2]!.author_command_admission_id) return row;
      const { command_payload: _withheld, ...metadata } = row;
      return { ...metadata, payload_availability: admissionGap };
    });
    assert.deepEqual(JSON.parse(new TextDecoder().decode(files.get("canonical/draft_artifacts.json"))), before.draft_artifacts);
    assert.deepEqual(JSON.parse(new TextDecoder().decode(files.get("canonical/draft_lifecycle_events.json"))), before.draft_lifecycle_events);
    assert.deepEqual(JSON.parse(new TextDecoder().decode(files.get("canonical/draft_close_events.json"))), before.draft_close_events);
    assert.deepEqual(JSON.parse(new TextDecoder().decode(files.get("canonical/draft_artifact_revisions.json"))), expectedRevisions);
    const exportedAdmissions = JSON.parse(new TextDecoder().decode(files.get("canonical/author_command_admissions.json")));
    for (const [table, path] of [["author_command_admissions", "canonical/author_command_admissions.json"],
      ["domain_receipts", "canonical/domain_receipts.json"]] as const) {
      const expected = JSON.parse(await queryPostgres(`SELECT jsonb_agg(to_jsonb(record) ORDER BY to_jsonb(record)::text)
        FROM storyos.${table} AS record WHERE project_id='${prepared.projectId}'::uuid AND command_kind='closeEditorFlowDraft'`));
      const exported = JSON.parse(new TextDecoder().decode(files.get(path)))
        .filter((record: Record<string, unknown>) => record.command_kind === "closeEditorFlowDraft");
      const identity = table === "author_command_admissions" ? "author_command_admission_id" : "receipt_id";
      const compare = (a: Record<string, unknown>, b: Record<string, unknown>) => String(a[identity]).localeCompare(String(b[identity]));
      assert.deepEqual(exported.sort(compare), expected.sort(compare));
    }
    assert.deepEqual(JSON.parse(new TextDecoder().decode(files.get("canonical/author_action_entries.json"))), before.author_action_entries);
    for (const content of files.values()) assert.ok(!new TextDecoder().decode(content).includes(nonce));
    assert.deepEqual(exportedAdmissions.filter((row: Record<string, unknown>) => row.command_kind === "applyAuthorEdit"), expectedAdmissions);
    const exportedPins = JSON.parse(new TextDecoder().decode(files.get("canonical/pinned_export_sources.json")));
    const expectedPins = pinnedSources.map((row: Record<string, unknown>) => {
      const { facts: _withheld, ...metadata } = row;
      return { ...metadata, payload_availability: { kind: "refused_edit_pinned_export_source_facts",
        reason: "withheld_due_to_tombstone", entry_path: "canonical/pinned_export_sources.json", record_id: row.export_id,
        payload_field: "facts", restricted_draft_ids: [tombstoned.draft.draft_id], facts_sha256: row.facts_sha256 } };
    });
    assert.deepEqual(exportedPins, expectedPins);
    for (const content of files.values()) assert.ok(!new TextDecoder().decode(content).includes("Restricted erased alternative"));
    const after = await retainedState(prepared.projectId);
    assert.deepEqual(after, { ...before, scope_counters: before.scope_counters.map((row: Record<string, number>) =>
      ({ ...row, project_activity_position: row.project_activity_position! + 1 })) });
    const restoredExports = priorExports.map((prior) => ({ ...prior, status: 422 }));
    const newReady = await getExportOperation({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      exportId: admitted.effect.export_id, fetchImpl: prepared.fetchImpl });
    if (newReady.status !== "ready") throw new Error("expected ready lifecycle archive");
    const newDownload = await prepared.fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${newReady.export_id}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
    assert.equal(newDownload.status, 200);
    const archiveExpectation = { exportId: newReady.export_id, root: newReady.immutable_root, status: 200,
      bytesSha256: createHash("sha256").update(new Uint8Array(await newDownload.arrayBuffer())).digest("hex") };
    const archivalRequest = { command_schema: "storyos.command.archive-project.request.v1" as const,
      archive_project_input: { ...BINDING, correlation_id: id("e0d71"), expected_project_revision: "1" } };
    const archivedProject = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "PUT",
      "/api/v1/projects/{project_id}/archival", archivalRequest.command_schema,
      await digestArchiveProject(archivalRequest), id("e0d72"), (antiForgery) => archiveProject({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
        request: archivalRequest, antiForgery, idempotencyKey: id("e0d72") }));
    assert.equal(archivedProject.effect.kind, "authoritative_applied");
    await assert.rejects(() => read(closed.draft.draft_id), (error) => requireStoryOSProtocolError(error).status === 404);
    await retainRefusedEditRecoveryExpectation(prepared.projectId,
      [{ draft: closedDraft, available: false },
        { draft: { ...archived.draft, retention_state: "archived" }, available: false },
        { draft: { ...tombstoned.draft, retention_state: "tombstoned" }, available: false }],
      [...restoredExports, archiveExpectation]);
  } finally { await stopRealServer(started.server); }
});

test("a distant generating Proposal does not block a proven span, while a selected generating source stays unchanged", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e0e111"), "Generating Source Boundary", "e0e2");
    const writer = await writePassage(started.baseUrl, prepared.fetchImpl, prepared.projectId, "e0e3");
    const chapter = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    const split = replaceUnit(0, 0, "", writer, "e0e4", id("e0eff1"));
    split.expected_proposal_head_revision_ids = [];
    split.observed_ownership_partition = "authoritative";
    split.author_edit_units = [{ normalized_primitives: [{ kind: "split_block",
      manuscript_block_id: chapter.chapter.current_revision.blocks[0]!.manuscript_block_id,
      offset: PROSE.length, new_manuscript_block_id: id("e0e41") }],
      selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: PROSE.length, to: PROSE.length } }];
    const divided = await sendMixed(started.baseUrl, prepared, split, id("e0e42"));
    assert.equal(divided.effect.kind, "authoritative_applied", JSON.stringify(divided));
    if (divided.effect.kind !== "authoritative_applied") throw new Error("expected lawful split");
    writer.authoritativeRevisionId = divided.effect.authoritative_revision.revision_id;
    writer.nextSequence = "3";
    const completed = await admitPhrase(started.baseUrl, prepared.fetchImpl, prepared.projectId,
      prepared.chapterId, id("e0e51"));
    if (completed.decision.kind !== "prose_change" || completed.decision.opened_proposal.kind !== "present")
      throw new Error("expected ready inline Proposal");
    const ready = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: completed.decision.opened_proposal.proposal_id, fetchImpl: prepared.fetchImpl });
    const stream = phraseRequest(prepared.chapterId, id("e0e61"));
    stream.create_agent_run_input.author_message.text = "Stream this passage: keep the voice.";
    const admitted = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/agent-runs", stream.command_schema, await digestCreateAgentRun(stream),
      id("e0e62"), (antiForgery) => createAgentRun({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl, request: stream, idempotencyKey: id("e0e62"), antiForgery }));
    if (admitted.effect.kind !== "admitted") throw new Error("expected streamed admission");
    await settleOnce();
    const run = await getAgentRun({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      runId: admitted.effect.run_id, fetchImpl: prepared.fetchImpl });
    if (run.decision.kind !== "prose_change" || run.decision.opened_proposal.kind !== "present")
      throw new Error("expected generating Proposal");
    const generating = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: run.decision.opened_proposal.proposal_id, fetchImpl: prepared.fetchImpl });
    assert.equal(generating.proposal.generation, "generating");
    assert.equal(generating.proposal.candidate_text, "Guard");
    assert.notEqual(generating.proposal.manuscript_block_id, ready.proposal.manuscript_block_id);
    const request = mixedRequest(ready, writer, "e0e7");
    request.expected_proposal_head_revision_ids = [ready.proposal.revision_id, generating.proposal.revision_id].sort();
    const before = await retainedState(prepared.projectId);
    const accepted = await sendMixed(started.baseUrl, prepared, request, id("e0e76"));
    if (accepted.effect.kind !== "refused_to_draft") throw new Error("expected proven distant-source Draft");
    const after = await retainedState(prepared.projectId);
    assert.deepEqual({ ...after, draft_artifacts: [], draft_artifact_revisions: [], draft_lifecycle_events: [] }, before);
    const selected = structuredClone(request);
    selected.correlation_id = id("e0e81");
    selected.completed_intent_record_id = id("e0e82");
    selected.local_intent_sequence = "4";
    const selection = selected.author_edit_units[0]!.selection_snapshot;
    if (selection.ordered_selection == null) throw new Error("expected ordered selection");
    selection.from = 0;
    selection.to = 5;
    const sources = selection.ordered_selection.sources;
    sources.shift();
    sources[1]!.to = PROSE.length;
    sources.push({ owner: { kind: "proposal", proposal_id: generating.proposal.proposal_id,
      operation_id: generating.proposal.operation_id, revision_id: generating.proposal.revision_id,
      manuscript_block_id: generating.proposal.manuscript_block_id },
      coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 5,
      block_kind: "paragraph", source_text: "Guard" });
    selection.ordered_selection.anchor = { source_index: 0, source_offset: 0 };
    selection.ordered_selection.head = { source_index: 2, source_offset: 5 };
    const refused = await sendMixed(started.baseUrl, prepared, selected, id("e0e83"));
    assert.equal(refused.effect.kind, "conflicted");
    assert.deepEqual(await retainedState(prepared.projectId), after);
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: generating.proposal.proposal_id, fetchImpl: prepared.fetchImpl })).proposal, generating.proposal);
  } finally { await drainLeftoverWork(); await stopRealServer(started.server); }
});

test("Root Undo reopens the exact public Discard without changing manuscript or Proposal content", async () => {
  let started = await startRealServer();
  let faultInstalled = false;
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e0fa111"), "Draft Discard Undo", "e0fa2");
    const originalHead = (await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl })).chapter.current_revision.revision_id;
    const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, prepared.chapterId, "e0fa3");
    const created = await sendMixed(started.baseUrl, prepared, mixedRequest(opened, writer, "e0fa4"), id("e0fa46"));
    if (created.effect.kind !== "refused_to_draft") throw new Error("expected public Refused Edit Draft");
    const read = () => getRefusedEditDraft({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      draftId: created.effect.kind === "refused_to_draft" ? created.effect.draft_id : "", fetchImpl: prepared.fetchImpl });
    const retained = (await read()).draft;
    const objects = () => Promise.all([getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl }).then((value) => value.chapter),
    getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl }).then((value) => value.proposal)]);
    const before = await objects();
    const close: CloseEditorFlowDraftRequest = { command_schema: "storyos.command.close-editor-flow-draft.request.v1",
      close_editor_flow_draft_input: { ...BINDING, correlation_id: id("e0fa51"), draft_id: retained.draft_id,
        draft_kind: "refused_edit", source_current_draft_revision_id: retained.draft_revision_id,
        source_draft_payload_digest: retained.payload_digest, expected_closure: "open", close_reason: "abandoned",
        editor_session_id: writer.session.editor_session.editor_session_id, writer_generation: writer.writerGeneration } };
    const closed = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/drafts/{draft_id}/closures", close.command_schema,
      await digestCloseEditorFlowDraft(close), id("e0fa52"), (antiForgery) => closeEditorFlowDraft({
        baseUrl: started.baseUrl, projectId: prepared.projectId, draftId: retained.draft_id, request: close,
        idempotencyKey: id("e0fa52"), antiForgery, fetchImpl: prepared.fetchImpl }));
    if (closed.effect.kind !== "draft_closure_changed") throw new Error("expected public Discard");
    assert.ok(closed.effect.event.author_action_sequence);
    const request = { command_schema: "storyos.command.undo-latest-author-action.request.v1",
      undo_latest_author_action_input: { ...BINDING, correlation_id: id("e0fa61"),
        editor_session_id: writer.session.editor_session.editor_session_id,
        expected_author_undo_frontier_sequence: closed.effect.event.author_action_sequence,
        expected_authoritative_revision_id: writer.authoritativeRevisionId } };
    const staleHeadRequest = { ...request, undo_latest_author_action_input: { ...request.undo_latest_author_action_input,
      expected_authoritative_revision_id: originalHead } };
    const staleHead = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/author-actions/undo", request.command_schema, await digestUndoLatestAuthorAction(staleHeadRequest),
      id("e0fa63"), (antiForgery) => undoLatestAuthorAction({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        request: staleHeadRequest, idempotencyKey: id("e0fa63"), antiForgery, fetchImpl: prepared.fetchImpl }));
    assert.deepEqual(staleHead.effect, { kind: "conflicted", reason: "wrong_target_head" });
    assert.deepEqual(await objects(), before);
    const digest = await digestUndoLatestAuthorAction(request);
    let nonce = "";
    const sendUndo = (fetchImpl = prepared.fetchImpl) => undoLatestAuthorAction({
      baseUrl: started.baseUrl, projectId: prepared.projectId, request, idempotencyKey: id("e0fa62"), antiForgery: nonce, fetchImpl });
    const beforeUndo = await retainedState(prepared.projectId);
    await queryPostgres(`CREATE FUNCTION storyos.undo_draft_831_fault() RETURNS trigger LANGUAGE plpgsql AS $fault$
      BEGIN IF NEW.project_id='${prepared.projectId}'::uuid THEN RAISE EXCEPTION 'Controlled Undo precommit failure';
      END IF; RETURN NULL; END $fault$;
      CREATE CONSTRAINT TRIGGER undo_draft_831_fault AFTER INSERT ON storyos.draft_reopen_events
      DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.undo_draft_831_fault();`);
    faultInstalled = true;
    await assert.rejects(() => challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/author-actions/undo", request.command_schema, digest, id("e0fa62"),
      (antiForgery) => { nonce = antiForgery; return sendUndo(); }), (error) => requireStoryOSProtocolError(error).status === 503);
    assert.deepEqual(await retainedState(prepared.projectId), beforeUndo);
    for (const table of ["domain_receipts", "author_command_admissions"]) assert.equal(await queryPostgres(
      `SELECT count(*)::text FROM storyos.${table} WHERE project_id='${prepared.projectId}'::uuid AND idempotency_key='${id("e0fa62")}'::uuid`), "0");
    await queryPostgres("DROP TRIGGER undo_draft_831_fault ON storyos.draft_reopen_events; DROP FUNCTION storyos.undo_draft_831_fault();");
    faultInstalled = false;
    let lost: Awaited<ReturnType<typeof undoLatestAuthorAction>> | undefined;
    await assert.rejects(() => sendUndo(async (input, init) => {
      const reply = await prepared.fetchImpl(input, init); assert.equal(reply.status, 200, await reply.clone().text());
      lost = await reply.json(); await stopRealServer(started.server); throw new Error("Controlled Undo response loss");
    }), /Controlled Undo response loss/);
    const origin = new URL(started.baseUrl);
    started = await startRealServer(`${origin.hostname}:${origin.port}`);
    prepared.fetchImpl = browserFetch(started.baseUrl, "session-a");
    const [undone, concurrent] = await Promise.all([sendUndo(), sendUndo()]);
    assert.deepEqual(undone, lost); assert.deepEqual(concurrent, undone);
    assert.equal(undone.effect.kind, "draft_compensated");
    const reopened = (await read()).draft;
    assert.equal(reopened.closure, "open");
    assert.deepEqual(reopened.payload, retained.payload);
    assert.deepEqual(reopened.creation, retained.creation);
    assert.deepEqual(reopened.closure_event, closed.effect.event);
    assert.deepEqual(await objects(), before);
    if (undone.effect.kind !== "draft_compensated") throw new Error("expected Draft compensation");
    const event = undone.effect.event, receipt = undone.receipt;
    assert.deepEqual(event, { schema_id: "storyos.event.editor-flow-draft-reopened.v1", event_kind: "editor_flow_draft_reopened",
      event_id: receipt.artifact_lifecycle_event_refs[0], project_scope: closed.project_scope,
      draft_id: retained.draft_id, draft_revision_id: retained.draft_revision_id, payload_digest: retained.payload_digest,
      source_close_event_id: closed.effect.event.event_id, prior_closure: "closed", closure: "open",
      source: { command_id: undone.command_id, author_command_admission_id: undone.author_command_admission_id,
        receipt_id: receipt.receipt_id, idempotency_key: id("e0fa62"), command_digest: digest },
      handler_receipt: { schema_id: "storyos.receipt.draft-reopen.v1", receipt_id: event.handler_receipt.receipt_id,
        project_scope: closed.project_scope, author_undo_receipt_id: receipt.receipt_id,
        source_close_event_id: closed.effect.event.event_id, event_id: event.event_id, result: "draft_reopened", created_at: receipt.created_at },
      source_author_action_sequence: closed.effect.event.author_action_sequence,
      author_action_sequence: String(Number(closed.effect.event.author_action_sequence) + 1), created_at: receipt.created_at });
    assert.deepEqual(reopened, { ...retained, closure_event: closed.effect.event, reopen_event: event });
    assert.deepEqual(receipt, { receipt_id: receipt.receipt_id, project_scope: closed.project_scope,
      command_kind: "undoLatestAuthorAction", command_digest: digest, idempotency_key: id("e0fa62"),
      producer_cause: "author_command_admission", author_command_admission_id: undone.author_command_admission_id,
      created_at: event.created_at, expected_heads: [writer.authoritativeRevisionId], prior_heads: [writer.authoritativeRevisionId],
      resulting_heads: [writer.authoritativeRevisionId], authoritative_revision_ids: [], authoritative_commit_ids: [],
      proposal_revision_ids: [], draft_artifact_refs: [retained.draft_id], artifact_lifecycle_event_refs: [event.event_id],
      author_action_sequence: event.author_action_sequence, condition_refs: [], result: "draft_closure_changed" });
    const afterUndo = await retainedState(prepared.projectId);
    for (const table of ["authoritative_heads", "authoritative_revisions", "authoritative_commits", "project_activity_events",
      "proposals", "proposal_heads", "proposal_revisions", "proposal_operations", "draft_artifact_revisions", "draft_lifecycle_events", "draft_close_events"])
      assert.deepEqual(afterUndo[table], beforeUndo[table]);
    assert.equal(afterUndo.author_action_entries.length, beforeUndo.author_action_entries.length + 1);
    assert.deepEqual(afterUndo.author_action_entries.filter((entry: { receipt_id: string }) => entry.receipt_id === receipt.receipt_id),
      [{ owner_user_id: USER_A, project_id: prepared.projectId, author_action_sequence: Number(event.author_action_sequence),
        disposition: "compensation", compensated_source_sequence: Number(event.source_author_action_sequence),
        receipt_id: receipt.receipt_id, receipt_result_kind: "draft_closure_changed", authoritative_commit_id: null }]);
    await assert.rejects(() => sendUndo(browserFetch(started.baseUrl, "session-b")),
      (error) => [404, 422].includes(requireStoryOSProtocolError(error).status ?? 0));
    assert.deepEqual(await retainedState(prepared.projectId), afterUndo);
    const stale = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/drafts/{draft_id}/closures", close.command_schema,
      await digestCloseEditorFlowDraft(close), id("e0fa72"), (antiForgery) => closeEditorFlowDraft({
        baseUrl: started.baseUrl, projectId: prepared.projectId, draftId: retained.draft_id, request: close,
        idempotencyKey: id("e0fa72"), antiForgery, fetchImpl: prepared.fetchImpl }));
    assert.deepEqual(stale.effect, { kind: "conflicted", current_revision_id: retained.draft_revision_id,
      current_digest: retained.payload_digest, current_closure: "open" });
    assert.deepEqual(await retainedState(prepared.projectId), afterUndo);
    await retainRefusedEditRecoveryExpectation(prepared.projectId, [{ draft: reopened, available: true }]);
  } finally {
    if (faultInstalled) await queryPostgres("DROP TRIGGER undo_draft_831_fault ON storyos.draft_reopen_events; DROP FUNCTION storyos.undo_draft_831_fault();");
    await stopRealServer(started.server);
  }
});

test("a complete source cannot prove a selection inside a UTF-16 surrogate pair", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e0f111"), "Scalar Selection Boundary", "e0f2");
    const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, prepared.chapterId, "e0f3");
    const revision = await sendMixed(started.baseUrl, prepared,
      replaceUnit(14, 17, "🙂", writer, "e0f4", opened.proposal.revision_id), id("e0f46"));
    if (revision.effect.kind !== "proposal_revised") throw new Error("expected lawful candidate edit");
    writer.nextSequence = "3";
    const current = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl });
    assert.equal(current.proposal.candidate_text, "narr🙂r tone");
    const request = mixedRequest(current, writer, "e0f5");
    const selection = request.author_edit_units[0]!.selection_snapshot;
    if (selection.ordered_selection == null) throw new Error("expected ordered selection");
    selection.from = 5;
    selection.ordered_selection.sources.shift();
    selection.ordered_selection.sources[0]!.from = 5;
    selection.ordered_selection.sources[0]!.to = current.proposal.candidate_text.length;
    selection.ordered_selection.sources[0]!.source_text = current.proposal.candidate_text;
    selection.ordered_selection.anchor = { source_index: 0, source_offset: 5 };
    selection.ordered_selection.head = { source_index: 1, source_offset: 26 };
    const before = await retainedState(prepared.projectId);
    const result = await sendMixed(started.baseUrl, prepared, request, id("e0f56"));
    assert.equal(result.effect.kind, "conflicted");
    assert.deepEqual(await retainedState(prepared.projectId), before);
  } finally { await stopRealServer(started.server); }
});

test("explicit Inline candidate input binds the exact Operation and preserves source prose", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("d828011"), "Explicit Inline Novel", "d82802");
    const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, prepared.chapterId, "d82803");
    const options = { baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl };
    const before = await getChapter({ ...options, chapterId: prepared.chapterId });
    const operation = opened.proposal.operations[0]!;
    const request = replaceUnit(5, 5, "xx", writer, "d82804", opened.proposal.revision_id);
    request.proposal_target = { proposal_id: opened.proposal.proposal_id,
      operation_id: operation.operation_id, revision_id: opened.proposal.revision_id,
      manuscript_block_id: opened.proposal.manuscript_block_id };
    const send = async (input: ApplyAuthorEditRequest, key: string) => challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits", input.command_schema,
      await digestApplyAuthorEdit(input), key, (antiForgery) => applyAuthorEdit({
        ...options, request: input, idempotencyKey: key, antiForgery }));
    const wrong = structuredClone(request);
    wrong.correlation_id = id("d828053");
    wrong.completed_intent_record_id = id("d828055");
    wrong.proposal_target!.operation_id = id("d828057");
    const refused = await send(wrong, id("d828056"));
    assert.equal(refused.effect.kind, "refused");
    assert.deepEqual((await getChapter({ ...options, chapterId: prepared.chapterId })).chapter, before.chapter);
    assert.deepEqual((await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal,
      opened.proposal);
    const edited = await send(request, id("d828046"));
    assert.equal(edited.effect.kind, "proposal_revised");
    const revised = (await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal;
    assert.equal(revised.candidate_text, "narraxxtor tone");
    assert.notEqual(revised.revision_id, opened.proposal.revision_id);
    assert.deepEqual((await getChapter({ ...options, chapterId: prepared.chapterId })).chapter, before.chapter);
  } finally { await stopRealServer(started.server); }
});
