// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/edit-proposal-candidate-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "vitest";
import {
  undoLatestAuthorAction, digestUndoLatestAuthorAction,
  acceptProposal, applyAuthorEdit,
  digestAcceptProposal, digestApplyAuthorEdit, digestExportProjectArchive, digestRejectProposalOperations,
  exportProjectArchive, getChapter, getExportOperation, getProposal,
  rejectProposalOperations, replanProposal, digestReplanProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  AcceptProposalRequest, ApplyAuthorEditRequest, RejectProposalOperationsRequest, ReplanProposalRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { requireStoryOSProtocolError, sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer, queryStoryOSPostgres as queryPostgres } from "../support/node-integration.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { BINDING, PROSE, USER_A, UUID_V7, challenged, drainLeftoverWork, id,
  prepare, seedAcceptanceCondition, settleOnce, startRealServer } from "../support/acceptance.ts";

import { admitCandidateRevision } from "../support/candidate-revision.ts";
import { INLINE_CANDIDATE, SOURCE_SLICE, acceptInline, admitPhrase, openInline, replaceUnit,
  sliceDigest, writePassage } from "../support/inline-proposal.ts";

const INTERIOR_CANDIDATE = "narrxxr tone";
const ACCEPTED_BODY = "Guard the narrxxr tone in this passage.";
const EDGE_BODY = "Keep the narrator voice in this passage.";

async function sendMixed(baseUrl: string, prepared: Awaited<ReturnType<typeof prepare>>,
  request: ApplyAuthorEditRequest, key: string) {
  return challenged(baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
    "/api/v1/projects/{project_id}/manuscript/author-edits", request.command_schema,
    await digestApplyAuthorEdit(request), key, (antiForgery) => applyAuthorEdit({ baseUrl,
      projectId: prepared.projectId, fetchImpl: prepared.fetchImpl, request,
      idempotencyKey: key, antiForgery }));
}

test("inline Proposal uses exact Anchors, keeps source and candidate distinct, and Accepts a splice", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e08111"), "Inline Proposal Novel", "e082");
    let { opened, writer } = await openInline(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, "e083",
    );
    await admitCandidateRevision(started.baseUrl, prepared.fetchImpl, prepared.projectId,
      opened.proposal, opened.proposal.operation_id, id("e0832"), "revise the candidate consistently.");
    await settleOnce();
    const prior = opened.proposal;
    opened = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: prior.proposal_id, fetchImpl: prepared.fetchImpl });
    assert.notEqual(opened.proposal.revision_id, prior.revision_id);
    assert.deepEqual(opened.proposal.anchors, prior.anchors);
    const INTERIOR_CANDIDATE = "narrxxr tone Keep the voice consistent.";
    const ACCEPTED_BODY = "Guard the narrxxr tone Keep the voice consistent. in this passage.";
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

test("edge input stays authoritative, the Proposal Revision stays valid on its base, and Reject leaves prose", async () => {
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
    const edged = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.notEqual(edged.proposal.revision_id, opened.proposal.revision_id);
    assert.deepEqual([edged.proposal.validation, edged.proposal.validation_receipt.kind === "present"
      && edged.proposal.validation_receipt.result, edged.proposal.base_authoritative_revision_id],
    ["valid", "valid", opened.proposal.base_authoritative_revision_id]);
    assert.equal(edged.proposal.candidate_text, INLINE_CANDIDATE);
    const accepted = await acceptInline({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl }, writer, edged.proposal, chapter.chapter.current_revision.revision_id, "e08f");
    if (accepted.effect.kind !== "applied") throw new Error(`expected applied, got ${JSON.stringify(accepted.effect)}`);
    assert.deepEqual([accepted.effect.target_resolution, accepted.effect.authoritative_revision.body],
      ["proven_range", EDGE_BODY.replace(SOURCE_SLICE, INLINE_CANDIDATE)]);
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
    assert.equal(anchors[0].proposal_revision_id, openedSecond.opened.proposal.revision_id);
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
    assert.deepEqual(refused.effect, { kind: "refused", reason: "target_mismatch" });
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

test("canonical Inline boundary input preserves exact target and refuses wrong, stale, or interior ownership", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    for (const [index, offset] of [10, 24].entries()) {
      const ns = `d829${index}`;
      const prepared = await prepare(started.baseUrl, id(`${ns}011`), "Canonical Inline Edge", `${ns}02`);
      const siblingText = "Second paragraph. 😀";
      const writer = await writePassage(started.baseUrl, prepared.fetchImpl,
        prepared.projectId, `${ns}03`, PROSE + siblingText);
      const initial = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
      const firstBlock = initial.chapter.current_revision.blocks[0]!;
      const split = replaceUnit(PROSE.length, PROSE.length, "", writer, `${ns}07`, id(`${ns}079`));
      split.expected_proposal_head_revision_ids = [];
      split.observed_ownership_partition = "authoritative";
      split.author_edit_units[0]!.normalized_primitives = [{ kind: "split_block",
        manuscript_block_id: firstBlock.manuscript_block_id, offset: PROSE.length,
        new_manuscript_block_id: id(`${ns}077`) }];
      const divided = await sendMixed(started.baseUrl, prepared, split, id(`${ns}076`));
      if (divided.effect.kind !== "authoritative_applied") throw new Error("expected split");
      writer.authoritativeRevisionId = divided.effect.authoritative_revision.revision_id;
      writer.nextSequence = "3";
      const run = await admitPhrase(started.baseUrl, prepared.fetchImpl,
        prepared.projectId, prepared.chapterId, id(`${ns}038`));
      if (run.decision.kind !== "prose_change" || run.decision.opened_proposal.kind !== "present")
        throw new Error("expected Inline Proposal");
      const opened = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: run.decision.opened_proposal.proposal_id, fetchImpl: prepared.fetchImpl });
      if (index === 0) {
        await queryPostgres(`UPDATE storyos.proposal_revisions SET validation='invalid'
          WHERE project_id='${prepared.projectId}'::uuid AND revision_id='${opened.proposal.revision_id}'::uuid`);
        opened.proposal = (await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
          proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl })).proposal;
        assert.equal(opened.proposal.validation, "invalid");
      }
      const options = { baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl };
      const before = await getChapter({ ...options, chapterId: prepared.chapterId });
      const request = replaceUnit(offset, offset, "!", writer, `${ns}04`, opened.proposal.revision_id);
      request.author_edit_units[0]!.normalized_primitives = [{ kind: "replace_block_selection",
        manuscript_block_id: opened.proposal.manuscript_block_id, from: offset, to: offset, text: "!" }];
      const send = async (input: ApplyAuthorEditRequest, key: string) => challenged(
        started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
        "/api/v1/projects/{project_id}/manuscript/author-edits", input.command_schema,
        await digestApplyAuthorEdit(input), key, (antiForgery) => applyAuthorEdit({
          ...options, request: input, idempotencyKey: key, antiForgery }));
      for (const [negativeIndex, shape] of ["wrong_block", "stale_head", "interior"].entries()) {
        const refused = structuredClone(request);
        refused.correlation_id = id(`${ns}05${negativeIndex}3`);
        refused.completed_intent_record_id = id(`${ns}05${negativeIndex}5`);
        const primitive = refused.author_edit_units[0]!.normalized_primitives[0]!;
        assert.ok(primitive.kind === "replace_block_selection");
        if (shape === "wrong_block") primitive.manuscript_block_id = id(`${ns}0577`);
        if (shape === "stale_head") refused.expected_proposal_head_revision_ids = [id(`${ns}0578`)];
        if (shape === "interior") {
          primitive.from = 15; primitive.to = 15;
          refused.author_edit_units[0]!.selection_snapshot.from = 15;
          refused.author_edit_units[0]!.selection_snapshot.to = 15;
        }
        assert.equal((await send(refused, id(`${ns}05${negativeIndex}6`))).effect.kind, "conflicted");
        assert.deepEqual((await getChapter({ ...options, chapterId: prepared.chapterId })).chapter, before.chapter);
        assert.deepEqual((await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal, opened.proposal);
      }
      assert.equal((await send(request, id(`${ns}046`))).effect.kind, "authoritative_applied");
      const after = await getChapter({ ...options, chapterId: prepared.chapterId });
      assert.deepEqual(after.chapter.current_revision.blocks, before.chapter.current_revision.blocks.map((block) => ({
        ...block, text: block.manuscript_block_id === opened.proposal.manuscript_block_id
          ? block.text.slice(0, offset) + "!" + block.text.slice(offset) : block.text })));
      const retained = (await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal;
      assert.equal(retained.revision_id, opened.proposal.revision_id);
      assert.equal(retained.candidate_text, INLINE_CANDIDATE);
      if (index === 0) {
        assert.equal(retained.validation, "invalid");
        if (retained.validation_receipt.kind !== "present") throw new Error("expected receipt");
        const acceptance: AcceptProposalRequest = { command_schema: "storyos.command.accept-proposal.request.v1",
          accept_proposal_input: { ...BINDING, correlation_id: id(`${ns}081`),
            proposal_revision_id: retained.revision_id,
            validation_receipt_id: retained.validation_receipt.validation_receipt_id,
            selected_operation_ids: [retained.operation_id],
            expected_authoritative_revision_id: after.chapter.current_revision.revision_id,
            editor_session_id: writer.session.editor_session.editor_session_id } };
        const result = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
          "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", acceptance.command_schema,
          await digestAcceptProposal(acceptance), id(`${ns}082`), (antiForgery) => acceptProposal({
            ...options, proposalId: retained.proposal_id, request: acceptance, antiForgery,
            idempotencyKey: id(`${ns}082`) }));
        assert.deepEqual(result.effect, { kind: "refused", reason: "not_eligible" });
        assert.deepEqual((await getProposal({ ...options, proposalId: retained.proposal_id })).proposal, retained);
        assert.deepEqual(result.receipt.authoritative_commit_ids, []);
        assert.equal(result.receipt.proposal_revision_id, retained.revision_id);
        assert.deepEqual(result.receipt.resulting_authoritative_revision_ids, [after.chapter.current_revision.revision_id]);
        assert.deepEqual((await getChapter({ ...options, chapterId: prepared.chapterId })).chapter, after.chapter);
      }
    }
  } finally { await stopRealServer(started.server); }
});

function movedDigest(manuscriptBlockId: string, from: number, to: number): string {
  return `sha256:${createHash("sha256").update(JSON.stringify({
    base_slice: SOURCE_SLICE, block_kind: "paragraph",
    coordinate_profile: "prosemirror-token-utf16.v1", from, manuscript_block_id: manuscriptBlockId,
    manuscript_schema_version: 1, to,
  })).digest("hex")}`;
}

async function bindingRows(proposalId: string) {
  return JSON.parse(await queryPostgres(`SELECT jsonb_build_object(${["proposal_revisions", "proposal_anchors"]
    .map((table) => `'${table}', (SELECT coalesce(jsonb_agg(to_jsonb(record) ORDER BY to_jsonb(record)::text),
      '[]'::jsonb) FROM storyos.${table} AS record WHERE record.proposal_id = '${proposalId}'::uuid)`)
    .join(",")})::text`)) as Record<"proposal_revisions" | "proposal_anchors", Record<string, unknown>[]>;
}

async function conflictAndReplan(
  options: { baseUrl: string; projectId: string; fetchImpl: typeof fetch },
  opened: Awaited<ReturnType<typeof getProposal>>,
  writer: Awaited<ReturnType<typeof writePassage>>,
  headRevisionId: string,
  ns: string,
) {
  // An Acceptance on the moved head applies (ADR 0064), so a fixture makes the Proposal Conflict.
  const { conflictRef } = await seedAcceptanceCondition(options.baseUrl, options.fetchImpl, options.projectId,
    opened.proposal, writer.session.editor_session.editor_session_id, headRevisionId, "proposal_conflict", ns);
  const afterConflict = await getProposal({ ...options, proposalId: opened.proposal.proposal_id });
  assert.deepEqual(afterConflict.proposal.source_condition,
    { kind: "proposal_conflict", proposal_conflict_ref: conflictRef });
  const rows = await bindingRows(opened.proposal.proposal_id);
  const request: ReplanProposalRequest = { command_schema: "storyos.command.replan-proposal.request.v1",
    replan_proposal_input: { ...BINDING, correlation_id: id(`${ns}3`),
      conflicted_proposal_revision_id: afterConflict.proposal.revision_id,
      expected_current_proposal_head: afterConflict.proposal.revision_id,
      expected_current_target_revisions: [headRevisionId],
      replacement_operations: [afterConflict.proposal.operation_id],
      source_condition: { kind: "proposal_conflict", proposal_conflict_ref: conflictRef },
      editor_session_id: writer.session.editor_session.editor_session_id } };
  const replanned = await challenged(options.baseUrl, options.fetchImpl, options.projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans", request.command_schema,
    await digestReplanProposal(request), id(`${ns}4`), (antiForgery) => replanProposal({
      ...options, proposalId: opened.proposal.proposal_id, request, antiForgery,
      idempotencyKey: id(`${ns}4`) }));
  return { afterConflict, rows, replanned };
}

test("an Inline Replan proves the moved Anchor, survives restart, and Accepts an edited candidate", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    // An insertion at the right edge keeps the range, and one at the left edge moves it.
    for (const [index, offset] of [24, 10].entries()) {
      const ns = `cd1cd${index}`;
      const prepared = await prepare(started.baseUrl, id(`${ns}011`), "Inline Replan Novel", `${ns}02`);
      const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
        prepared.projectId, prepared.chapterId, ns);
      let options = { baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl };
      const send = async (input: ApplyAuthorEditRequest, key: string) => challenged(
        options.baseUrl, options.fetchImpl, options.projectId, "POST",
        "/api/v1/projects/{project_id}/manuscript/author-edits", input.command_schema,
        await digestApplyAuthorEdit(input), key, (antiForgery) => applyAuthorEdit({
          ...options, request: input, idempotencyKey: key, antiForgery }));
      const edge = replaceUnit(offset, offset, "!", writer, `${ns}4`, opened.proposal.revision_id);
      edge.author_edit_units[0]!.normalized_primitives = [{ kind: "replace_block_selection",
        manuscript_block_id: opened.proposal.manuscript_block_id, from: offset, to: offset, text: "!" }];
      const inserted = await send(edge, id(`${ns}46`));
      if (inserted.effect.kind !== "authoritative_applied") throw new Error("expected edge insertion");
      const head = inserted.effect.authoritative_revision.revision_id;
      writer.authoritativeRevisionId = head;
      writer.nextSequence = "3";
      const { afterConflict, rows, replanned } = await conflictAndReplan(
        options, opened, writer, head, `${ns}8`);
      if (replanned.effect.kind !== "resolved") throw new Error("expected resolved Replan");
      const replannedId = replanned.effect.resulting_proposal_revision_id;
      const from = offset === 10 ? 11 : 10;
      const movedAnchor = { ...opened.proposal.anchors[0]!, base_authoritative_revision_id: head,
        from, to: from + SOURCE_SLICE.length,
        base_slice_digest: movedDigest(opened.proposal.manuscript_block_id, from, from + SOURCE_SLICE.length) };
      const inspected = await getProposal({ ...options, proposalId: opened.proposal.proposal_id });
      assert.equal(inspected.proposal.revision_id, replannedId);
      assert.equal(inspected.proposal.operation_id, afterConflict.proposal.operation_id);
      assert.equal(inspected.proposal.base_authoritative_revision_id, head);
      assert.equal(inspected.proposal.validation, "pending");
      assert.deepEqual(inspected.proposal.anchors, [movedAnchor]);
      // The earlier Revision and its Anchor set stay unchanged, and the Replan adds one of each.
      const bound = await bindingRows(opened.proposal.proposal_id);
      const [oldAnchor] = rows.proposal_anchors;
      assert.equal(rows.proposal_anchors.length, 1);
      assert.deepEqual(bound.proposal_revisions.filter((row) => row.revision_id !== replannedId),
        rows.proposal_revisions);
      assert.deepEqual(bound.proposal_revisions.filter((row) => row.revision_id === replannedId)
        .map((row) => [row.parent_revision_id, row.base_authoritative_revision_id, row.candidate_text]),
      [[afterConflict.proposal.revision_id, head, INLINE_CANDIDATE]]);
      assert.deepEqual(bound.proposal_anchors, [oldAnchor, { ...oldAnchor, proposal_revision_id: replannedId,
        base_authoritative_revision_id: head, range_from: from, range_to: from + SOURCE_SLICE.length,
        base_slice_digest: movedAnchor.base_slice_digest }].sort((left, right) =>
        JSON.stringify(left).localeCompare(JSON.stringify(right))));
      await stopRealServer(started.server);
      const origin = new URL(started.baseUrl);
      started = await startRealServer(`${origin.hostname}:${origin.port}`);
      options = { ...options, baseUrl: started.baseUrl };
      assert.deepEqual((await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal,
        inspected.proposal);
      const interior = replaceUnit(from + 4, from + 7, "xx", writer, `${ns}5`, replannedId);
      const edited = await send(interior, id(`${ns}56`));
      assert.equal(edited.effect.kind, "proposal_revised");
      const revised = (await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal;
      assert.equal(revised.candidate_text, "narrxxr tone");
      assert.equal(revised.validation, "valid");
      assert.deepEqual(revised.anchors, [movedAnchor]);
      if (revised.validation_receipt.kind !== "present") throw new Error("expected receipt");
      const acceptance: AcceptProposalRequest = { command_schema: "storyos.command.accept-proposal.request.v1",
        accept_proposal_input: { ...BINDING, correlation_id: id(`${ns}61`),
          proposal_revision_id: revised.revision_id,
          validation_receipt_id: revised.validation_receipt.validation_receipt_id,
          selected_operation_ids: [revised.operation_id], expected_authoritative_revision_id: head,
          editor_session_id: writer.session.editor_session.editor_session_id } };
      const accepted = await challenged(options.baseUrl, options.fetchImpl, options.projectId, "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", acceptance.command_schema,
        await digestAcceptProposal(acceptance), id(`${ns}62`), (antiForgery) => acceptProposal({
          ...options, proposalId: revised.proposal_id, request: acceptance, antiForgery,
          idempotencyKey: id(`${ns}62`) }));
      assert.equal(accepted.effect.kind, "applied");
      const edged = PROSE.slice(0, offset) + "!" + PROSE.slice(offset);
      assert.equal((await getChapter({ ...options, chapterId: prepared.chapterId })).chapter.current_revision.body,
        edged.slice(0, from) + "narrxxr tone" + edged.slice(from + SOURCE_SLICE.length));
    }
  } finally { await stopRealServer(started.server); }
});

/** Make a new Chapter head with database writes. While the Inline reservation exists, ApplyAuthorEdit
 * accepts in the Anchor Block only a collapsed insertion at an Anchor edge. Thus the public path
 * cannot change the range text or remove the Block identity. */
async function driftHead(projectId: string, chapterId: string, revisionId: string, ns: string,
  change: { text: [string, string] } | { blockId: string }) {
  const next = id(`${ns}71`);
  const payload = id(`${ns}72`);
  const block = id(`${ns}73`);
  const scope = `project_id = '${projectId}'::uuid`;
  const member = "blockId" in change
    ? `CASE WHEN manuscript_block_id = '${change.blockId}'::uuid THEN '${block}'::uuid ELSE manuscript_block_id END`
    : "manuscript_block_id";
  const bytes = "text" in change ? `convert_to(replace(convert_from(canonical_bytes, 'UTF8'),
    '${change.text[0]}', '${change.text[1]}'), 'UTF8')` : "canonical_bytes";
  await queryPostgres(`${"blockId" in change ? `INSERT INTO storyos.manuscript_blocks
      (owner_user_id, project_id, manuscript_block_id, manuscript_object_id, block_kind)
    SELECT owner_user_id, project_id, '${block}'::uuid, manuscript_object_id, block_kind
      FROM storyos.manuscript_blocks WHERE ${scope} AND manuscript_block_id = '${change.blockId}'::uuid;` : ""}
    INSERT INTO storyos.authoritative_payloads (owner_user_id, project_id, payload_id, canonical_bytes)
    SELECT payload.owner_user_id, payload.project_id, '${payload}'::uuid, ${bytes}
      FROM storyos.authoritative_payloads AS payload JOIN storyos.authoritative_revisions AS revision
        USING (owner_user_id, project_id, payload_id)
     WHERE revision.${scope} AND revision.revision_id = '${revisionId}'::uuid;
    INSERT INTO storyos.authoritative_revisions
      (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
    SELECT owner_user_id, project_id, manuscript_object_id, '${next}'::uuid, '${payload}'::uuid
      FROM storyos.authoritative_revisions WHERE ${scope} AND revision_id = '${revisionId}'::uuid;
    INSERT INTO storyos.manuscript_revision_members
      (owner_user_id, project_id, manuscript_object_id, revision_id, manuscript_block_id, block_order)
    SELECT owner_user_id, project_id, manuscript_object_id, '${next}'::uuid, ${member}, block_order
      FROM storyos.manuscript_revision_members WHERE ${scope} AND revision_id = '${revisionId}'::uuid;
    UPDATE storyos.authoritative_heads SET current_revision_id = '${next}'::uuid
     WHERE ${scope} AND manuscript_object_id = '${chapterId}'::uuid`);
  return next;
}

test("an Inline Replan refuses a changed range, changed slice text, or removed Block and keeps the conflict", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    for (const [index, shape] of ["inside", "slice", "block"].entries()) {
      const ns = `cd2ce${index}`;
      const prepared = await prepare(started.baseUrl, id(`${ns}011`), "Inline Replan Refusal", `${ns}02`);
      const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
        prepared.projectId, prepared.chapterId, ns);
      const options = { baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl };
      const head = await driftHead(prepared.projectId, prepared.chapterId, writer.authoritativeRevisionId, ns,
        shape === "inside" ? { text: ["narrator voice", "narrator  voice"] }
          : shape === "slice" ? { text: ["narrator voice", "narrator voicE"] }
            : { blockId: opened.proposal.manuscript_block_id });
      const before = await getChapter({ ...options, chapterId: prepared.chapterId });
      assert.equal(before.chapter.current_revision.revision_id, head);
      const { afterConflict, rows, replanned } = await conflictAndReplan(
        options, opened, writer, head, `${ns}8`);
      assert.deepEqual(replanned.effect, { kind: "refused", reason: "unavailable_proof" });
      assert.deepEqual((await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal,
        afterConflict.proposal);
      assert.deepEqual(await bindingRows(opened.proposal.proposal_id), rows);
      assert.deepEqual((await getChapter({ ...options, chapterId: prepared.chapterId })).chapter, before.chapter);
    }
  } finally { await stopRealServer(started.server); }
});

function undoRequestFor(writer: Awaited<ReturnType<typeof writePassage>>, headRevisionId: string,
  sequence: string, ns: string) {
  return { command_schema: "storyos.command.undo-latest-author-action.request.v1" as const,
    undo_latest_author_action_input: { ...BINDING, correlation_id: id(`${ns}71`),
      editor_session_id: writer.session.editor_session.editor_session_id,
      expected_authoritative_revision_id: headRevisionId,
      expected_author_undo_frontier_sequence: sequence } };
}

test("Undo of an Inline Replan restores the Anchor set of the old base, and the candidate stays editable", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    for (const [index, path] of ["explicit", "replan", "explicit_undo"].entries()) {
      const ns = `dc1dc${index}`;
      const prepared = await prepare(started.baseUrl, id(`${ns}011`), "Inline Replan Undo", `${ns}02`);
      const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
        prepared.projectId, prepared.chapterId, ns);
      const options = { baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl };
      const send = async (input: ApplyAuthorEditRequest, key: string) => challenged(
        options.baseUrl, options.fetchImpl, options.projectId, "POST",
        "/api/v1/projects/{project_id}/manuscript/author-edits", input.command_schema,
        await digestApplyAuthorEdit(input), key, (antiForgery) => applyAuthorEdit({
          ...options, request: input, idempotencyKey: key, antiForgery }));
      const edge = replaceUnit(24, 24, "!", writer, `${ns}4`, opened.proposal.revision_id);
      edge.author_edit_units[0]!.normalized_primitives = [{ kind: "replace_block_selection",
        manuscript_block_id: opened.proposal.manuscript_block_id, from: 24, to: 24, text: "!" }];
      const inserted = await send(edge, id(`${ns}46`));
      if (inserted.effect.kind !== "authoritative_applied") throw new Error("expected edge insertion");
      const head = inserted.effect.authoritative_revision.revision_id;
      writer.authoritativeRevisionId = head;
      writer.nextSequence = "3";
      const { afterConflict, rows: conflictRows, replanned } = await conflictAndReplan(
        options, opened, writer, head, `${ns}8`);
      if (replanned.effect.kind !== "resolved") throw new Error("expected resolved Replan");
      const beforeUndo = await bindingRows(opened.proposal.proposal_id);
      const undo = undoRequestFor(writer, head, replanned.effect.author_action_sequence, ns);
      const sendUndo = async (key: string) => challenged(options.baseUrl, options.fetchImpl, options.projectId,
        "POST", "/api/v1/projects/{project_id}/author-actions/undo", undo.command_schema,
        await digestUndoLatestAuthorAction(undo), key, (antiForgery) => undoLatestAuthorAction({
          ...options, request: undo, antiForgery, idempotencyKey: key }));
      const undone = await sendUndo(id(`${ns}72`));
      assert.equal(undone.effect.kind, "compensated");
      assert.deepEqual(await sendUndo(id(`${ns}72`)), undone);
      const restored = await getProposal({ ...options, proposalId: opened.proposal.proposal_id });
      assert.equal(restored.proposal.revision_id, undone.proposal_revision_id);
      assert.equal(restored.proposal.operation_id, afterConflict.proposal.operation_id);
      assert.equal(restored.proposal.base_authoritative_revision_id,
        opened.proposal.base_authoritative_revision_id);
      assert.equal(restored.proposal.validation, "valid");
      assert.deepEqual(restored.proposal.anchors, afterConflict.proposal.anchors);
      // The Undo adds one copy of the restored Anchor set, and the earlier rows stay unchanged.
      const bound = await bindingRows(opened.proposal.proposal_id);
      const restoredId = restored.proposal.revision_id;
      assert.deepEqual(bound.proposal_anchors.filter((row) => row.proposal_revision_id !== restoredId),
        beforeUndo.proposal_anchors);
      assert.deepEqual(bound.proposal_anchors.filter((row) => row.proposal_revision_id === restoredId),
        conflictRows.proposal_anchors.map((row) => ({ ...row, proposal_revision_id: restoredId })));
      if (path !== "replan") {
        // The explicit candidate edit and its Undo validate against the old base, and Acceptance applies.
        const explicit = replaceUnit(5, 5, "xx", writer, `${ns}5`, restoredId);
        explicit.proposal_target = { proposal_id: restored.proposal.proposal_id,
          operation_id: restored.proposal.operation_id, revision_id: restoredId,
          manuscript_block_id: restored.proposal.manuscript_block_id };
        const edited = await send(explicit, id(`${ns}56`));
        if (edited.effect.kind !== "proposal_revised") throw new Error("expected proposal_revised");
        const targeted = (await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal;
        assert.equal(targeted.candidate_text, "narraxxtor tone");
        assert.equal(targeted.validation, "valid");
        assert.deepEqual(targeted.anchors, afterConflict.proposal.anchors);
        const acceptApplied = async (proposal: typeof targeted, candidate: string) => {
          const accepted = await acceptInline(options, writer, proposal, head, `${ns}e`);
          if (accepted.effect.kind !== "applied") throw new Error(`expected applied, got ${JSON.stringify(accepted.effect)}`);
          assert.deepEqual([accepted.effect.target_resolution, accepted.effect.authoritative_revision.body],
            ["proven_range", `${PROSE.slice(0, 10)}${candidate}!${PROSE.slice(24)}`]);
        };
        if (path === "explicit") {
          await acceptApplied(targeted, "narraxxtor tone");
          continue;
        }
        const editUndo = undoRequestFor(writer, head, edited.effect.author_action_sequence, `${ns}6`);
        const editUndone = await challenged(options.baseUrl, options.fetchImpl, options.projectId, "POST",
          "/api/v1/projects/{project_id}/author-actions/undo", editUndo.command_schema,
          await digestUndoLatestAuthorAction(editUndo), id(`${ns}672`), (antiForgery) => undoLatestAuthorAction({
            ...options, request: editUndo, antiForgery, idempotencyKey: id(`${ns}672`) }));
        assert.equal(editUndone.effect.kind, "compensated");
        const reverted = (await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal;
        assert.equal(reverted.revision_id, editUndone.proposal_revision_id);
        assert.deepEqual([reverted.candidate_text, reverted.validation, reverted.base_authoritative_revision_id],
          [restored.proposal.candidate_text, "valid", opened.proposal.base_authoritative_revision_id]);
        await acceptApplied(reverted, restored.proposal.candidate_text);
        continue;
      }
      const again = await conflictAndReplan(options, restored, writer, head, `${ns}9`);
      if (again.replanned.effect.kind !== "resolved") throw new Error("expected a second resolved Replan");
      const interior = replaceUnit(14, 17, "yy", writer, `${ns}6`,
        again.replanned.effect.resulting_proposal_revision_id);
      assert.equal((await send(interior, id(`${ns}66`))).effect.kind, "proposal_revised");
      const revised = (await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal;
      assert.equal(revised.candidate_text, "narryyr tone");
      assert.equal(revised.validation, "valid");
      assert.deepEqual(revised.anchors.map((anchor) => [anchor.base_authoritative_revision_id, anchor.from, anchor.to]),
        [[head, 10, 24]]);
    }
  } finally { await stopRealServer(started.server); }
});

test("Undo of an Inline Acceptance in Reopen mode gives an editable Inline Proposal on the restored base", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = "dc2db0";
    const prepared = await prepare(started.baseUrl, id(`${ns}011`), "Inline Reopen Undo", `${ns}02`);
    const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl,
      prepared.projectId, prepared.chapterId, ns);
    const options = { baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl };
    const accept = async (proposal: typeof opened.proposal, expected: string, key: string) => {
      if (proposal.validation_receipt.kind !== "present") throw new Error("expected receipt");
      const acceptance: AcceptProposalRequest = { command_schema: "storyos.command.accept-proposal.request.v1",
        accept_proposal_input: { ...BINDING, correlation_id: id(`${key}1`), proposal_revision_id: proposal.revision_id,
          validation_receipt_id: proposal.validation_receipt.validation_receipt_id,
          selected_operation_ids: [proposal.operation_id], expected_authoritative_revision_id: expected,
          editor_session_id: writer.session.editor_session.editor_session_id } };
      return challenged(options.baseUrl, options.fetchImpl, options.projectId, "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", acceptance.command_schema,
        await digestAcceptProposal(acceptance), id(`${key}2`), (antiForgery) => acceptProposal({
          ...options, proposalId: proposal.proposal_id, request: acceptance, antiForgery, idempotencyKey: id(`${key}2`) }));
    };
    const accepted = await accept(opened.proposal, writer.authoritativeRevisionId, `${ns}8`);
    if (accepted.effect.kind !== "applied") throw new Error("expected applied Acceptance");
    const head = accepted.effect.authoritative_revision.revision_id;
    const undo = undoRequestFor(writer, head, accepted.effect.author_action_sequence, ns);
    const sendUndo = async (key: string) => challenged(options.baseUrl, options.fetchImpl, options.projectId, "POST",
      "/api/v1/projects/{project_id}/author-actions/undo", undo.command_schema, await digestUndoLatestAuthorAction(undo),
      key, (antiForgery) => undoLatestAuthorAction({ ...options, request: undo, antiForgery, idempotencyKey: key }));
    const undone = await sendUndo(id(`${ns}72`));
    assert.equal(undone.effect.kind, "compensated");
    assert.deepEqual(await sendUndo(id(`${ns}72`)), undone);
    const restoredChapter = (await getChapter({ ...options, chapterId: prepared.chapterId })).chapter;
    assert.equal(restoredChapter.current_revision.body, PROSE);
    const restoredHead = restoredChapter.current_revision.revision_id;
    assert.notEqual(restoredHead, head);
    const reopened = (await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal;
    assert.equal(reopened.operation_id, opened.proposal.operation_id);
    assert.equal(reopened.base_authoritative_revision_id, restoredHead);
    assert.equal(reopened.validation, "valid");
    assert.deepEqual(reopened.anchors, [{ ...opened.proposal.anchors[0]!, base_authoritative_revision_id: restoredHead }]);
    writer.authoritativeRevisionId = restoredHead;
    const interior = replaceUnit(14, 17, "xx", writer, `${ns}5`, reopened.revision_id);
    assert.equal((await challenged(options.baseUrl, options.fetchImpl, options.projectId, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits", interior.command_schema,
      await digestApplyAuthorEdit(interior), id(`${ns}56`), (antiForgery) => applyAuthorEdit({
        ...options, request: interior, idempotencyKey: id(`${ns}56`), antiForgery }))).effect.kind, "proposal_revised");
    const revised = (await getProposal({ ...options, proposalId: opened.proposal.proposal_id })).proposal;
    assert.equal(revised.candidate_text, "narrxxr tone");
    assert.equal(revised.validation, "valid");
    assert.equal((await accept(revised, restoredHead, `${ns}9`)).effect.kind, "applied");
    assert.equal((await getChapter({ ...options, chapterId: prepared.chapterId })).chapter.current_revision.body,
      "Guard the narrxxr tone in this passage.");
  } finally { await stopRealServer(started.server); }
});
