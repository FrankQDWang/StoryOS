// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/replan-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { createEditorSession, digestCreateEditorSession, digestUpdateProjectAssistance,
  digestWithdrawProposal, getChapter, getProposal, updateProjectAssistance, withdrawProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { UpdateProjectAssistanceRequest, WithdrawProposalRequest,
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
