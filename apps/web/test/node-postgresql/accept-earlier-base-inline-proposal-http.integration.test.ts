// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/accept-earlier-base-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { acceptProposal, applyAuthorEdit, digestAcceptProposal, digestApplyAuthorEdit, digestExportProjectArchive,
  digestUndoLatestAuthorAction, exportProjectArchive, getChapter, getExportOperation, getProposal, undoLatestAuthorAction,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, AcceptProposalResponse, BlockProposalInspect, UndoLatestAuthorActionRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { BINDING, advanceChapterHead, challenged, drainLeftoverWork, id, prepare, settleOnce,
  startRealServer } from "../support/acceptance.ts";
import { admitPhrase, openInline, replaceUnit } from "../support/inline-proposal.ts";

type Started = Awaited<ReturnType<typeof startRealServer>>;
type Context = { started: Started; projectId: string; chapterId: string; fetchImpl: typeof fetch; sessionId: string };

/** An Inline Proposal on "narrator voice" (range 10..24) of the Block on the Chapter Head H0. */
async function openedInline(started: Started, ns: string) {
  const prepared = await prepare(started.baseUrl, id(`${ns}011`), "Inline Earlier Base Novel", `${ns}02`);
  const { opened, writer } = await openInline(started.baseUrl, prepared.fetchImpl, prepared.projectId,
    prepared.chapterId, `${ns}03`);
  const context: Context = { started, projectId: prepared.projectId, chapterId: prepared.chapterId,
    fetchImpl: prepared.fetchImpl, sessionId: writer.session.editor_session.editor_session_id };
  return { context, opened: opened.proposal, writer, h0: writer.authoritativeRevisionId };
}

async function accept(context: Context, proposal: BlockProposalInspect, expectedRevisionId: string, ns: string) {
  if (proposal.validation_receipt.kind !== "present") throw new Error("expected a Validation Receipt");
  const request: AcceptProposalRequest = { command_schema: "storyos.command.accept-proposal.request.v1",
    accept_proposal_input: { proposal_revision_id: proposal.revision_id,
      validation_receipt_id: proposal.validation_receipt.validation_receipt_id,
      selected_operation_ids: [proposal.operation_id], expected_authoritative_revision_id: expectedRevisionId,
      editor_session_id: context.sessionId, ...BINDING, correlation_id: id(`${ns}1`) } };
  const { started, projectId, fetchImpl } = context;
  return challenged(started.baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", request.command_schema,
    await digestAcceptProposal(request), id(`${ns}2`), (antiForgery) => acceptProposal({ baseUrl: started.baseUrl,
      projectId, proposalId: proposal.proposal_id, fetchImpl, request, antiForgery, idempotencyKey: id(`${ns}2`) }));
}

function applied(response: AcceptProposalResponse) {
  if (response.effect.kind !== "applied") throw new Error(`expected applied, got ${JSON.stringify(response.effect)}`);
  return response.effect;
}

async function undoAcceptance(context: Context, accepted: ReturnType<typeof applied>, ns: string) {
  const request: UndoLatestAuthorActionRequest = { command_schema: "storyos.command.undo-latest-author-action.request.v1",
    undo_latest_author_action_input: { expected_author_undo_frontier_sequence: accepted.author_action_sequence,
      expected_authoritative_revision_id: accepted.authoritative_revision.revision_id,
      editor_session_id: context.sessionId, ...BINDING, correlation_id: id(`${ns}1`) } };
  const { started, projectId, fetchImpl } = context;
  const undone = await challenged(started.baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/author-actions/undo", request.command_schema,
    await digestUndoLatestAuthorAction(request), id(`${ns}2`), (antiForgery) => undoLatestAuthorAction({
      baseUrl: started.baseUrl, projectId, fetchImpl, request, antiForgery, idempotencyKey: id(`${ns}2`) }));
  if (undone.effect.kind !== "compensated" || undone.effect.authoritative_revision === undefined) {
    throw new Error(`expected compensated, got ${JSON.stringify(undone.effect)}`);
  }
  return undone.effect.authoritative_revision;
}

const read = (context: Context, proposalId: string) => getProposal({ baseUrl: context.started.baseUrl,
  projectId: context.projectId, proposalId, fetchImpl: context.fetchImpl }).then((result) => result.proposal);

const chapterOf = (context: Context) => getChapter({ baseUrl: context.started.baseUrl, projectId: context.projectId,
  chapterId: context.chapterId, fetchImpl: context.fetchImpl }).then((result) => result.chapter);

async function storedResolution(receiptId: string) {
  return JSON.parse(await queryPostgres(`SELECT jsonb_build_object('applied', applied_authoritative_revision_id,
    'resolution', target_resolution)::text FROM storyos.acceptance_receipts
    WHERE acceptance_receipt_id = '${receiptId}'::uuid`));
}

/** The Acceptance rows of a Project archive export, as receipt, applied head, and target resolution. */
async function archivedAcceptances(context: Context, ns: string) {
  const { started, projectId, fetchImpl } = context;
  const request = { command_schema: "storyos.command.export-project-archive.request.v1" as const,
    export_project_archive_input: { ...BINDING, correlation_id: id(`${ns}1`), archive_profile: "storyos.project-export.v1",
      archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" } };
  const archive = await challenged(started.baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/exports",
    request.command_schema, await digestExportProjectArchive(request), id(`${ns}2`), (antiForgery) =>
      exportProjectArchive({ baseUrl: started.baseUrl, projectId, fetchImpl, request, idempotencyKey: id(`${ns}2`), antiForgery }));
  if (archive.effect.kind !== "admitted") throw new Error("expected admitted archive");
  await settleOnce();
  assert.equal((await getExportOperation({ baseUrl: started.baseUrl, projectId, fetchImpl,
    exportId: archive.effect.export_id })).status, "ready");
  const download = await fetchImpl(`${started.baseUrl}/api/v1/projects/${projectId}/exports/${archive.effect.export_id}`,
    { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
  const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
  return (JSON.parse(new TextDecoder().decode(files.get("canonical/acceptance_receipts.json"))) as Record<string, unknown>[])
    .map((row) => [row.acceptance_receipt_id, row.applied_authoritative_revision_id, row.target_resolution]);
}

/** Author Undo reopens the Proposal on its own base with its Anchor set, and the same rule accepts it again. */
async function reopenedOnItsBase(context: Context, before: BlockProposalInspect, restoredRevisionId: string,
  resolution: string, body: string, ns: string) {
  const reopened = await read(context, before.proposal_id);
  assert.equal(reopened.operation_resolution, "pending");
  assert.equal(reopened.base_authoritative_revision_id, before.base_authoritative_revision_id);
  assert.deepEqual(reopened.anchors, before.anchors);
  const again = applied(await accept(context, reopened, restoredRevisionId, ns));
  assert.equal(again.target_resolution, resolution);
  assert.equal(again.authoritative_revision.body, body);
}

test("an author edit outside the Inline range keeps the proven range, and Author Undo restores the author text", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const { context, opened, writer, h0 } = await openedInline(started, "d31");
    const edge = replaceUnit(0, 5, "Keep", writer, "d3104", opened.revision_id);
    const edited = await challenged(started.baseUrl, context.fetchImpl, context.projectId, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits", edge.command_schema, await digestApplyAuthorEdit(edge),
      id("d31046"), (antiForgery) => applyAuthorEdit({ baseUrl: started.baseUrl, projectId: context.projectId,
        fetchImpl: context.fetchImpl, request: edge, antiForgery, idempotencyKey: id("d31046") }));
    if (edited.effect.kind !== "authoritative_applied") throw new Error("expected the author edit to apply");
    const h1 = edited.effect.authoritative_revision.revision_id;
    const before = await read(context, opened.proposal_id);
    assert.equal(before.base_authoritative_revision_id, h0);

    const response = await accept(context, before, h1, "d3105");
    const accepted = applied(response);
    assert.equal(accepted.target_resolution, "proven_range");
    assert.equal(accepted.authoritative_revision.body, "Keep the narrator tone in this passage.");
    assert.deepEqual(await storedResolution(response.receipt.receipt_id), { applied: h1, resolution: "proven_range" });

    await stopRealServer(started.server);
    started = await startRealServer();
    Object.assign(context, { started, fetchImpl: browserFetch(started.baseUrl, "session-a") });
    assert.equal((await chapterOf(context)).current_revision.body, "Keep the narrator tone in this passage.");
    assert.equal((await read(context, opened.proposal_id)).operation_resolution, "applied");
    assert.deepEqual(await archivedAcceptances(context, "d3106"), [[response.receipt.receipt_id, h1, "proven_range"]]);

    const restored = await undoAcceptance(context, accepted, "d3107");
    assert.equal(restored.body, "Keep the narrator voice in this passage.");
    await reopenedOnItsBase(context, before, restored.revision_id, "proven_range",
      "Keep the narrator tone in this passage.", "d3108");
  } finally {
    await stopRealServer(started.server);
  }
});

test("a changed Inline range gives the Anchor base Block text with the candidate, and Author Undo restores the author text", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const { context, opened, h0 } = await openedInline(started, "d32");
    const h1 = id("d32051");
    await advanceChapterHead(context.projectId, context.chapterId, h0, h1,
      [{ manuscript_block_id: opened.manuscript_block_id, text: "Guard the narrator's voice in this passage." }]);

    const response = await accept(context, opened, h1, "d3205");
    const accepted = applied(response);
    assert.equal(accepted.target_resolution, "base_block_replacement");
    assert.equal(accepted.authoritative_revision.body, "Guard the narrator tone in this passage.");
    assert.deepEqual(await storedResolution(response.receipt.receipt_id), { applied: h1, resolution: "base_block_replacement" });
    assert.equal((await read(context, opened.proposal_id)).operation_resolution, "applied");
    assert.deepEqual(await archivedAcceptances(context, "d3206"), [[response.receipt.receipt_id, h1, "base_block_replacement"]]);

    const restored = await undoAcceptance(context, accepted, "d3207");
    assert.equal(restored.body, "Guard the narrator's voice in this passage.");
    await reopenedOnItsBase(context, opened, restored.revision_id, "base_block_replacement",
      "Guard the narrator tone in this passage.", "d3208");
  } finally {
    await stopRealServer(started.server);
  }
});

test("Acceptance refuses an Inline Proposal whose target Block was deleted and changes nothing", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const { context, opened, h0 } = await openedInline(started, "d34");
    const h1 = id("d34051");
    await advanceChapterHead(context.projectId, context.chapterId, h0, h1,
      [{ manuscript_block_id: id("d34052"), text: "The author replaced the paragraph." }]);
    const chapter = await chapterOf(context);
    const response = await accept(context, opened, h1, "d3405");
    assert.deepEqual(response.effect, { kind: "refused", reason: "missing_target" });
    assert.deepEqual(await chapterOf(context), chapter);
    assert.deepEqual(await read(context, opened.proposal_id), opened);
  } finally {
    await stopRealServer(started.server);
  }
});

test("a second Proposal request on the Block of an open Inline Proposal opens no Proposal, and the database refuses a second reservation", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const { context, opened } = await openedInline(started, "f5c1");
    const chapter = await chapterOf(context);
    const second = await admitPhrase(started.baseUrl, context.fetchImpl, context.projectId, context.chapterId,
      id("f5c10061"));
    if (second.decision.kind !== "prose_change") throw new Error(`expected prose_change, got ${second.decision.kind}`);
    assert.equal(second.status, "completed");
    assert.deepEqual([second.decision.opened_proposal, second.decision.locations], [{ kind: "absent" }, undefined]);
    const reservations = `SELECT count(*)::text FROM storyos.proposal_operations
      WHERE project_id = '${context.projectId}'::uuid AND manuscript_block_id = '${opened.manuscript_block_id}'::uuid
        AND reservation_state = 'unresolved'`;
    assert.equal(await queryPostgres(reservations), "1");
    // The foreign keys wait, so that only the unique index can refuse the copy of the reservation.
    await assert.rejects(queryPostgres(`BEGIN; SET LOCAL session_replication_role = replica;
      INSERT INTO storyos.proposal_operations SELECT (jsonb_populate_record(operation, jsonb_build_object(
        'proposal_id', '${id("f5c10071")}', 'operation_id', '${id("f5c10072")}'))).*
        FROM storyos.proposal_operations AS operation
       WHERE operation.project_id = '${context.projectId}'::uuid AND operation.proposal_id = '${opened.proposal_id}'::uuid;
      COMMIT;`), /duplicate key value violates unique constraint "proposal_operations_one_unresolved_block"/);
    assert.equal(await queryPostgres(reservations), "1");
    assert.deepEqual(await read(context, opened.proposal_id), opened);
    assert.deepEqual(await chapterOf(context), chapter);
  } finally {
    await stopRealServer(started.server);
  }
});
