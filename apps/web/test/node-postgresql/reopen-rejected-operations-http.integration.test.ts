// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/reject-proposal-operations-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { createEditorSession, digestCreateEditorSession, digestExportProjectArchive,
  digestRejectProposalOperations, digestReopenRejectedOperations, exportProjectArchive,
  getChapter, getExportOperation, getProposal, rejectProposalOperations,
  reopenRejectedOperations,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { RejectProposalOperationsRequest, ReopenRejectedOperationsRequest
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, requireStoryOSProtocolError,
  sessionFetch as browserFetch, stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { USER_A, UUID_V7, BINDING, id, startRealServer, settleOnce,
  drainLeftoverWork, challenged, prepare, admitProse } from "../support/acceptance.ts";

test("reopenRejectedOperations appends a pending Revision without rewriting Rejection", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("f7330111"), "Reopen Proposal Novel", "f7332");
    const before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const queried = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("f7330131"));
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
    const sessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1" as const,
      ...BINDING,
      correlation_id: id("f7330141"),
    };
    const session = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/editor-sessions",
      sessionRequest.command_schema,
      await digestCreateEditorSession(sessionRequest),
      id("f7330142"),
      (antiForgery) => createEditorSession({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f7330142"),
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
        correlation_id: id("f7330151"),
      },
    };
    const rejected = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
      rejectRequest.command_schema,
      await digestRejectProposalOperations(rejectRequest),
      id("f7330152"),
      (antiForgery) => rejectProposalOperations({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f7330152"),
        antiForgery,
        request: rejectRequest,
      }),
    );
    assert.equal(rejected.effect.kind, "resolved");
    if (rejected.effect.kind !== "resolved") throw new Error("expected rejected");
    const resolutionEventId = rejected.effect.resolution_event_refs[0] ?? "";
    const afterReject = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(afterReject.proposal.operation_resolution, "rejected");
    assert.equal(afterReject.proposal.revision_id, opened.proposal.revision_id);
    const reopenRequest: ReopenRejectedOperationsRequest = {
      command_schema: "storyos.command.reopen-rejected-operations.request.v1",
      reopen_rejected_operations_input: {
        proposal_revision_id: opened.proposal.revision_id,
        selected_rejected_operation_ids: [opened.proposal.operation_id],
        rejection_event_refs: [resolutionEventId],
        expected_target_revisions: [before.chapter.current_revision.revision_id],
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING,
        correlation_id: id("f7330161"),
      },
    };
    const reopenOptions = {
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("f7330162"),
      antiForgery: "",
      request: reopenRequest,
    };
    const unavailable: ReopenRejectedOperationsRequest = {
      command_schema: "storyos.command.reopen-rejected-operations.request.v1",
      reopen_rejected_operations_input: {
        ...reopenRequest.reopen_rejected_operations_input,
        rejection_event_refs: [id("f7330153")],
        correlation_id: id("f7330154"),
      },
    };
    const unavailableProof = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/operation-reopenings",
      unavailable.command_schema,
      await digestReopenRejectedOperations(unavailable),
      id("f7330155"),
      (antiForgery) => reopenRejectedOperations({
        ...reopenOptions,
        idempotencyKey: id("f7330155"),
        antiForgery,
        request: unavailable,
      }),
    );
    assert.deepEqual(unavailableProof.effect, { kind: "refused", reason: "unavailable_proof" });
    assert.equal((await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    })).proposal.operation_resolution, "rejected");
    const reopened = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/operation-reopenings",
      reopenRequest.command_schema,
      await digestReopenRejectedOperations(reopenRequest),
      id("f7330162"),
      async (antiForgery) => {
        await queryPostgres(`CREATE FUNCTION storyos.test_reopen_failure() RETURNS trigger
          LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'reopen fault'; END $$;
          CREATE TRIGGER test_reopen_failure BEFORE INSERT ON storyos.proposal_operation_reopenings
          FOR EACH ROW EXECUTE FUNCTION storyos.test_reopen_failure();`);
        try {
          await assert.rejects(
            () => reopenRejectedOperations({ ...reopenOptions, antiForgery }),
            (error) => requireStoryOSProtocolError(error).status === 503,
          );
          assert.equal((await getProposal({
            baseUrl: started.baseUrl,
            projectId: prepared.projectId,
            proposalId: opened.proposal.proposal_id,
            fetchImpl: prepared.fetchImpl,
          })).proposal.operation_resolution, "rejected");
        } finally {
          await queryPostgres(`DROP TRIGGER test_reopen_failure ON storyos.proposal_operation_reopenings;
            DROP FUNCTION storyos.test_reopen_failure();`);
        }
        reopenOptions.antiForgery = antiForgery;
        return reopenRejectedOperations(reopenOptions);
      },
    );
    assert.equal(reopened.effect.kind, "resolved");
    assert.equal(reopened.receipt.result, "resolved");
    if (reopened.effect.kind !== "resolved") throw new Error("expected resolved");
    assert.equal(reopened.effect.undo_disposition, "forward");
    assert.equal(reopened.effect.prior_resolution, "rejected");
    assert.equal(reopened.effect.resulting_resolution, "pending");
    assert.equal(reopened.effect.resulting_validation, "pending");
    assert.deepEqual(reopened.effect.operation_ids, [opened.proposal.operation_id]);
    assert.deepEqual(reopened.effect.rejection_event_refs, [resolutionEventId]);
    assert.match(reopened.effect.author_action_sequence, /^\d+$/);
    assert.equal(reopened.effect.state_event_refs.length, 1);
    const reopenEventId = reopened.effect.state_event_refs[0] ?? "";
    assert.match(reopenEventId, UUID_V7);
    assert.notEqual(reopened.effect.resulting_proposal_revision_id, opened.proposal.revision_id);
    assert.equal(reopened.receipt.source_proposal_revision_id, opened.proposal.revision_id);
    assert.equal(reopened.receipt.resulting_proposal_revision_id, reopened.effect.resulting_proposal_revision_id);
    assert.deepEqual(reopened.receipt.authoritative_commit_ids, []);
    assert.deepEqual(await reopenRejectedOperations(reopenOptions), reopened);
    const inspected = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(inspected.proposal.operation_resolution, "pending");
    assert.equal(inspected.proposal.reservation_state, "unresolved");
    assert.equal(inspected.proposal.validation, "pending");
    assert.equal(inspected.proposal.revision_id, reopened.effect.resulting_proposal_revision_id);
    assert.deepEqual(inspected.proposal.validation_receipt, { kind: "absent" });
    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(after.chapter, before.chapter);
    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    const reopenDigest = await digestReopenRejectedOperations(reopenRequest);
    await assert.rejects(
      () => challenged(
        started.baseUrl,
        foreignFetch,
        prepared.projectId,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/operation-reopenings",
        reopenRequest.command_schema,
        reopenDigest,
        id("f7330163"),
        (antiForgery) => reopenRejectedOperations({
          ...reopenOptions,
          fetchImpl: foreignFetch,
          idempotencyKey: id("f7330163"),
          antiForgery,
        }),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    const stale: ReopenRejectedOperationsRequest = {
      command_schema: "storyos.command.reopen-rejected-operations.request.v1",
      reopen_rejected_operations_input: {
        ...reopenRequest.reopen_rejected_operations_input,
        proposal_revision_id: opened.proposal.revision_id,
        correlation_id: id("f7330171"),
      },
    };
    const refused = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/operation-reopenings",
      stale.command_schema,
      await digestReopenRejectedOperations(stale),
      id("f7330172"),
      (antiForgery) => reopenRejectedOperations({
        ...reopenOptions,
        idempotencyKey: id("f7330172"),
        antiForgery,
        request: stale,
      }),
    );
    assert.deepEqual(refused.effect, { kind: "refused", reason: "stale_proposal_revision" });
    const archiveRequest = {
      command_schema: "storyos.command.export-project-archive.request.v1" as const,
      export_project_archive_input: {
        ...BINDING,
        correlation_id: id("f7330180"),
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
      id("f7330181"),
      (antiForgery) => exportProjectArchive({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        request: archiveRequest,
        idempotencyKey: id("f7330181"),
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
    const reopenings = JSON.parse(new TextDecoder().decode(files.get("canonical/proposal_operation_reopenings.json")));
    assert.deepEqual(reopenings, [{
      owner_user_id: USER_A,
      project_id: prepared.projectId,
      reopen_event_id: reopenEventId,
      proposal_id: opened.proposal.proposal_id,
      source_proposal_revision_id: opened.proposal.revision_id,
      resulting_proposal_revision_id: reopened.effect.resulting_proposal_revision_id,
      operation_id: opened.proposal.operation_id,
      rejection_event_id: resolutionEventId,
      prior_resolution: "rejected",
      resulting_resolution: "pending",
      reopen_receipt_id: reopened.receipt.receipt_id,
      author_action_sequence: Number(reopened.effect.author_action_sequence),
    }]);
    await stopRealServer(started.server);
    started = await startRealServer();
    const reloaded = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: browserFetch(started.baseUrl, "session-a"),
    });
    assert.equal(reloaded.proposal.operation_resolution, "pending");
    assert.equal(reloaded.proposal.validation, "pending");
    assert.equal(reloaded.proposal.revision_id, reopened.effect.resulting_proposal_revision_id);
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
