// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/edit-proposal-candidate-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { acceptProposal, digestAcceptProposal, digestExportProjectArchive, exportProjectArchive,
  getExportOperation, getChapter, getProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, requireStoryOSProtocolError,
  sessionFetch as browserFetch, stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { USER_A, UUID_V7, BINDING, PROSE, REVISED, id, startRealServer, settleOnce,
  drainLeftoverWork, challenged, prepare, admitProse, reviseCandidate } from "../support/acceptance.ts";

test("acceptProposal applies one pending Operation and retries the settled outcome", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("d111"), "Accept Proposal Novel", "d2");
    const before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const queried = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("d131"));
    assert.equal(queried.decision.kind, "prose_change");
    if (queried.decision.kind !== "prose_change") throw new Error("expected prose");
    assert.equal(queried.decision.opened_proposal.kind, "present");
    if (queried.decision.opened_proposal.kind !== "present") throw new Error("expected opened");
    const opened = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: queried.decision.opened_proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(opened.proposal.candidate_text, PROSE);
    const { session, revised } = await reviseCandidate(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      opened,
      "d14",
    );
    assert.equal(revised.proposal.candidate_text, REVISED);
    assert.equal(revised.proposal.validation_receipt.kind, "present");
    if (revised.proposal.validation_receipt.kind !== "present") throw new Error("expected receipt");
    const acceptRequest: AcceptProposalRequest = {
      command_schema: "storyos.command.accept-proposal.request.v1",
      accept_proposal_input: {
        proposal_revision_id: revised.proposal.revision_id,
        validation_receipt_id: revised.proposal.validation_receipt.validation_receipt_id,
        selected_operation_ids: [revised.proposal.operation_id],
        expected_authoritative_revision_id: before.chapter.current_revision.revision_id,
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING,
        correlation_id: id("d151"),
      },
    };
    const acceptOptions = {
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: revised.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("d152"),
      antiForgery: "",
      request: acceptRequest,
    };
    const accepted = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      acceptRequest.command_schema,
      await digestAcceptProposal(acceptRequest),
      id("d152"),
      (antiForgery) => {
        acceptOptions.antiForgery = antiForgery;
        return acceptProposal(acceptOptions);
      },
    );
    assert.equal(accepted.effect.kind, "applied");
    assert.equal(accepted.receipt.result, "applied");
    if (accepted.effect.kind !== "applied") throw new Error("expected applied");
    assert.match(accepted.effect.authoritative_commit_id, UUID_V7);
    assert.match(accepted.effect.authoritative_revision.revision_id, UUID_V7);
    assert.equal(accepted.effect.authoritative_revision.body, REVISED);
    assert.deepEqual(await acceptProposal(acceptOptions), accepted);
    const inspected = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: revised.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(inspected.proposal.operation_resolution, "applied");
    assert.equal(inspected.proposal.reservation_state, "resolved");
    assert.equal(inspected.proposal.revision_id, revised.proposal.revision_id);
    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(after.chapter.current_revision.body, REVISED);
    assert.equal(after.chapter.current_revision.revision_id, accepted.effect.authoritative_revision.revision_id);
    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    const acceptDigest = await digestAcceptProposal(acceptRequest);
    await assert.rejects(
      () => challenged(
        started.baseUrl,
        foreignFetch,
        prepared.projectId,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
        acceptRequest.command_schema,
        acceptDigest,
        id("d153"),
        (antiForgery) => acceptProposal({
          ...acceptOptions,
          fetchImpl: foreignFetch,
          idempotencyKey: id("d153"),
          antiForgery,
        }),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    const wrongAdmission: AcceptProposalRequest = {
      command_schema: "storyos.command.accept-proposal.request.v1",
      accept_proposal_input: {
        ...acceptRequest.accept_proposal_input,
        editor_session_id: id("d160"),
        correlation_id: id("d161"),
      },
    };
    const digest = await digestAcceptProposal(wrongAdmission);
    await assert.rejects(
      () => challenged(
        started.baseUrl,
        prepared.fetchImpl,
        prepared.projectId,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
        wrongAdmission.command_schema,
        digest,
        id("d162"),
        (antiForgery) => acceptProposal({
          baseUrl: started.baseUrl,
          projectId: prepared.projectId,
          proposalId: revised.proposal.proposal_id,
          fetchImpl: prepared.fetchImpl,
          idempotencyKey: id("d162"),
          antiForgery,
          request: wrongAdmission,
        }),
      ),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );
  } finally {
    await stopRealServer(started.server);
  }
});

test.each(["invalid_validation", "altered_candidate"] as const)("acceptProposal retains %s across reload and refuses another attempt until revision", async (reason) => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id({ invalid_validation: "d211", altered_candidate: "d213" }[reason]), "Refuse Acceptance Novel", "d3");
    const before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const queried = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("d231"));
    if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
      throw new Error("expected opened prose");
    }
    const opened = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: queried.decision.opened_proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    const { session, revised } = await reviseCandidate(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      opened,
      "d24",
    );
    if (opened.proposal.validation_receipt.kind !== "present" || revised.proposal.validation_receipt.kind !== "present") {
      throw new Error("expected receipts");
    }
    const stale: AcceptProposalRequest = {
      command_schema: "storyos.command.accept-proposal.request.v1",
      accept_proposal_input: {
        proposal_revision_id: opened.proposal.revision_id,
        validation_receipt_id: opened.proposal.validation_receipt.validation_receipt_id,
        selected_operation_ids: [opened.proposal.operation_id],
        expected_authoritative_revision_id: before.chapter.current_revision.revision_id,
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING,
        correlation_id: id("d251"),
      },
    };
    const staleAccepted = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      stale.command_schema,
      await digestAcceptProposal(stale),
      id("d252"),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("d252"),
        antiForgery,
        request: stale,
      }),
    );
    assert.equal(staleAccepted.effect.kind, "refused");
    assert.equal(staleAccepted.receipt.result, "refused");
    if (staleAccepted.effect.kind !== "refused") throw new Error("expected refused");
    assert.equal(staleAccepted.effect.reason, "stale_proposal_revision");
    if (reason === "altered_candidate") {
      await queryPostgres(`UPDATE storyos.proposal_revisions
        SET candidate_text = 'Tampered narrator voice.'
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${prepared.projectId}'::uuid
          AND revision_id = '${revised.proposal.revision_id}'::uuid`);
    }
    const request: AcceptProposalRequest = {
      command_schema: "storyos.command.accept-proposal.request.v1",
      accept_proposal_input: {
        proposal_revision_id: revised.proposal.revision_id,
        validation_receipt_id: reason === "invalid_validation"
          ? opened.proposal.validation_receipt.validation_receipt_id
          : revised.proposal.validation_receipt.validation_receipt_id,
        selected_operation_ids: [revised.proposal.operation_id],
        expected_authoritative_revision_id: before.chapter.current_revision.revision_id,
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING, correlation_id: id("d253"),
      },
    };
    const command = {
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: revised.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("d254"), request,
    };
    let originalNonce = "";
    const failed = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      request.command_schema, await digestAcceptProposal(request), command.idempotencyKey,
      async (antiForgery) => {
        originalNonce = antiForgery;
        return acceptProposal({ ...command, antiForgery });
      },
    );
    const validation = "invalid";
    assert.deepEqual(failed.effect, { kind: validation, reason });
    assert.deepEqual(failed.receipt.condition_refs, []);
    const tampered = "Tampered narrator voice.";
    const comparison = revised.proposal.revision_comparison;
    const expected = {
      ...revised.proposal, validation, condition_refs: failed.receipt.condition_refs,
      candidate_text: reason === "altered_candidate" ? tampered : revised.proposal.candidate_text,
      revision_comparison: reason === "altered_candidate" && comparison.kind === "present"
        ? { ...comparison, spans: [{ base_from: 0, base_to: 0, candidate_from: 0, candidate_to: tampered.length, base_text: "", candidate_text: tampered }] }
        : comparison,
      source_condition: revised.proposal.source_condition,
    };
    const reload = await getProposal(command);
    assert.deepEqual(reload.proposal, expected);
    const replayed = await Promise.all([0, 1].map(() => acceptProposal({ ...command, antiForgery: originalNonce })));
    for (const replay of replayed) assert.deepEqual(replay, failed);
    const retried: AcceptProposalRequest = {
      ...request,
      accept_proposal_input: {
        ...request.accept_proposal_input,
        validation_receipt_id: revised.proposal.validation_receipt.validation_receipt_id,
        expected_authoritative_revision_id: before.chapter.current_revision.revision_id,
      },
    };
    const refused = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      retried.command_schema, await digestAcceptProposal(retried), id("d258"),
      (antiForgery) => acceptProposal({ ...command, idempotencyKey: id("d258"), request: retried, antiForgery }),
    );
    assert.deepEqual(refused.effect, { kind: "refused", reason: "not_eligible" });
    assert.deepEqual((await getProposal(command)).proposal, expected);
    await assert.rejects(() => getProposal({ ...command, fetchImpl: browserFetch(started.baseUrl, "session-b") }),
      (error) => requireStoryOSProtocolError(error).status === 404);
    await stopRealServer(started.server);
    started = await startRealServer();
    prepared.fetchImpl = browserFetch(started.baseUrl, "session-a");
    assert.deepEqual((await getProposal({ ...command, baseUrl: started.baseUrl,
      fetchImpl: browserFetch(started.baseUrl, "session-a") })).proposal, expected);

    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(after.chapter, before.chapter);
    const stillPending = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: revised.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(stillPending.proposal.operation_resolution, "pending");
    if (reason === "invalid_validation") {
      const recovered = await reviseCandidate(started.baseUrl, prepared.fetchImpl, prepared.projectId,
        stillPending, "d27", session);
      assert.equal(recovered.revised.proposal.validation, "valid");
      assert.deepEqual(recovered.revised.proposal.condition_refs, []);
      if (recovered.revised.proposal.validation_receipt.kind !== "present") throw new Error("expected validation");
      const fresh: AcceptProposalRequest = { ...request, accept_proposal_input: {
        ...request.accept_proposal_input,
        proposal_revision_id: recovered.revised.proposal.revision_id,
        validation_receipt_id: recovered.revised.proposal.validation_receipt.validation_receipt_id,
      } };
      const applied = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", fresh.command_schema,
        await digestAcceptProposal(fresh), id("d278"),
        (antiForgery) => acceptProposal({ ...command, baseUrl: started.baseUrl, fetchImpl: prepared.fetchImpl,
          request: fresh, idempotencyKey: id("d278"), antiForgery }));
      assert.equal(applied.effect.kind, "applied");
    }
  } finally {
    await stopRealServer(started.server);
  }
});
