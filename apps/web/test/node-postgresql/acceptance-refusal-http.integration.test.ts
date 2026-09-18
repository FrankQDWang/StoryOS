import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { test } from "vitest";
import { acceptProposal, createEditorSession, digestAcceptProposal, digestCreateEditorSession,
  digestTakeOverProjectWriter, getProposal, takeOverProjectWriter, digestExportProjectArchive, exportProjectArchive, getExportOperation,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, id, startRealServer, drainLeftoverWork, challenged, prepare,
  admitProse, reviseCandidate, settleOnce, USER_A } from "../support/acceptance.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { queryStoryOSPostgres, sessionFetch, requireStoryOSProtocolError, stopStoryOSServer } from "../support/node-integration.ts";

test("Acceptance retains a stale-writer refusal without changing Proposal validity", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const project = await prepare(started.baseUrl, id("f7300111"), "Refusal Evidence Novel", "e1");
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
      assert.deepEqual((await getProposal(options)).proposal, revised.proposal);
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
    assert.deepEqual(refused.proposal, { ...revised.proposal, latest_acceptance_refusal: refusal });
    await Promise.all([1, 2].map(() => assert.rejects(
      () => acceptProposal({ ...options, request, idempotencyKey: key, antiForgery: nonce }),
      (error) => requireStoryOSProtocolError(error).status === 409)));
    assert.deepEqual((await getProposal(options)).proposal, refused.proposal);
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
    assert.deepEqual((await getProposal(reloaded)).proposal, refused.proposal);
    const accepted = await challenged(started.baseUrl, reloaded.fetchImpl, project.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", request.command_schema,
      digest, id("e172"),
      (antiForgery) => acceptProposal({ ...reloaded, request, idempotencyKey: id("e172"), antiForgery }));
    assert.equal(accepted.effect.kind, "applied");
    assert.deepEqual((await getProposal(reloaded)).proposal.latest_acceptance_refusal, refusal);
    assert.deepEqual(JSON.parse(await state()), { admissions: 1, receipts: 1, refusals: 1, consumed: 1 });
    await assert.rejects(() => acceptProposal({ ...reloaded, request, idempotencyKey: id("e181"), antiForgery: nonce }),
      (error) => requireStoryOSProtocolError(error).status === 422);
    assert.deepEqual((await getProposal(reloaded)).proposal.latest_acceptance_refusal, refusal);
    const raceKey = id("e182");
    const raceNonce = await challenged(started.baseUrl, reloaded.fetchImpl, project.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", request.command_schema,
      digest, raceKey, async (antiForgery) => antiForgery);
    const lock = spawn("docker", ["exec", "-i", process.env.STORYOS_TEST_POSTGRES_CONTAINER ?? "storyos-dev-postgres",
      "psql", "-X", "-qAt", "-U", "postgres", "-v", "ON_ERROR_STOP=1"], { stdio: ["pipe", "pipe", "pipe"] });
    lock.stdin.write("SELECT pg_advisory_lock(730); SELECT 'locked';\n");
    let output = "";
    while (!output.includes("locked")) output += (await once(lock.stdout, "data"))[0].toString();
    await queryStoryOSPostgres(`CREATE FUNCTION storyos.test_refusal_pause() RETURNS trigger LANGUAGE plpgsql AS
      'BEGIN PERFORM pg_advisory_xact_lock(730); RETURN NEW; END';
      CREATE TRIGGER test_refusal_pause BEFORE INSERT ON storyos.acceptance_refusals
      FOR EACH ROW EXECUTE FUNCTION storyos.test_refusal_pause();`);
    const rejected = assert.rejects(() => acceptProposal({ ...reloaded, request, idempotencyKey: raceKey, antiForgery: "0".repeat(64) }),
      (error) => requireStoryOSProtocolError(error).status === 422);
    const waiting = async (queryFragment: string) => {
      for (let attempt = 0; attempt < 100; attempt += 1) {
        if (await queryStoryOSPostgres(`SELECT count(*) FROM pg_stat_activity WHERE usename = 'storyos_runtime'
          AND wait_event_type = 'Lock' AND query LIKE '%${queryFragment}%'`) !== "0") return;
      }
      throw new Error(`expected blocked query: ${queryFragment}`);
    };
    try {
      await waiting("INSERT INTO storyos.acceptance_refusals");
      const competing = assert.rejects(() => acceptProposal({ ...reloaded, request, idempotencyKey: raceKey, antiForgery: raceNonce }),
        (error) => requireStoryOSProtocolError(error).status === 503);
      await waiting("project_command_challenges");
      lock.stdin.end("SELECT pg_advisory_unlock(730);\n");
      await Promise.all([rejected, competing]);
    } finally {
      lock.stdin.end();
      await queryStoryOSPostgres("DROP TRIGGER test_refusal_pause ON storyos.acceptance_refusals; DROP FUNCTION storyos.test_refusal_pause();");
    }
    await assert.rejects(() => acceptProposal({ ...reloaded, request, idempotencyKey: raceKey, antiForgery: raceNonce }),
      (error) => requireStoryOSProtocolError(error).status === 422);
    const invalid = (await getProposal(reloaded)).proposal.latest_acceptance_refusal;
    if (invalid.kind !== "present") throw new Error("expected challenge refusal");
    assert.deepEqual(invalid, { ...refusal, refusal_id: invalid.refusal_id, recorded_at: invalid.recorded_at,
      reason: "invalid_challenge", boundary: "challenge" });
    assert.deepEqual(JSON.parse(await state()), { admissions: 1, receipts: 1, refusals: 2, consumed: 1 });
    for (const [handle, status] of [["session-b", 422], [undefined, 401]] as const) {
      await assert.rejects(() => acceptProposal({ ...reloaded, fetchImpl: sessionFetch(started.baseUrl, handle),
        request, idempotencyKey: raceKey, antiForgery: raceNonce }),
      (error) => requireStoryOSProtocolError(error).status === status);
    }
    const otherSession = sessionFetch(started.baseUrl, "session-c");
    await assert.rejects(() => challenged(started.baseUrl, otherSession, project.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", request.command_schema,
      digest, id("e183"), (antiForgery) => acceptProposal({ ...reloaded, fetchImpl: otherSession,
        request, idempotencyKey: id("e183"), antiForgery })),
      (error) => requireStoryOSProtocolError(error).status === 409);
    const changed = (await getProposal(reloaded)).proposal.latest_acceptance_refusal;
    if (changed.kind !== "present") throw new Error("expected session refusal");
    assert.deepEqual(changed, { ...refusal, refusal_id: changed.refusal_id, recorded_at: changed.recorded_at,
      reason: "session_changed" });
    assert.deepEqual(JSON.parse(await state()), { admissions: 1, receipts: 1, refusals: 3, consumed: 1 });

    const archiveRequest = { command_schema: "storyos.command.export-project-archive.request.v1" as const,
      export_project_archive_input: { ...BINDING, correlation_id: id("e191"),
        archive_profile: "storyos.project-export.v1", archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" } };
    const archive = await challenged(started.baseUrl, reloaded.fetchImpl, project.projectId, "POST",
      "/api/v1/projects/{project_id}/exports", archiveRequest.command_schema,
      await digestExportProjectArchive(archiveRequest), id("e192"),
      (antiForgery) => exportProjectArchive({ ...reloaded, request: archiveRequest, idempotencyKey: id("e192"), antiForgery }));
    if (archive.effect.kind !== "admitted") throw new Error("expected export admission");
    await settleOnce();
    assert.equal((await getExportOperation({ ...reloaded, exportId: archive.effect.export_id })).status, "ready");
    const download = await reloaded.fetchImpl(`${started.baseUrl}/api/v1/projects/${project.projectId}/exports/${archive.effect.export_id}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
    assert.equal(download.status, 200);
    const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
    const archived = JSON.parse(new TextDecoder().decode(files.get("canonical/acceptance_refusals.json")));
    const rows = JSON.parse(await queryStoryOSPostgres(`SELECT json_agg(r ORDER BY refusal_id)::text
      FROM storyos.acceptance_refusals r WHERE project_id = '${project.projectId}'`));
    assert.deepEqual(archived.sort((left: { refusal_id: string }, right: { refusal_id: string }) => left.refusal_id.localeCompare(right.refusal_id)), rows);
    assert.deepEqual(Object.keys(rows[0]).sort(), ["owner_user_id", "project_id", "proposal_id", "refusal_id",
      "idempotency_key", "correlation_id", "command_kind", "boundary", "command_schema", "refusal_profile_revision",
      "reason", "client_contract_revision", "security_policy_revision", "limit_profile_revision",
      "challenge_rate_policy_revision", "recorded_at"].sort());
    assert.equal(rows[0].owner_user_id, USER_A);
    for (const secret of [nonce, "session-a", "session-c", "invalid-nonce", session.editor_session.editor_session_id]) {
      assert.equal(JSON.stringify(archived).includes(secret), false);
    }
    const size = await queryStoryOSPostgres(`SELECT json_build_object('max_row_bytes', max(pg_column_size(r)),
      'max_json_bytes', max(octet_length(row_to_json(r)::text)))::text
      FROM storyos.acceptance_refusals r WHERE project_id = '${project.projectId}'`);
    console.log(`Acceptance refusal measured size: ${size}`);


  } finally {
    await stopStoryOSServer(started.server);
  }
});
