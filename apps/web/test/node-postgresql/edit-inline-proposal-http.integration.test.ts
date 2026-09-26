// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/edit-proposal-candidate-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "vitest";
import {
  acceptProposal, applyAuthorEdit, createAgentRun, createEditorSession,
  digestAcceptProposal, digestApplyAuthorEdit, digestCreateAgentRun,
  digestCreateEditorSession, digestExportProjectArchive, digestRejectProposalOperations,
  exportProjectArchive, getAgentRun, getChapter, getExportOperation, getProposal, getRefusedEditDraft, getApplyAuthorEditOutcome,
  rejectProposalOperations,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  AcceptProposalRequest, ApplyAuthorEditRequest, CreateAgentRunRequest,
  CreateEditorSessionRequest, RejectProposalOperationsRequest, SelectedEditSource,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { requireStoryOSProtocolError, sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
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
      normalized_primitives: [{ kind: "replace_selection", from: 0, to: 0, text: PROSE }],
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
    assert.deepEqual(drafts, [{ ...rowScope, draft_id: queried.draft.draft_id, kind: "refused_edit",
      current_revision_id: queried.draft.draft_revision_id, closure: "open", retention_state: "retained" }]);
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
      result_kind: "refused_to_draft", created_at: revisionCreatedAt }]);
    for (const content of files.values()) assert.ok(!new TextDecoder().decode(content).includes(originalNonce));
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
