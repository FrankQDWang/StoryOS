import assert from "node:assert/strict";
import { test } from "vitest";
import { acceptProposal, createEditorSession, digestAcceptProposal, digestCreateEditorSession,
  digestTakeOverProjectWriter, getProposal, takeOverProjectWriter,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, id, startRealServer, drainLeftoverWork, challenged, prepare,
  admitProse, reviseCandidate } from "../support/acceptance.ts";
import { queryStoryOSPostgres, sessionFetch, requireStoryOSProtocolError, stopStoryOSServer } from "../support/node-integration.ts";

test("Acceptance retains a stale-writer refusal without changing Proposal validity", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const project = await prepare(started.baseUrl, id("e111"), "Refusal Evidence Novel", "e1");
    const base = { baseUrl: started.baseUrl, projectId: project.projectId, fetchImpl: project.fetchImpl };
    const run = await admitProse(started.baseUrl, project.fetchImpl, project.projectId, project.chapterId, id("e121"));
    if (run.decision.kind !== "prose_change" || run.decision.opened_proposal.kind !== "present") throw new Error("expected proposal");
    const options = { ...base, proposalId: run.decision.opened_proposal.proposal_id };
    const opened = await getProposal(options);
    const { session, revised } = await reviseCandidate(started.baseUrl, project.fetchImpl, project.projectId, opened, "e13");
    if (session.writer.kind !== "current_writer" || revised.proposal.validation_receipt.kind !== "present") throw new Error("expected validated writer");
    const openRequest = { command_schema: "storyos.command.create-editor-session.request.v1", ...BINDING, correlation_id: id("e141") };
    const observer = await challenged(started.baseUrl, project.fetchImpl, project.projectId, "POST",
      "/api/v1/projects/{project_id}/editor-sessions", openRequest.command_schema,
      await digestCreateEditorSession(openRequest), id("e142"),
      (antiForgery) => createEditorSession({ ...base, request: openRequest, idempotencyKey: id("e142"), antiForgery }));
    const takeoverRequest = { command_schema: "storyos.command.take-over-project-writer.request.v1", ...BINDING,
      correlation_id: id("e143"), editor_session_id: observer.editor_session.editor_session_id,
      observed_writer_generation: session.writer.writer_generation, editor_contract_revision: "storyos.editor-contract.release-1.v2" };
    const takeover = await challenged(started.baseUrl, project.fetchImpl, project.projectId, "POST",
      "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers", takeoverRequest.command_schema,
      await digestTakeOverProjectWriter(takeoverRequest), id("e144"),
      (antiForgery) => takeOverProjectWriter({ ...base, editorSessionId: observer.editor_session.editor_session_id,
        request: takeoverRequest, idempotencyKey: id("e144"), antiForgery }));
    const request = { command_schema: "storyos.command.accept-proposal.request.v1", accept_proposal_input: {
      ...BINDING, correlation_id: id("e151"), editor_session_id: session.editor_session.editor_session_id,
      proposal_revision_id: revised.proposal.revision_id,
      validation_receipt_id: revised.proposal.validation_receipt.validation_receipt_id,
      selected_operation_id: revised.proposal.operation_id,
      expected_authoritative_revision_id: revised.proposal.base_authoritative_revision_id,
    } };
    const digest = await digestAcceptProposal(request);
    const key = id("e152");
    let nonce = "";
    const send = () => challenged(started.baseUrl, project.fetchImpl, project.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", request.command_schema,
      digest, key, (antiForgery) => {
        nonce = antiForgery;
        return acceptProposal({ ...options, request, idempotencyKey: key, antiForgery });
      });
    const state = () => queryStoryOSPostgres(`SELECT json_build_object(
      'admissions', (SELECT count(*) FROM storyos.author_command_admissions WHERE project_id = '${project.projectId}' AND command_kind = 'acceptProposal'),
      'receipts', (SELECT count(*) FROM storyos.acceptance_receipts WHERE project_id = '${project.projectId}'),
      'refusals', (SELECT count(*) FROM storyos.acceptance_refusals WHERE project_id = '${project.projectId}'),
      'consumed', (SELECT count(*) FROM storyos.project_command_challenges WHERE project_id = '${project.projectId}' AND command_kind = 'acceptProposal' AND consumed_at IS NOT NULL)
    )::text`);
    const empty = { admissions: 0, receipts: 0, refusals: 0, consumed: 0 };
    await queryStoryOSPostgres(`CREATE FUNCTION storyos.test_refusal_fault() RETURNS trigger LANGUAGE plpgsql AS
      'BEGIN RAISE EXCEPTION ''refusal storage fault''; END';
      CREATE TRIGGER test_refusal_fault BEFORE INSERT ON storyos.acceptance_refusals
      FOR EACH ROW EXECUTE FUNCTION storyos.test_refusal_fault();`);
    try {
      await assert.rejects(send, (error) => requireStoryOSProtocolError(error).status === 503);
      assert.deepEqual(JSON.parse(await state()), empty);
      assert.deepEqual(await getProposal(options), revised);
    } finally {
      await queryStoryOSPostgres("DROP TRIGGER test_refusal_fault ON storyos.acceptance_refusals; DROP FUNCTION storyos.test_refusal_fault();");
    }
    await assert.rejects(send, (error) => requireStoryOSProtocolError(error).status === 409);
    const refused = await getProposal(options);
    const refusal = refused.proposal.latest_acceptance_refusal;
    if (refusal.kind !== "present") throw new Error("expected durable refusal");
    assert.deepEqual(refusal, {
      kind: "present", refusal_id: refusal.refusal_id, correlation_id: id("e151"),
      reason: "stale_writer", boundary: "writer_session", command_schema: request.command_schema,
      refusal_profile_revision: "storyos.acceptance-refusal.fixed-fields.v1", ...BINDING,
      limit_profile_revision: "storyos.foundation.absolute.v1",
      challenge_rate_policy_revision: "storyos.project-command-challenge-rate.fixed-window.v1",
      recorded_at: refusal.recorded_at,
    });
    assert.deepEqual(refused, { ...revised, proposal: { ...revised.proposal, latest_acceptance_refusal: refusal } });
    await Promise.all([1, 2].map(() => assert.rejects(
      () => acceptProposal({ ...options, request, idempotencyKey: key, antiForgery: nonce }),
      (error) => requireStoryOSProtocolError(error).status === 409)));
    assert.deepEqual(await getProposal(options), refused);
    assert.deepEqual(JSON.parse(await state()), { ...empty, refusals: 1 });
    await assert.rejects(() => getProposal({ ...options, fetchImpl: sessionFetch(started.baseUrl, "session-b") }),
      (error) => requireStoryOSProtocolError(error).status === 404);
    await assert.rejects(() => getProposal({ ...options, fetchImpl: sessionFetch(started.baseUrl) }),
      (error) => requireStoryOSProtocolError(error).status === 401);
    if (takeover.result.kind !== "takeover_applied") throw new Error("expected takeover");
    const restore = { ...takeoverRequest, correlation_id: id("e161"),
      editor_session_id: session.editor_session.editor_session_id,
      observed_writer_generation: takeover.result.resulting_writer_generation };
    await challenged(started.baseUrl, project.fetchImpl, project.projectId, "POST",
      "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers", restore.command_schema,
      await digestTakeOverProjectWriter(restore), id("e162"),
      (antiForgery) => takeOverProjectWriter({ ...base, editorSessionId: restore.editor_session_id,
        request: restore, idempotencyKey: id("e162"), antiForgery }));
    await assert.rejects(send, (error) => requireStoryOSProtocolError(error).status === 409);
    assert.deepEqual(JSON.parse(await state()), { ...empty, refusals: 1 });
    await stopStoryOSServer(started.server);
    started = await startRealServer();
    const reloaded = { ...options, baseUrl: started.baseUrl, fetchImpl: sessionFetch(started.baseUrl, "session-a") };
    assert.deepEqual(await getProposal(reloaded), refused);
    const accepted = await challenged(started.baseUrl, reloaded.fetchImpl, project.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", request.command_schema,
      digest, id("e172"),
      (antiForgery) => acceptProposal({ ...reloaded, request, idempotencyKey: id("e172"), antiForgery }));
    assert.equal(accepted.effect.kind, "applied");
    assert.deepEqual((await getProposal(reloaded)).proposal.latest_acceptance_refusal, refusal);
    assert.deepEqual(JSON.parse(await state()), { admissions: 1, receipts: 1, refusals: 1, consumed: 1 });

  } finally {
    await stopStoryOSServer(started.server);
  }
});
