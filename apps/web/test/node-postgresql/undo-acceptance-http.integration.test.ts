// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/accept-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { acceptProposal, digestAcceptProposal, digestUndoLatestAuthorAction, getChapter, getProposal,
  undoLatestAuthorAction } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, UndoLatestAuthorActionRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, requireStoryOSProtocolError,
  sessionFetch as browserFetch, stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { USER_A, UUID_V7, BINDING, REVISED, id, startRealServer, drainLeftoverWork, challenged,
  prepare, admitProse, reviseCandidate } from "../support/acceptance.ts";

const undoRoute = "/api/v1/projects/{project_id}/author-actions/undo";
const acceptRoute = "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances";

test("undo acceptance restores the prior chapter, reopens the proposal, and reapply is a fresh acceptance", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e151"), "Undo Acceptance Novel", "e2");
    const accepted = await acceptOpened(started.baseUrl, prepared, "ea");
    const undoRequest = undoBody(accepted.sessionId, accepted.frontier, accepted.chapterRevision, id("e156"));
    const undoOptions = {
      baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("e157"), antiForgery: "", request: undoRequest,
    };
    const undone = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      undoRoute, undoRequest.command_schema, await digestUndoLatestAuthorAction(undoRequest), id("e157"),
      (antiForgery) => { undoOptions.antiForgery = antiForgery; return undoLatestAuthorAction(undoOptions); });
    assert.equal(undone.effect.kind, "compensated");
    if (undone.effect.kind !== "compensated") throw new Error("expected compensated");
    const { authoritative_revision: restored, authoritative_commit_id: commitId } = undone.effect;
    if (restored === undefined || commitId === undefined) throw new Error("expected the restored Revision");
    assert.notEqual(restored.revision_id, accepted.chapterRevision);
    assert.match(commitId, UUID_V7);
    assert.equal(undone.proposal_id, accepted.proposalId);
    assert.deepEqual(await undoLatestAuthorAction(undoOptions), undone);
    const chapter = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    assert.equal(chapter.chapter.current_revision.body, accepted.beforeBody);
    assert.equal(chapter.chapter.current_revision.revision_id, restored.revision_id);
    assert.equal(await queryPostgres(`SELECT count(*) FROM storyos.authoritative_revisions
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${prepared.projectId}'::uuid
        AND revision_id = '${accepted.chapterRevision}'::uuid`), "1");
    assert.equal(await queryPostgres(`SELECT count(*) FROM storyos.authoritative_commits
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${prepared.projectId}'::uuid
        AND authoritative_commit_id = '${accepted.commitId}'::uuid`), "1");
    const reopened = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: accepted.proposalId, fetchImpl: prepared.fetchImpl });
    assert.equal(reopened.proposal.operation_resolution, "pending");
    assert.equal(reopened.proposal.reservation_state, "unresolved");
    assert.equal(reopened.proposal.candidate_text, REVISED);
    assert.notEqual(reopened.proposal.revision_id, accepted.proposalRevisionId);
    assert.equal(reopened.proposal.validation_receipt.kind, "present");
    if (reopened.proposal.validation_receipt.kind !== "present") throw new Error("expected receipt");
    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    const foreignDigest = await digestUndoLatestAuthorAction(undoRequest);
    await assert.rejects(() => challenged(started.baseUrl, foreignFetch, prepared.projectId, "POST",
      undoRoute, undoRequest.command_schema, foreignDigest, id("e158"),
      (antiForgery) => undoLatestAuthorAction({ ...undoOptions, fetchImpl: foreignFetch,
        idempotencyKey: id("e158"), antiForgery })),
    (error: unknown) => requireStoryOSProtocolError(error).status === 404
      && !String(requireStoryOSProtocolError(error).responseBody).includes(USER_A));
    const reapplied = await sendAccept(started.baseUrl, prepared, {
      proposalId: accepted.proposalId, proposalRevisionId: reopened.proposal.revision_id,
      validationReceiptId: reopened.proposal.validation_receipt.validation_receipt_id,
      operationId: reopened.proposal.operation_id, expectedHead: chapter.chapter.current_revision.revision_id,
      sessionId: accepted.sessionId, correlationId: id("e159"), idempotencyKey: id("e15a"),
    });
    assert.equal(reapplied.effect.kind, "applied");
    if (reapplied.effect.kind !== "applied") throw new Error("expected applied");
    assert.equal(reapplied.effect.authoritative_revision.body, REVISED);
    assert.notEqual(reapplied.receipt.receipt_id, accepted.receiptId);
  } finally {
    await stopRealServer(started.server);
  }
});

test("undo acceptance derives a proposal when the accepted lineage has drifted", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e171"), "Undo Acceptance Drift Novel", "e3");
    const accepted = await acceptOpened(started.baseUrl, prepared, "eb");
    await queryPostgres(`UPDATE storyos.proposal_revisions SET closure = 'withdrawn'
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${prepared.projectId}'::uuid
        AND revision_id = '${accepted.proposalRevisionId}'::uuid`);
    const undone = await sendUndo(started.baseUrl, prepared, accepted.sessionId, accepted.frontier,
      accepted.chapterRevision, id("e176"), id("e177"));
    assert.equal(undone.effect.kind, "compensated");
    assert.equal(typeof undone.proposal_id, "string");
    assert.notEqual(undone.proposal_id, accepted.proposalId);
    const chapter = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    assert.equal(chapter.chapter.current_revision.body, accepted.beforeBody);
    const original = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: accepted.proposalId, fetchImpl: prepared.fetchImpl });
    assert.equal(original.proposal.operation_resolution, "applied");
    const derived = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: undone.proposal_id!, fetchImpl: prepared.fetchImpl });
    assert.equal(derived.proposal.operation_resolution, "pending");
    assert.equal(derived.proposal.reservation_state, "unresolved");
    assert.equal(derived.proposal.candidate_text, REVISED);
    assert.notEqual(derived.proposal.proposal_id, accepted.proposalId);
  } finally {
    await stopRealServer(started.server);
  }
});

test("undo acceptance requires a reversal proposal when the authoritative head has drifted", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e1c1"), "Undo Acceptance Reversal Novel", "e4");
    const accepted = await acceptOpened(started.baseUrl, prepared, "ec");
    await queryPostgres(`UPDATE storyos.authoritative_heads SET current_revision_id = '${accepted.beforeRevision}'::uuid
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${prepared.projectId}'::uuid
        AND manuscript_object_id = '${prepared.chapterId}'::uuid
        AND current_revision_id = '${accepted.chapterRevision}'::uuid`);
    const undone = await sendUndo(started.baseUrl, prepared, accepted.sessionId, accepted.frontier,
      accepted.beforeRevision, id("e196"), id("e197"));
    assert.equal(undone.effect.kind, "reversal_required");
    assert.equal(undone.receipt.result, "proposal_revised");
    if (undone.effect.kind !== "reversal_required") throw new Error("expected reversal");
    assert.equal(undone.effect.source_sequence, accepted.frontier);
    assert.equal(undone.proposal_id, undone.effect.proposal_id);
    assert.deepEqual(await undoLatestAuthorAction({
      baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("e197"), antiForgery: undone.nonce, request: undone.request,
    }), undone.response);
    const chapter = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    assert.equal(chapter.chapter.current_revision.revision_id, accepted.beforeRevision);
    assert.equal(await queryPostgres(`SELECT count(*) FROM storyos.author_action_entries
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${prepared.projectId}'::uuid
        AND compensated_source_sequence = ${accepted.frontier}
        AND disposition = 'compensation'`), "0");
    assert.equal(await queryPostgres(`SELECT count(*) FROM storyos.authoritative_revisions
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${prepared.projectId}'::uuid
        AND revision_id = '${accepted.chapterRevision}'::uuid`), "1");
    const reversal = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: undone.effect.proposal_id, fetchImpl: prepared.fetchImpl });
    assert.equal(reversal.proposal.kind, "reversal");
    assert.equal(reversal.proposal.operation_resolution, "pending");
    assert.equal(reversal.proposal.candidate_text, accepted.beforeBody);
  } finally {
    await stopRealServer(started.server);
  }
});

test("undo acceptance is unavailable when the accepted payload digest does not match", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e1b1"), "Undo Acceptance Unavailable Novel", "e5");
    const accepted = await acceptOpened(started.baseUrl, prepared, "ed");
    await queryPostgres(`UPDATE storyos.authoritative_revision_envelopes
      SET payload_digest = '0000000000000000000000000000000000000000000000000000000000000000'
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${prepared.projectId}'::uuid
        AND revision_id = '${accepted.chapterRevision}'::uuid`);
    const undone = await sendUndo(started.baseUrl, prepared, accepted.sessionId, accepted.frontier,
      accepted.chapterRevision, id("e1b6"), id("e1b7"));
    assert.equal(undone.effect.kind, "unavailable");
    if (undone.effect.kind !== "unavailable") throw new Error("expected unavailable");
    assert.equal(undone.effect.reason, "source_unavailable");
    assert.equal(undone.receipt.result, "refused");
    const chapter = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    assert.equal(chapter.chapter.current_revision.revision_id, accepted.chapterRevision);
    assert.equal(await queryPostgres(`SELECT count(*) FROM storyos.author_action_entries
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${prepared.projectId}'::uuid
        AND compensated_source_sequence = ${accepted.frontier}
        AND disposition = 'compensation'`), "0");
  } finally {
    await stopRealServer(started.server);
  }
});

async function acceptOpened(baseUrl: string, prepared: Awaited<ReturnType<typeof prepare>>, ns: string) {
  const before = await getChapter({ baseUrl, projectId: prepared.projectId, chapterId: prepared.chapterId,
    fetchImpl: prepared.fetchImpl });
  const queried = await admitProse(baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id(`${ns}8`));
  if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
    throw new Error("expected an opened prose proposal");
  }
  const opened = await getProposal({ baseUrl, projectId: prepared.projectId,
    proposalId: queried.decision.opened_proposal.proposal_id, fetchImpl: prepared.fetchImpl });
  const { session, revised } = await reviseCandidate(baseUrl, prepared.fetchImpl, prepared.projectId, opened, ns);
  if (revised.proposal.validation_receipt.kind !== "present") throw new Error("expected receipt");
  const accepted = await sendAccept(baseUrl, prepared, {
    proposalId: revised.proposal.proposal_id, proposalRevisionId: revised.proposal.revision_id,
    validationReceiptId: revised.proposal.validation_receipt.validation_receipt_id,
    operationId: revised.proposal.operation_id, expectedHead: before.chapter.current_revision.revision_id,
    sessionId: session.editor_session.editor_session_id, correlationId: id(`${ns}9`), idempotencyKey: id(`${ns}a`),
  });
  if (accepted.effect.kind !== "applied") throw new Error("expected applied");
  return {
    sessionId: session.editor_session.editor_session_id,
    frontier: accepted.effect.author_action_sequence,
    chapterRevision: accepted.effect.authoritative_revision.revision_id,
    commitId: accepted.effect.authoritative_commit_id,
    receiptId: accepted.receipt.receipt_id,
    proposalId: revised.proposal.proposal_id,
    proposalRevisionId: revised.proposal.revision_id,
    beforeBody: before.chapter.current_revision.body,
    beforeRevision: before.chapter.current_revision.revision_id,
  };
}

async function sendAccept(baseUrl: string, prepared: Awaited<ReturnType<typeof prepare>>, input: {
  proposalId: string; proposalRevisionId: string; validationReceiptId: string; operationId: string;
  expectedHead: string; sessionId: string; correlationId: string; idempotencyKey: string;
}) {
  const request: AcceptProposalRequest = {
    command_schema: "storyos.command.accept-proposal.request.v1",
    accept_proposal_input: {
      proposal_revision_id: input.proposalRevisionId,
      validation_receipt_id: input.validationReceiptId,
      selected_operation_ids: [input.operationId],
      expected_authoritative_revision_id: input.expectedHead,
      editor_session_id: input.sessionId,
      ...BINDING,
      correlation_id: input.correlationId,
    },
  };
  return challenged(baseUrl, prepared.fetchImpl, prepared.projectId, "POST", acceptRoute,
    request.command_schema, await digestAcceptProposal(request), input.idempotencyKey,
    (antiForgery) => acceptProposal({
      baseUrl, projectId: prepared.projectId, proposalId: input.proposalId, fetchImpl: prepared.fetchImpl,
      idempotencyKey: input.idempotencyKey, antiForgery, request,
    }));
}

function undoBody(sessionId: string, frontier: string, expectedHead: string, correlationId: string): UndoLatestAuthorActionRequest {
  return {
    command_schema: "storyos.command.undo-latest-author-action.request.v1",
    undo_latest_author_action_input: {
      expected_author_undo_frontier_sequence: frontier,
      expected_authoritative_revision_id: expectedHead,
      editor_session_id: sessionId,
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

async function sendUndo(baseUrl: string, prepared: Awaited<ReturnType<typeof prepare>>, sessionId: string,
  frontier: string, expectedHead: string, correlationId: string, idempotencyKey: string) {
  const request = undoBody(sessionId, frontier, expectedHead, correlationId);
  const options = { baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
    idempotencyKey, antiForgery: "", request };
  const response = await challenged(baseUrl, prepared.fetchImpl, prepared.projectId, "POST", undoRoute,
    request.command_schema, await digestUndoLatestAuthorAction(request), idempotencyKey,
    (antiForgery) => { options.antiForgery = antiForgery; return undoLatestAuthorAction(options); });
  return { ...response, request, nonce: options.antiForgery, response };
}
