// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/accept-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { acceptProposal, digestAcceptProposal, digestExportProjectArchive, digestReplanProposal,
  exportProjectArchive, getChapter, getExportOperation, getProposal, replanProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, ReplanProposalRequest
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, requireStoryOSProtocolError,
  sessionFetch as browserFetch, stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { USER_A, UUID_V7, BINDING, id, startRealServer, settleOnce,
  drainLeftoverWork, challenged, prepare, admitProse, reviseCandidate } from "../support/acceptance.ts";

test("replanProposal appends a pending Revision bound to the current Head", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e3860111"), "Replan Proposal Novel", "e3862");
    let before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const queried = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("e3860131"));
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
    assert.deepEqual(opened.proposal.source_condition, { kind: "absent" });
    const { session, revised } = await reviseCandidate(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      opened,
      "e38614",
    );
    if (revised.proposal.validation_receipt.kind !== "present") {
      throw new Error("expected validation receipt");
    }
    await queryPostgres(`INSERT INTO storyos.authoritative_revisions
      SELECT owner_user_id, project_id, manuscript_object_id, '${id("e3860155")}'::uuid, payload_id
      FROM storyos.authoritative_revisions WHERE project_id = '${prepared.projectId}'::uuid
        AND revision_id = '${before.chapter.current_revision.revision_id}'::uuid;
      INSERT INTO storyos.manuscript_revision_members
      SELECT owner_user_id, project_id, manuscript_object_id, '${id("e3860155")}'::uuid,
        manuscript_block_id, block_order FROM storyos.manuscript_revision_members
      WHERE project_id = '${prepared.projectId}'::uuid
        AND revision_id = '${before.chapter.current_revision.revision_id}'::uuid;
      UPDATE storyos.authoritative_heads SET current_revision_id = '${id("e3860155")}'::uuid
      WHERE project_id = '${prepared.projectId}'::uuid
        AND manuscript_object_id = '${prepared.chapterId}'::uuid`);
    before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const acceptRequest: AcceptProposalRequest = {
      command_schema: "storyos.command.accept-proposal.request.v1",
      accept_proposal_input: {
        proposal_revision_id: revised.proposal.revision_id,
        validation_receipt_id: revised.proposal.validation_receipt.validation_receipt_id,
        selected_operation_ids: [revised.proposal.operation_id],
        expected_authoritative_revision_id: before.chapter.current_revision.revision_id,
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING,
        correlation_id: id("e3860151"),
      },
    };
    const conflicted = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      acceptRequest.command_schema,
      await digestAcceptProposal(acceptRequest),
      id("e3860152"),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        proposalId: revised.proposal.proposal_id,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("e3860152"),
        antiForgery,
        request: acceptRequest,
      }),
    );
    assert.deepEqual(conflicted.effect, { kind: "conflicted", reason: "changed_head" });
    const conflictRef = conflicted.receipt.condition_refs[0] ?? "";
    assert.match(conflictRef, UUID_V7);
    const afterConflict = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: revised.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(afterConflict.proposal.validation, "conflicted");
    assert.equal(afterConflict.proposal.revision_id, revised.proposal.revision_id);
    assert.deepEqual(afterConflict.proposal.source_condition, {
      kind: "proposal_conflict",
      proposal_conflict_ref: conflictRef,
    });
    await queryPostgres(`UPDATE storyos.proposal_validation_conditions
      SET condition_kind = 'proposal_recovery_conflict'
      WHERE conflict_id = '${conflictRef}'::uuid`);
    assert.deepEqual((await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: revised.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    })).proposal.source_condition, {
      kind: "proposal_recovery_conflict",
      proposal_recovery_conflict_ref: conflictRef,
    });
    await queryPostgres(`UPDATE storyos.proposal_validation_conditions
      SET condition_kind = 'proposal_conflict'
      WHERE conflict_id = '${conflictRef}'::uuid`);
    assert.deepEqual((await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: revised.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    })).proposal.source_condition, {
      kind: "proposal_conflict",
      proposal_conflict_ref: conflictRef,
    });
    const replanRequest: ReplanProposalRequest = {
      command_schema: "storyos.command.replan-proposal.request.v1",
      replan_proposal_input: {
        conflicted_proposal_revision_id: afterConflict.proposal.revision_id,
        expected_current_proposal_head: afterConflict.proposal.revision_id,
        expected_current_target_revisions: [before.chapter.current_revision.revision_id],
        replacement_operations: [afterConflict.proposal.operation_id],
        source_condition: { kind: "proposal_conflict", proposal_conflict_ref: conflictRef },
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING,
        correlation_id: id("e3860171"),
      },
    };
    const replanOptions = {
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: afterConflict.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("e3860172"),
      antiForgery: "",
      request: replanRequest,
    };
    const unavailable: ReplanProposalRequest = {
      command_schema: "storyos.command.replan-proposal.request.v1",
      replan_proposal_input: {
        ...replanRequest.replan_proposal_input,
        source_condition: { kind: "proposal_conflict", proposal_conflict_ref: id("e3860163") },
        correlation_id: id("e3860164"),
      },
    };
    const unavailableProof = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans",
      unavailable.command_schema,
      await digestReplanProposal(unavailable),
      id("e3860165"),
      (antiForgery) => replanProposal({
        ...replanOptions,
        idempotencyKey: id("e3860165"),
        antiForgery,
        request: unavailable,
      }),
    );
    assert.deepEqual(unavailableProof.effect, { kind: "refused", reason: "unavailable_proof" });
    assert.deepEqual((await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: afterConflict.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    })).proposal.source_condition, {
      kind: "proposal_conflict",
      proposal_conflict_ref: conflictRef,
    });
    const replanned = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans",
      replanRequest.command_schema,
      await digestReplanProposal(replanRequest),
      id("e3860172"),
      async (antiForgery) => {
        await queryPostgres(`CREATE FUNCTION storyos.test_replan_failure() RETURNS trigger
          LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'replan fault'; END $$;
          CREATE TRIGGER test_replan_failure BEFORE INSERT ON storyos.proposal_replans
          FOR EACH ROW EXECUTE FUNCTION storyos.test_replan_failure();`);
        try {
          await assert.rejects(
            () => replanProposal({ ...replanOptions, antiForgery }),
            (error) => requireStoryOSProtocolError(error).status === 503,
          );
          const stillConflicted = await getProposal({
            baseUrl: started.baseUrl,
            projectId: prepared.projectId,
            proposalId: afterConflict.proposal.proposal_id,
            fetchImpl: prepared.fetchImpl,
          });
          assert.equal(stillConflicted.proposal.revision_id, afterConflict.proposal.revision_id);
          assert.equal(stillConflicted.proposal.validation, "conflicted");
        } finally {
          await queryPostgres(`DROP TRIGGER test_replan_failure ON storyos.proposal_replans;
            DROP FUNCTION storyos.test_replan_failure();`);
        }
        replanOptions.antiForgery = antiForgery;
        return replanProposal(replanOptions);
      },
    );
    assert.equal(replanned.effect.kind, "resolved");
    assert.equal(replanned.receipt.result, "resolved");
    if (replanned.effect.kind !== "resolved") throw new Error("expected resolved");
    assert.equal(replanned.effect.undo_disposition, "forward");
    assert.equal(replanned.effect.resulting_validation, "pending");
    assert.equal(replanned.effect.preserved_generation, afterConflict.proposal.generation);
    assert.equal(replanned.effect.preserved_closure, afterConflict.proposal.closure);
    assert.deepEqual(replanned.effect.source_condition, {
      kind: "proposal_conflict",
      proposal_conflict_ref: conflictRef,
    });
    assert.match(replanned.effect.author_action_sequence, /^\d+$/);
    assert.equal(replanned.effect.state_event_refs.length, 1);
    const replanEventId = replanned.effect.state_event_refs[0] ?? "";
    assert.match(replanEventId, UUID_V7);
    assert.notEqual(replanned.effect.resulting_proposal_revision_id, afterConflict.proposal.revision_id);
    assert.equal(replanned.receipt.source_proposal_revision_id, afterConflict.proposal.revision_id);
    assert.equal(replanned.receipt.resulting_proposal_revision_id, replanned.effect.resulting_proposal_revision_id);
    assert.deepEqual(replanned.receipt.authoritative_commit_ids, []);
    assert.deepEqual(await replanProposal(replanOptions), replanned);
    const inspected = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: afterConflict.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(inspected.proposal.operation_id, afterConflict.proposal.operation_id);
    assert.equal(inspected.proposal.operation_resolution, afterConflict.proposal.operation_resolution);
    assert.equal(inspected.proposal.validation, "pending");
    assert.equal(inspected.proposal.revision_id, replanned.effect.resulting_proposal_revision_id);
    assert.equal(inspected.proposal.base_authoritative_revision_id, before.chapter.current_revision.revision_id);
    assert.deepEqual(inspected.proposal.source_condition, { kind: "absent" });
    assert.deepEqual(inspected.proposal.validation_receipt, { kind: "absent" });
    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(after.chapter, before.chapter);
    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    const replanDigest = await digestReplanProposal(replanRequest);
    await assert.rejects(
      () => challenged(
        started.baseUrl,
        foreignFetch,
        prepared.projectId,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans",
        replanRequest.command_schema,
        replanDigest,
        id("e3860173"),
        (antiForgery) => replanProposal({
          ...replanOptions,
          fetchImpl: foreignFetch,
          idempotencyKey: id("e3860173"),
          antiForgery,
        }),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    const stale: ReplanProposalRequest = {
      command_schema: "storyos.command.replan-proposal.request.v1",
      replan_proposal_input: {
        ...replanRequest.replan_proposal_input,
        conflicted_proposal_revision_id: afterConflict.proposal.revision_id,
        expected_current_proposal_head: afterConflict.proposal.revision_id,
        correlation_id: id("e3860181"),
      },
    };
    const refused = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans",
      stale.command_schema,
      await digestReplanProposal(stale),
      id("e3860182"),
      (antiForgery) => replanProposal({
        ...replanOptions,
        idempotencyKey: id("e3860182"),
        antiForgery,
        request: stale,
      }),
    );
    assert.deepEqual(refused.effect, { kind: "refused", reason: "stale_proposal_revision" });
    const archiveRequest = {
      command_schema: "storyos.command.export-project-archive.request.v1" as const,
      export_project_archive_input: {
        ...BINDING,
        correlation_id: id("e3860190"),
        archive_profile: "storyos.project-export.v1",
        archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
      },
    };
    const archive = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/exports",
      archiveRequest.command_schema,
      await digestExportProjectArchive(archiveRequest),
      id("e3860191"),
      (antiForgery) => exportProjectArchive({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        request: archiveRequest,
        idempotencyKey: id("e3860191"),
        antiForgery,
      }),
    );
    if (archive.effect.kind !== "admitted") throw new Error("expected admitted archive");
    await settleOnce();
    assert.equal((await getExportOperation({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      exportId: archive.effect.export_id,
      fetchImpl: prepared.fetchImpl,
    })).status, "ready");
    const download = await prepared.fetchImpl(
      `${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${archive.effect.export_id}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } },
    );
    assert.equal(download.status, 200);
    const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
    const conditions = JSON.parse(new TextDecoder().decode(files.get("canonical/proposal_validation_conditions.json")));
    assert.deepEqual(conditions, [{
      owner_user_id: USER_A,
      project_id: prepared.projectId,
      proposal_id: afterConflict.proposal.proposal_id,
      proposal_revision_id: afterConflict.proposal.revision_id,
      acceptance_receipt_id: conflicted.receipt.receipt_id,
      validation: "conflicted",
      conflict_id: conflictRef,
      condition_kind: "proposal_conflict",
    }]);
    const replans = JSON.parse(new TextDecoder().decode(files.get("canonical/proposal_replans.json")));
    assert.deepEqual(replans, [{
      owner_user_id: USER_A,
      project_id: prepared.projectId,
      replan_event_id: replanEventId,
      proposal_id: afterConflict.proposal.proposal_id,
      source_proposal_revision_id: afterConflict.proposal.revision_id,
      resulting_proposal_revision_id: replanned.effect.resulting_proposal_revision_id,
      source_condition_kind: "proposal_conflict",
      source_condition_ref: conflictRef,
      replan_receipt_id: replanned.receipt.receipt_id,
      author_action_sequence: Number(replanned.effect.author_action_sequence),
      preserved_generation: replanned.effect.preserved_generation,
      preserved_closure: replanned.effect.preserved_closure,
    }]);
    await stopRealServer(started.server);
    started = await startRealServer();
    const reloaded = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: afterConflict.proposal.proposal_id,
      fetchImpl: browserFetch(started.baseUrl, "session-a"),
    });
    assert.equal(reloaded.proposal.revision_id, replanned.effect.resulting_proposal_revision_id);
    assert.equal(reloaded.proposal.validation, "pending");
    assert.equal(reloaded.proposal.base_authoritative_revision_id, before.chapter.current_revision.revision_id);
    assert.deepEqual(reloaded.proposal.source_condition, { kind: "absent" });
    const restarted = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: browserFetch(started.baseUrl, "session-a"),
    });
    assert.deepEqual(restarted.chapter, before.chapter);
  } finally {
    await stopRealServer(started.server);
  }
});
