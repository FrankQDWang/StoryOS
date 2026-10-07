// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { createEditorSession, digestCreateEditorSession, digestRejectProposalOperations,
  digestReopenWithdrawnProposal, digestWithdrawProposal, getChapter, getProposal,
  rejectProposalOperations, reopenWithdrawnProposal, withdrawProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { RejectProposalOperationsRequest, ReopenWithdrawnProposalRequest,
  WithdrawProposalRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { BINDING, id, startRealServer, drainLeftoverWork, challenged, prepare, admitProse,
} from "../support/acceptance.ts";

test("an exact retry of a rejection returns its first acknowledgement after a withdrawal", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await openedProposal(started.baseUrl, "d9a1");
    const rejected = await reject(started.baseUrl, opened, opened.proposalRevisionId, "d9a171");
    const first = await rejected.call();
    assert.equal(first.effect.kind, "resolved");
    const withdrawn = await withdraw(started.baseUrl, opened, "d9a181");
    assert.equal(withdrawn.effect.kind, "resolved");
    assert.deepEqual(await rejected.call(), first);
  } finally {
    await stopRealServer(started.server);
  }
});

test("an exact retry of a reopened withdrawal returns its first acknowledgement after a rejection", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await openedProposal(started.baseUrl, "d9a2");
    const withdrawn = await withdraw(started.baseUrl, opened, "d9a271");
    if (withdrawn.effect.kind !== "resolved") throw new Error("expected an author withdrawal");
    const reopenRequest: ReopenWithdrawnProposalRequest = {
      command_schema: "storyos.command.reopen-withdrawn-proposal.request.v1",
      reopen_withdrawn_proposal_input: {
        proposal_revision_id: opened.proposalRevisionId,
        withdrawal_event_ref: withdrawn.effect.closure_event_refs[0] ?? "",
        expected_closure: "withdrawn",
        expected_target_revisions: [opened.chapterRevisionId],
        editor_session_id: opened.editorSessionId,
        ...BINDING,
        correlation_id: id("d9a281"),
      },
    };
    const reopen = (antiForgery: string) => reopenWithdrawnProposal({
      baseUrl: started.baseUrl,
      projectId: opened.projectId,
      proposalId: opened.proposalId,
      fetchImpl: opened.fetchImpl,
      idempotencyKey: id("d9a282"),
      antiForgery,
      request: reopenRequest,
    });
    let nonce = "";
    const reopened = await challenged(
      started.baseUrl,
      opened.fetchImpl,
      opened.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/reopenings",
      reopenRequest.command_schema,
      await digestReopenWithdrawnProposal(reopenRequest),
      id("d9a282"),
      (antiForgery) => {
        nonce = antiForgery;
        return reopen(antiForgery);
      },
    );
    if (reopened.effect.kind !== "resolved") throw new Error("expected a resolved reopen");
    assert.equal(reopened.effect.preserved_operation_resolution, "pending");
    const rejected = await reject(
      started.baseUrl, opened, reopened.effect.resulting_proposal_revision_id, "d9a291",
    );
    assert.equal((await rejected.call()).effect.kind, "resolved");
    assert.deepEqual(await reopen(nonce), reopened);
  } finally {
    await stopRealServer(started.server);
  }
});

/** One open Proposal with one pending Proposal Operation and a writer Editor Session. */
async function openedProposal(baseUrl: string, ns: string) {
  const prepared = await prepare(baseUrl, id(`${ns}0111`), "Proposal Decision Replay Novel", ns);
  const before = await getChapter({
    baseUrl,
    projectId: prepared.projectId,
    chapterId: prepared.chapterId,
    fetchImpl: prepared.fetchImpl,
  });
  const queried = await admitProse(
    baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id(`${ns}0131`),
  );
  if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
    throw new Error("expected opened prose");
  }
  const proposal = await getProposal({
    baseUrl,
    projectId: prepared.projectId,
    proposalId: queried.decision.opened_proposal.proposal_id,
    fetchImpl: prepared.fetchImpl,
  });
  const sessionRequest = {
    command_schema: "storyos.command.create-editor-session.request.v1" as const,
    ...BINDING,
    correlation_id: id(`${ns}0151`),
  };
  const session = await challenged(
    baseUrl,
    prepared.fetchImpl,
    prepared.projectId,
    "POST",
    "/api/v1/projects/{project_id}/editor-sessions",
    sessionRequest.command_schema,
    await digestCreateEditorSession(sessionRequest),
    id(`${ns}0152`),
    (antiForgery) => createEditorSession({
      baseUrl,
      projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id(`${ns}0152`),
      antiForgery,
      request: sessionRequest,
    }),
  );
  return {
    fetchImpl: prepared.fetchImpl,
    projectId: prepared.projectId,
    proposalId: proposal.proposal.proposal_id,
    proposalRevisionId: proposal.proposal.revision_id,
    operationId: proposal.proposal.operation_id,
    chapterRevisionId: before.chapter.current_revision.revision_id,
    editorSessionId: session.editor_session.editor_session_id,
  };
}

type OpenedProposal = Awaited<ReturnType<typeof openedProposal>>;

/** Admits one rejection of the pending Proposal Operation. `call` sends it again exactly. */
async function reject(baseUrl: string, opened: OpenedProposal, revisionId: string, key: string) {
  const request: RejectProposalOperationsRequest = {
    command_schema: "storyos.command.reject-proposal-operations.request.v1",
    reject_proposal_operations_input: {
      proposal_revision_id: revisionId,
      selected_pending_operation_ids: [opened.operationId],
      expected_target_revisions: [opened.chapterRevisionId],
      rejection_reason: { kind: "author_declined", note: { kind: "omitted" } },
      editor_session_id: opened.editorSessionId,
      ...BINDING,
      correlation_id: id(key),
    },
  };
  const nonce = await challenged(
    baseUrl,
    opened.fetchImpl,
    opened.projectId,
    "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
    request.command_schema,
    await digestRejectProposalOperations(request),
    id(`${key}2`),
    async (antiForgery) => antiForgery,
  );
  return {
    call: () => rejectProposalOperations({
      baseUrl,
      projectId: opened.projectId,
      proposalId: opened.proposalId,
      fetchImpl: opened.fetchImpl,
      idempotencyKey: id(`${key}2`),
      antiForgery: nonce,
      request,
    }),
  };
}

async function withdraw(baseUrl: string, opened: OpenedProposal, key: string) {
  const request: WithdrawProposalRequest = {
    command_schema: "storyos.command.withdraw-proposal.request.v1",
    withdraw_proposal_input: {
      cause: "author",
      proposal_revision_id: opened.proposalRevisionId,
      expected_closure: "open",
      expected_target_revisions: [opened.chapterRevisionId],
      withdrawal_reason: { kind: "author_withdrew", note: { kind: "omitted" } },
      editor_session_id: opened.editorSessionId,
      ...BINDING,
      correlation_id: id(key),
    },
  };
  return challenged(
    baseUrl,
    opened.fetchImpl,
    opened.projectId,
    "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/withdrawals",
    request.command_schema,
    await digestWithdrawProposal(request),
    id(`${key}2`),
    (antiForgery) => withdrawProposal({
      baseUrl,
      projectId: opened.projectId,
      proposalId: opened.proposalId,
      fetchImpl: opened.fetchImpl,
      idempotencyKey: id(`${key}2`),
      antiForgery,
      request,
    }),
  );
}
