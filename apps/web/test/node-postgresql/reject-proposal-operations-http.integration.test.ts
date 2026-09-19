import assert from "node:assert/strict";
import { test } from "vitest";
import { createAgentRun, createEditorSession, digestCreateAgentRun,
  digestCreateEditorSession, digestExportProjectArchive, digestRejectProposalOperations,
  exportProjectArchive, getAgentRun, getChapter, getExportOperation, getProposal,
  rejectProposalOperations,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { RejectProposalOperationsRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, requireStoryOSProtocolError,
  sessionFetch as browserFetch, stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { USER_A, UUID_V7, BINDING, id, startRealServer, settleOnce,
  drainLeftoverWork, challenged, prepare, admitProse } from "../support/acceptance.ts";

test("rejectProposalOperations freezes one pending Operation without changing authority", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("f111"), "Reject Proposal Novel", "f2");
    const before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const queried = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("f131"));
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
    assert.equal(opened.proposal.operation_resolution, "pending");
    const discussion = {
      command_schema: "storyos.command.create-agent-run.request.v2" as const,
      create_agent_run_input: {
        conversation: { kind: "new" as const },
        author_message: { text: "The candidate feels too formal." },
        working_target: { kind: "current_chapter" as const, chapter_id: prepared.chapterId },
        instruction: { kind: "absent" as const },
        cause: { kind: "author_request" as const },
        ...BINDING,
        correlation_id: id("f133"),
      },
    };
    const discussed = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/agent-runs",
      discussion.command_schema,
      await digestCreateAgentRun(discussion),
      id("f132"),
      (antiForgery) => createAgentRun({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f132"),
        antiForgery,
        request: discussion,
      }),
    );
    if (discussed.effect.kind !== "admitted") throw new Error("expected admitted discussion");
    await settleOnce();
    const feedback = await getAgentRun({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      runId: discussed.effect.run_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(feedback.decision.kind, "advisory");
    const afterFeedback = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(afterFeedback.proposal.operation_resolution, "pending");
    const sessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1" as const,
      ...BINDING,
      correlation_id: id("f141"),
    };
    const session = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/editor-sessions",
      sessionRequest.command_schema,
      await digestCreateEditorSession(sessionRequest),
      id("f142"),
      (antiForgery) => createEditorSession({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f142"),
        antiForgery,
        request: sessionRequest,
      }),
    );
    const rejectRequest: RejectProposalOperationsRequest = {
      command_schema: "storyos.command.reject-proposal-operations.request.v1",
      reject_proposal_operations_input: {
        proposal_revision_id: opened.proposal.revision_id,
        selected_pending_operation_ids: [opened.proposal.operation_id],
        expected_target_revisions: [before.chapter.current_revision.revision_id],
        rejection_reason: { kind: "author_declined", note: { kind: "omitted" } },
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING,
        correlation_id: id("f151"),
      },
    };
    const rejectOptions = {
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("f152"),
      antiForgery: "",
      request: rejectRequest,
    };
    const rejected = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
      rejectRequest.command_schema,
      await digestRejectProposalOperations(rejectRequest),
      id("f152"),
      async (antiForgery) => {
        await queryPostgres(`CREATE FUNCTION storyos.test_rejection_failure() RETURNS trigger
          LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'rejection fault'; END $$;
          CREATE TRIGGER test_rejection_failure BEFORE INSERT ON storyos.proposal_operation_resolutions
          FOR EACH ROW EXECUTE FUNCTION storyos.test_rejection_failure();`);
        try {
          await assert.rejects(
            () => rejectProposalOperations({ ...rejectOptions, antiForgery }),
            (error) => requireStoryOSProtocolError(error).status === 503,
          );
          assert.equal((await getProposal({
            baseUrl: started.baseUrl,
            projectId: prepared.projectId,
            proposalId: opened.proposal.proposal_id,
            fetchImpl: prepared.fetchImpl,
          })).proposal.operation_resolution, "pending");
        } finally {
          await queryPostgres(`DROP TRIGGER test_rejection_failure ON storyos.proposal_operation_resolutions;
            DROP FUNCTION storyos.test_rejection_failure();`);
        }
        rejectOptions.antiForgery = antiForgery;
        return rejectProposalOperations(rejectOptions);
      },
    );
    assert.equal(rejected.effect.kind, "resolved");
    assert.equal(rejected.receipt.result, "resolved");
    if (rejected.effect.kind !== "resolved") throw new Error("expected resolved");
    assert.equal(rejected.effect.undo_disposition, "forward");
    assert.equal(rejected.effect.prior_resolution, "pending");
    assert.equal(rejected.effect.resulting_resolution, "rejected");
    assert.deepEqual(rejected.effect.operation_ids, [opened.proposal.operation_id]);
    assert.equal(rejected.effect.rejection_reason.kind, "author_declined");
    assert.match(rejected.effect.author_action_sequence, /^\d+$/);
    assert.equal(rejected.effect.resolution_event_refs.length, 1);
    const resolutionEventId = rejected.effect.resolution_event_refs[0] ?? "";
    assert.match(resolutionEventId, UUID_V7);
    assert.deepEqual(rejected.receipt.authoritative_commit_ids, []);
    assert.deepEqual(await rejectProposalOperations(rejectOptions), rejected);
    const inspected = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(inspected.proposal.operation_resolution, "rejected");
    assert.equal(inspected.proposal.reservation_state, "resolved");
    assert.equal(inspected.proposal.revision_id, opened.proposal.revision_id);
    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(after.chapter, before.chapter);
    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    const rejectDigest = await digestRejectProposalOperations(rejectRequest);
    await assert.rejects(
      () => challenged(
        started.baseUrl,
        foreignFetch,
        prepared.projectId,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
        rejectRequest.command_schema,
        rejectDigest,
        id("f153"),
        (antiForgery) => rejectProposalOperations({
          ...rejectOptions,
          fetchImpl: foreignFetch,
          idempotencyKey: id("f153"),
          antiForgery,
        }),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    const wrongAdmission: RejectProposalOperationsRequest = {
      command_schema: "storyos.command.reject-proposal-operations.request.v1",
      reject_proposal_operations_input: {
        ...rejectRequest.reject_proposal_operations_input,
        editor_session_id: id("f160"),
        correlation_id: id("f161"),
      },
    };
    const wrongAdmissionDigest = await digestRejectProposalOperations(wrongAdmission);
    await assert.rejects(
      () => challenged(
        started.baseUrl,
        prepared.fetchImpl,
        prepared.projectId,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
        wrongAdmission.command_schema,
        wrongAdmissionDigest,
        id("f162"),
        (antiForgery) => rejectProposalOperations({
          ...rejectOptions,
          idempotencyKey: id("f162"),
          antiForgery,
          request: wrongAdmission,
        }),
      ),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );
    const second: RejectProposalOperationsRequest = {
      command_schema: "storyos.command.reject-proposal-operations.request.v1",
      reject_proposal_operations_input: {
        ...rejectRequest.reject_proposal_operations_input,
        correlation_id: id("f171"),
      },
    };
    const refused = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
      second.command_schema,
      await digestRejectProposalOperations(second),
      id("f172"),
      (antiForgery) => rejectProposalOperations({
        ...rejectOptions,
        idempotencyKey: id("f172"),
        antiForgery,
        request: second,
      }),
    );
    assert.deepEqual(refused.effect, { kind: "refused", reason: "operation_not_pending" });
    const archiveRequest = {
      command_schema: "storyos.command.export-project-archive.request.v1" as const,
      export_project_archive_input: {
        ...BINDING,
        correlation_id: id("f180"),
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
      id("f181"),
      (antiForgery) => exportProjectArchive({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        request: archiveRequest,
        idempotencyKey: id("f181"),
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
    const receipts = JSON.parse(new TextDecoder().decode(files.get("canonical/proposal_rejection_receipts.json")));
    assert.deepEqual(receipts.find((row: { result: string }) => row.result === "proposal_operations_resolved"), {
      owner_user_id: USER_A,
      project_id: prepared.projectId,
      rejection_receipt_id: rejected.receipt.receipt_id,
      proposal_id: opened.proposal.proposal_id,
      proposal_revision_id: opened.proposal.revision_id,
      selected_operation_id: opened.proposal.operation_id,
      result: "proposal_operations_resolved",
      rejection_reason: "author_declined",
      rejection_note: null,
    });
    const resolutions = JSON.parse(new TextDecoder().decode(files.get("canonical/proposal_operation_resolutions.json")));
    assert.deepEqual(resolutions, [{
      owner_user_id: USER_A,
      project_id: prepared.projectId,
      resolution_event_id: resolutionEventId,
      proposal_id: opened.proposal.proposal_id,
      proposal_revision_id: opened.proposal.revision_id,
      operation_id: opened.proposal.operation_id,
      prior_resolution: "pending",
      resulting_resolution: "rejected",
      rejection_receipt_id: rejected.receipt.receipt_id,
      author_action_sequence: Number(rejected.effect.author_action_sequence),
    }]);
    await stopRealServer(started.server);
    started = await startRealServer();
    const reloaded = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: browserFetch(started.baseUrl, "session-a"),
    });
    assert.equal(reloaded.proposal.operation_resolution, "rejected");
    assert.equal(reloaded.proposal.reservation_state, "resolved");
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
