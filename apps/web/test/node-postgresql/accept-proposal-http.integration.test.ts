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
        selected_operation_id: revised.proposal.operation_id,
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

test.each(["invalid_validation", "changed_head", "altered_candidate"] as const)("acceptProposal retains %s across reload and refuses another attempt until revision", async (reason) => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id({ invalid_validation: "d211", changed_head: "d212", altered_candidate: "d213" }[reason]), "Refuse Acceptance Novel", "d3");
    let before = await getChapter({
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
        selected_operation_id: opened.proposal.operation_id,
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
    if (reason === "changed_head") {
      // Advance the stored Head without revalidating the pending Proposal.
      await queryPostgres(`INSERT INTO storyos.authoritative_revisions
        SELECT owner_user_id, project_id, manuscript_object_id, '${id("d255")}'::uuid, payload_id
        FROM storyos.authoritative_revisions WHERE project_id = '${prepared.projectId}'::uuid
          AND revision_id = '${before.chapter.current_revision.revision_id}'::uuid;
        INSERT INTO storyos.manuscript_revision_members
        SELECT owner_user_id, project_id, manuscript_object_id, '${id("d255")}'::uuid,
          manuscript_block_id, block_order FROM storyos.manuscript_revision_members
        WHERE project_id = '${prepared.projectId}'::uuid
          AND revision_id = '${before.chapter.current_revision.revision_id}'::uuid;
        UPDATE storyos.authoritative_heads SET current_revision_id = '${id("d255")}'::uuid
        WHERE project_id = '${prepared.projectId}'::uuid
          AND manuscript_object_id = '${prepared.chapterId}'::uuid`);
      before = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    }
    const request: AcceptProposalRequest = {
      command_schema: "storyos.command.accept-proposal.request.v1",
      accept_proposal_input: {
        proposal_revision_id: revised.proposal.revision_id,
        validation_receipt_id: reason === "invalid_validation"
          ? opened.proposal.validation_receipt.validation_receipt_id
          : revised.proposal.validation_receipt.validation_receipt_id,
        selected_operation_id: revised.proposal.operation_id,
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
        if (reason === "changed_head") {
          await queryPostgres(`CREATE FUNCTION storyos.test_condition_failure() RETURNS trigger
            LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'condition fault'; END $$;
            CREATE TRIGGER test_condition_failure BEFORE INSERT ON storyos.proposal_validation_conditions
            FOR EACH ROW EXECUTE FUNCTION storyos.test_condition_failure();`);
          try {
            await assert.rejects(() => acceptProposal({ ...command, antiForgery }),
              (error) => requireStoryOSProtocolError(error).status === 503);
            assert.deepEqual((await getProposal(command)).proposal, revised.proposal);
          } finally {
            await queryPostgres(`DROP TRIGGER test_condition_failure ON storyos.proposal_validation_conditions;
              DROP FUNCTION storyos.test_condition_failure();`);
          }
        }
        return acceptProposal({ ...command, antiForgery });
      },
    );
    const validation = reason === "changed_head" ? "conflicted" : "invalid";
    assert.deepEqual(failed.effect, { kind: validation, reason });
    assert.equal(failed.receipt.condition_refs.length, reason === "changed_head" ? 1 : 0);
    for (const ref of failed.receipt.condition_refs) assert.match(ref, UUID_V7);
    const expected = {
      ...revised.proposal, validation, condition_refs: failed.receipt.condition_refs,
      candidate_text: reason === "altered_candidate" ? "Tampered narrator voice." : revised.proposal.candidate_text,
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
    if (reason === "changed_head") {
      const archiveRequest = {
        command_schema: "storyos.command.export-project-archive.request.v1" as const,
        export_project_archive_input: { ...BINDING, correlation_id: id("d260"),
          archive_profile: "storyos.project-export.v1",
          archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" },
      };
      const archive = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId,
        "POST", "/api/v1/projects/{project_id}/exports", archiveRequest.command_schema,
        await digestExportProjectArchive(archiveRequest), id("d261"),
        (antiForgery) => exportProjectArchive({ ...command, request: archiveRequest, idempotencyKey: id("d261"), antiForgery }));
      if (archive.effect.kind !== "admitted") throw new Error("expected admitted archive");
      await settleOnce();
      const archiveOptions = { ...command, exportId: archive.effect.export_id };
      assert.equal((await getExportOperation(archiveOptions)).status, "ready");
      const download = await prepared.fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${archive.effect.export_id}`,
        { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
      assert.equal(download.status, 200);
      const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
      const conditions = JSON.parse(new TextDecoder().decode(files.get("canonical/proposal_validation_conditions.json")));
      assert.deepEqual(conditions, [{ owner_user_id: USER_A, project_id: prepared.projectId,
        proposal_id: revised.proposal.proposal_id, proposal_revision_id: revised.proposal.revision_id,
        acceptance_receipt_id: failed.receipt.receipt_id, validation: "conflicted",
        conflict_id: failed.receipt.condition_refs[0] }]);
    }
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
