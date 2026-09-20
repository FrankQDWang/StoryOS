// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/edit-proposal-candidate-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "vitest";
import {
  acceptProposal, applyAuthorEdit, createAgentRun, createEditorSession,
  digestAcceptProposal, digestApplyAuthorEdit, digestCreateAgentRun,
  digestCreateEditorSession, digestExportProjectArchive, digestRejectProposalOperations,
  exportProjectArchive, getAgentRun, getChapter, getExportOperation, getProposal,
  rejectProposalOperations,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  AcceptProposalRequest, ApplyAuthorEditRequest, CreateAgentRunRequest,
  CreateEditorSessionRequest, RejectProposalOperationsRequest,
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
    editor_contract_revision: "storyos.editor-contract.release-1.v2",
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
    editor_contract_revision: "storyos.editor-contract.release-1.v2",
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
