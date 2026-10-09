// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/replan-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { createEditorSession, digestCreateEditorSession, digestReopenWithdrawnProposal,
  digestUndoLatestAuthorAction, digestUpdateProjectAssistance, digestWithdrawProposal, getChapter,
  getProposal, reopenWithdrawnProposal, undoLatestAuthorAction, updateProjectAssistance,
  withdrawProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ReopenWithdrawnProposalRequest, UndoLatestAuthorActionRequest,
  UpdateProjectAssistanceRequest, WithdrawProposalRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { requireStoryOSProtocolError, sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { BINDING, USER_A, id, startRealServer, drainLeftoverWork, challenged, prepare, admitProse,
} from "../support/acceptance.ts";

test("author withdraw closes the current Proposal without changing authority", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e8f00111"), "Withdraw Proposal Novel", "e8f2");
    const before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const queried = await admitProse(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("e8f00131"),
    );
    assert.equal(queried.decision.kind, "prose_change");
    if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
      throw new Error("expected opened prose");
    }
    const opened = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: queried.decision.opened_proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(opened.proposal.closure, "open");
    const assistanceRequest: UpdateProjectAssistanceRequest = {
      command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: {
        availability: "unavailable",
        expected_assistance_revision: "1",
        ...BINDING,
        correlation_id: id("e8f00141"),
      },
    };
    const stopped = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "PUT",
      "/api/v1/projects/{project_id}/assistance",
      assistanceRequest.command_schema,
      await digestUpdateProjectAssistance(assistanceRequest),
      id("e8f00143"),
      (antiForgery) => updateProjectAssistance({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("e8f00143"),
        antiForgery,
        request: assistanceRequest,
      }),
    );
    assert.equal(stopped.effect.kind, "authoritative_applied");
    if (stopped.effect.kind === "authoritative_applied") {
      assert.equal(stopped.effect.availability, "unavailable");
    }
    const afterStop = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(afterStop.proposal.closure, "open");
    assert.equal(afterStop.proposal.operation_resolution, "pending");
    const sessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1" as const,
      ...BINDING,
      correlation_id: id("e8f00151"),
    };
    const session = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/editor-sessions",
      sessionRequest.command_schema,
      await digestCreateEditorSession(sessionRequest),
      id("e8f00152"),
      (antiForgery) => createEditorSession({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("e8f00152"),
        antiForgery,
        request: sessionRequest,
      }),
    );
    const withdrawRequest: WithdrawProposalRequest = {
      command_schema: "storyos.command.withdraw-proposal.request.v1",
      withdraw_proposal_input: {
        cause: "author",
        proposal_revision_id: opened.proposal.revision_id,
        expected_closure: "open",
        expected_target_revisions: [before.chapter.current_revision.revision_id],
        withdrawal_reason: { kind: "author_withdrew", note: { kind: "omitted" } },
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING,
        correlation_id: id("e8f00161"),
      },
    };
    const withdrawn = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/withdrawals",
      withdrawRequest.command_schema,
      await digestWithdrawProposal(withdrawRequest),
      id("e8f00162"),
      (antiForgery) => withdrawProposal({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("e8f00162"),
        antiForgery,
        request: withdrawRequest,
      }),
    );
    assert.equal(withdrawn.effect.kind, "resolved");
    if (withdrawn.effect.kind !== "resolved") throw new Error("expected resolved withdrawal");
    assert.equal(withdrawn.effect.prior_closure, "open");
    assert.equal(withdrawn.effect.resulting_closure, "withdrawn");
    assert.deepEqual(withdrawn.receipt.authoritative_commit_ids, []);
    const inspected = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(inspected.proposal.closure, "withdrawn");
    assert.equal(inspected.proposal.operation_resolution, "pending");
    assert.equal(inspected.proposal.revision_id, opened.proposal.revision_id);
    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(after.chapter, before.chapter);
    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    const foreignDigest = await digestWithdrawProposal(withdrawRequest);
    await assert.rejects(
      () => challenged(
        started.baseUrl,
        foreignFetch,
        prepared.projectId,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/withdrawals",
        withdrawRequest.command_schema,
        foreignDigest,
        id("e8f00181"),
        (antiForgery) => withdrawProposal({
          baseUrl: started.baseUrl,
          projectId: prepared.projectId,
          proposalId: opened.proposal.proposal_id,
          fetchImpl: foreignFetch,
          idempotencyKey: id("e8f00181"),
          antiForgery,
          request: withdrawRequest,
        }),
      ),
      (error: unknown) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    const againRequest: WithdrawProposalRequest = {
      ...withdrawRequest,
      withdraw_proposal_input: {
        ...withdrawRequest.withdraw_proposal_input,
        correlation_id: id("e8f00171"),
      },
    };
    const again = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/withdrawals",
      againRequest.command_schema,
      await digestWithdrawProposal(againRequest),
      id("e8f00172"),
      (antiForgery) => withdrawProposal({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("e8f00172"),
        antiForgery,
        request: againRequest,
      }),
    );
    assert.deepEqual(again.effect, { kind: "no_effect", reason: "closure_not_open" });
    assert.equal(again.receipt.authoritative_commit_ids.length, 0);
  } finally {
    await stopRealServer(started.server);
  }
});

test("current producer withdraw closes the Proposal with zero author actions", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e8f00211"), "Producer Withdraw Novel", "e8f3");
    const before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const queried = await admitProse(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("e8f00231"),
    );
    if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
      throw new Error("expected opened prose");
    }
    const opened = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: queried.decision.opened_proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(opened.proposal.closure, "open");
    if (opened.proposal.source.kind !== "agent_run_decision") {
      throw new Error("expected an agent-run producer");
    }
    const mismatch: WithdrawProposalRequest = {
      command_schema: "storyos.command.withdraw-proposal.request.v1",
      withdraw_proposal_input: {
        cause: "current_producer",
        producer: {
          kind: "agent_run_decision",
          run_id: id("e8f00299"),
          decision_id: id("e8f00298"),
        },
        proposal_revision_id: opened.proposal.revision_id,
        expected_closure: "open",
        expected_target_revisions: [before.chapter.current_revision.revision_id],
        withdrawal_reason: { kind: "current_producer_withdrew" },
        ...BINDING,
        correlation_id: id("e8f00241"),
      },
    };
    const mismatched = await withdrawProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("e8f00242"),
      request: mismatch,
    });
    assert.deepEqual(mismatched.effect, { kind: "no_effect", reason: "unsupported_cause" });
    assert.equal(mismatched.author_command_admission_id, undefined);
    const stillOpen = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(stillOpen.proposal.closure, "open");
    const withdrawRequest: WithdrawProposalRequest = {
      command_schema: "storyos.command.withdraw-proposal.request.v1",
      withdraw_proposal_input: {
        cause: "current_producer",
        producer: {
          kind: "agent_run_decision",
          run_id: opened.proposal.source.run_id,
          decision_id: opened.proposal.source.decision_id,
        },
        proposal_revision_id: opened.proposal.revision_id,
        expected_closure: "open",
        expected_target_revisions: [before.chapter.current_revision.revision_id],
        withdrawal_reason: { kind: "current_producer_withdrew" },
        ...BINDING,
        correlation_id: id("e8f00251"),
      },
    };
    const withdrawn = await withdrawProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("e8f00252"),
      request: withdrawRequest,
    });
    assert.equal(withdrawn.effect.kind, "resolved");
    if (withdrawn.effect.kind !== "resolved") throw new Error("expected resolved withdrawal");
    assert.equal(withdrawn.effect.prior_closure, "open");
    assert.equal(withdrawn.effect.resulting_closure, "withdrawn");
    assert.equal(withdrawn.effect.withdrawal_reason.kind, "current_producer_withdrew");
    assert.equal(withdrawn.effect.author_action_sequence, undefined);
    assert.equal(withdrawn.effect.undo_disposition, undefined);
    assert.equal(withdrawn.author_command_admission_id, undefined);
    assert.equal(withdrawn.receipt.author_command_admission_id, undefined);
    assert.deepEqual(withdrawn.receipt.authoritative_commit_ids, []);
    const repeated = await withdrawProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("e8f00252"),
      request: withdrawRequest,
    });
    assert.equal(repeated.receipt.receipt_id, withdrawn.receipt.receipt_id);
    assert.equal(repeated.effect.kind, "resolved");
    const inspected = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(inspected.proposal.closure, "withdrawn");
    assert.equal(inspected.proposal.operation_resolution, "pending");
    assert.equal(inspected.proposal.revision_id, opened.proposal.revision_id);
    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(after.chapter, before.chapter);
    const closedRequest: WithdrawProposalRequest = {
      ...withdrawRequest,
      withdraw_proposal_input: {
        ...withdrawRequest.withdraw_proposal_input,
        correlation_id: id("e8f00261"),
      },
    };
    const closed = await withdrawProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("e8f00262"),
      request: closedRequest,
    });
    assert.deepEqual(closed.effect, { kind: "no_effect", reason: "closure_not_open" });
  } finally {
    await stopRealServer(started.server);
  }
});

test("explicit reopen appends a pending open revision and leaves authority unchanged", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await withdrawOpenedProposal(started.baseUrl, "e8f00411", "e8f4");
    const reopenRequest: ReopenWithdrawnProposalRequest = {
      command_schema: "storyos.command.reopen-withdrawn-proposal.request.v1",
      reopen_withdrawn_proposal_input: {
        proposal_revision_id: opened.proposalRevisionId,
        withdrawal_event_ref: opened.withdrawalEventId,
        expected_closure: "withdrawn",
        expected_target_revisions: [opened.chapterRevisionId],
        editor_session_id: opened.editorSessionId,
        ...BINDING,
        correlation_id: id("e8f00471"),
      },
    };
    const reopened = await challenged(
      started.baseUrl,
      opened.fetchImpl,
      opened.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/reopenings",
      reopenRequest.command_schema,
      await digestReopenWithdrawnProposal(reopenRequest),
      id("e8f00472"),
      (antiForgery) => reopenWithdrawnProposal({
        baseUrl: started.baseUrl,
        projectId: opened.projectId,
        proposalId: opened.proposalId,
        fetchImpl: opened.fetchImpl,
        idempotencyKey: id("e8f00472"),
        antiForgery,
        request: reopenRequest,
      }),
    );
    assert.equal(reopened.effect.kind, "resolved");
    if (reopened.effect.kind !== "resolved") throw new Error("expected resolved reopen");
    assert.equal(reopened.effect.prior_closure, "withdrawn");
    assert.equal(reopened.effect.resulting_closure, "open");
    assert.equal(reopened.effect.resulting_validation, "pending");
    assert.equal(reopened.effect.undo_disposition, "forward");
    assert.equal(reopened.effect.preserved_operation_resolution, "pending");
    assert.deepEqual(reopened.receipt.authoritative_commit_ids, []);
    assert.notEqual(reopened.effect.resulting_proposal_revision_id, opened.proposalRevisionId);
    const inspected = await getProposal({
      baseUrl: started.baseUrl,
      projectId: opened.projectId,
      proposalId: opened.proposalId,
      fetchImpl: opened.fetchImpl,
    });
    assert.equal(inspected.proposal.closure, "open");
    assert.equal(inspected.proposal.validation, "pending");
    assert.equal(inspected.proposal.operation_resolution, "pending");
    assert.equal(inspected.proposal.revision_id, reopened.effect.resulting_proposal_revision_id);
    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: opened.projectId,
      chapterId: opened.chapterId,
      fetchImpl: opened.fetchImpl,
    });
    assert.deepEqual(after.chapter, opened.chapter);
    const againRequest: ReopenWithdrawnProposalRequest = {
      command_schema: reopenRequest.command_schema,
      reopen_withdrawn_proposal_input: {
        ...reopenRequest.reopen_withdrawn_proposal_input,
        proposal_revision_id: reopened.effect.resulting_proposal_revision_id,
        correlation_id: id("e8f00481"),
      },
    };
    const again = await challenged(
      started.baseUrl,
      opened.fetchImpl,
      opened.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/reopenings",
      againRequest.command_schema,
      await digestReopenWithdrawnProposal(againRequest),
      id("e8f00482"),
      (antiForgery) => reopenWithdrawnProposal({
        baseUrl: started.baseUrl,
        projectId: opened.projectId,
        proposalId: opened.proposalId,
        fetchImpl: opened.fetchImpl,
        idempotencyKey: id("e8f00482"),
        antiForgery,
        request: againRequest,
      }),
    );
    assert.deepEqual(again.effect, { kind: "no_effect", reason: "withdrawal_event_mismatch" });
    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    const foreignDigest = await digestReopenWithdrawnProposal(reopenRequest);
    await assert.rejects(
      () => challenged(
        started.baseUrl,
        foreignFetch,
        opened.projectId,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/reopenings",
        reopenRequest.command_schema,
        foreignDigest,
        id("e8f00491"),
        (antiForgery) => reopenWithdrawnProposal({
          baseUrl: started.baseUrl,
          projectId: opened.projectId,
          proposalId: opened.proposalId,
          fetchImpl: foreignFetch,
          idempotencyKey: id("e8f00491"),
          antiForgery,
          request: reopenRequest,
        }),
      ),
      (error: unknown) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
  } finally {
    await stopRealServer(started.server);
  }
});

test("root undo of an author withdrawal reopens the Proposal under the existing frontier", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await withdrawOpenedProposal(started.baseUrl, "e8f00511", "e8f5");
    const undoRequest: UndoLatestAuthorActionRequest = {
      command_schema: "storyos.command.undo-latest-author-action.request.v1",
      undo_latest_author_action_input: {
        expected_author_undo_frontier_sequence: opened.authorActionSequence,
        expected_authoritative_revision_id: opened.chapterRevisionId,
        editor_session_id: opened.editorSessionId,
        ...BINDING,
        correlation_id: id("e8f00571"),
      },
    };
    const undone = await challenged(
      started.baseUrl,
      opened.fetchImpl,
      opened.projectId,
      "POST",
      "/api/v1/projects/{project_id}/author-actions/undo",
      undoRequest.command_schema,
      await digestUndoLatestAuthorAction(undoRequest),
      id("e8f00572"),
      (antiForgery) => undoLatestAuthorAction({
        baseUrl: started.baseUrl,
        projectId: opened.projectId,
        fetchImpl: opened.fetchImpl,
        idempotencyKey: id("e8f00572"),
        antiForgery,
        request: undoRequest,
      }),
    );
    assert.equal(undone.effect.kind, "compensated");
    if (undone.effect.kind !== "compensated") throw new Error("expected compensated undo");
    assert.equal(undone.effect.source_sequence, opened.authorActionSequence);
    assert.deepEqual([Object.hasOwn(undone.effect, "authoritative_revision"), Object.hasOwn(undone.effect, "authoritative_commit_id")],
      [false, false]);
    const inspected = await getProposal({
      baseUrl: started.baseUrl,
      projectId: opened.projectId,
      proposalId: opened.proposalId,
      fetchImpl: opened.fetchImpl,
    });
    assert.equal(inspected.proposal.closure, "open");
    assert.equal(inspected.proposal.validation, "pending");
    assert.equal(inspected.proposal.operation_resolution, "pending");
    assert.notEqual(inspected.proposal.revision_id, opened.proposalRevisionId);
    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: opened.projectId,
      chapterId: opened.chapterId,
      fetchImpl: opened.fetchImpl,
    });
    assert.deepEqual(after.chapter, opened.chapter);
    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    const foreignDigest = await digestUndoLatestAuthorAction(undoRequest);
    await assert.rejects(
      () => challenged(
        started.baseUrl,
        foreignFetch,
        opened.projectId,
        "POST",
        "/api/v1/projects/{project_id}/author-actions/undo",
        undoRequest.command_schema,
        foreignDigest,
        id("e8f00581"),
        (antiForgery) => undoLatestAuthorAction({
          baseUrl: started.baseUrl,
          projectId: opened.projectId,
          fetchImpl: foreignFetch,
          idempotencyKey: id("e8f00581"),
          antiForgery,
          request: undoRequest,
        }),
      ),
      (error: unknown) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
  } finally {
    await stopRealServer(started.server);
  }
});

test("an exact retry of the undo of an author withdrawal returns the first acknowledgement", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await withdrawOpenedProposal(started.baseUrl, "e8f00611", "e8f6");
    const undoRequest: UndoLatestAuthorActionRequest = {
      command_schema: "storyos.command.undo-latest-author-action.request.v1",
      undo_latest_author_action_input: {
        expected_author_undo_frontier_sequence: opened.authorActionSequence,
        expected_authoritative_revision_id: opened.chapterRevisionId,
        editor_session_id: opened.editorSessionId,
        ...BINDING,
        correlation_id: id("e8f00671"),
      },
    };
    let nonce = "";
    const send = (antiForgery: string) => {
      nonce = antiForgery;
      return undoLatestAuthorAction({
        baseUrl: started.baseUrl,
        projectId: opened.projectId,
        fetchImpl: opened.fetchImpl,
        idempotencyKey: id("e8f00672"),
        antiForgery,
        request: undoRequest,
      });
    };
    const undone = await challenged(
      started.baseUrl,
      opened.fetchImpl,
      opened.projectId,
      "POST",
      "/api/v1/projects/{project_id}/author-actions/undo",
      undoRequest.command_schema,
      await digestUndoLatestAuthorAction(undoRequest),
      id("e8f00672"),
      send,
    );
    assert.equal(undone.effect.kind, "compensated");
    const retried = await send(nonce);
    assert.deepEqual(retried, undone);
  } finally {
    await stopRealServer(started.server);
  }
});

async function withdrawOpenedProposal(baseUrl: string, projectKey: string, ns: string) {
  const prepared = await prepare(baseUrl, id(projectKey), "Reopen Withdrawn Proposal Novel", ns);
  const before = await getChapter({
    baseUrl,
    projectId: prepared.projectId,
    chapterId: prepared.chapterId,
    fetchImpl: prepared.fetchImpl,
  });
  const queried = await admitProse(
    baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id(`${ns}31`),
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
    correlation_id: id(`${ns}51`),
  };
  const session = await challenged(
    baseUrl,
    prepared.fetchImpl,
    prepared.projectId,
    "POST",
    "/api/v1/projects/{project_id}/editor-sessions",
    sessionRequest.command_schema,
    await digestCreateEditorSession(sessionRequest),
    id(`${ns}52`),
    (antiForgery) => createEditorSession({
      baseUrl,
      projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id(`${ns}52`),
      antiForgery,
      request: sessionRequest,
    }),
  );
  const withdrawRequest: WithdrawProposalRequest = {
    command_schema: "storyos.command.withdraw-proposal.request.v1",
    withdraw_proposal_input: {
      cause: "author",
      proposal_revision_id: proposal.proposal.revision_id,
      expected_closure: "open",
      expected_target_revisions: [before.chapter.current_revision.revision_id],
      withdrawal_reason: { kind: "author_withdrew", note: { kind: "omitted" } },
      editor_session_id: session.editor_session.editor_session_id,
      ...BINDING,
      correlation_id: id(`${ns}61`),
    },
  };
  const withdrawn = await challenged(
    baseUrl,
    prepared.fetchImpl,
    prepared.projectId,
    "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/withdrawals",
    withdrawRequest.command_schema,
    await digestWithdrawProposal(withdrawRequest),
    id(`${ns}62`),
    (antiForgery) => withdrawProposal({
      baseUrl,
      projectId: prepared.projectId,
      proposalId: proposal.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id(`${ns}62`),
      antiForgery,
      request: withdrawRequest,
    }),
  );
  if (withdrawn.effect.kind !== "resolved" || withdrawn.effect.author_action_sequence === null) {
    throw new Error("expected an author withdrawal");
  }
  const withdrawalEventId = withdrawn.effect.closure_event_refs[0];
  if (withdrawalEventId === undefined) throw new Error("expected a withdrawal event");
  return {
    fetchImpl: prepared.fetchImpl,
    projectId: prepared.projectId,
    chapterId: prepared.chapterId,
    proposalId: proposal.proposal.proposal_id,
    proposalRevisionId: proposal.proposal.revision_id,
    chapterRevisionId: before.chapter.current_revision.revision_id,
    chapter: before.chapter,
    editorSessionId: session.editor_session.editor_session_id,
    withdrawalEventId,
    authorActionSequence: withdrawn.effect.author_action_sequence,
  };
}
