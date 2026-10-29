// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/reject-proposal-operations-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { test } from "vitest";
import { acceptProposal, createEditorSession, digestAcceptProposal, digestCreateEditorSession,
  digestTakeOverProjectWriter, getProposal, takeOverProjectWriter, digestExportProjectArchive, exportProjectArchive, getExportOperation,
  getChapter,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, id, drainLeftoverWork, challenged, prepare,
  admitProse, reviseCandidate, seedAcceptanceCondition, settleOnce, USER_A } from "../support/acceptance.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { queryStoryOSPostgres, sessionFetch, requireStoryOSProtocolError, stopStoryOSServer } from "../support/node-integration.ts";
import { TestServer, UUID_V7, archivedTables, createStory, freshNamespace, openProposal, openedProposal, problemOf,
  rejection, startServer, waitFor } from "../support/http-harness.ts";
import type { CommandInput } from "../support/http-harness.ts";

test("Acceptance retains a stale-writer refusal without changing Proposal validity", async () => {
  let started = await startServer();
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
      observed_writer_generation: session.writer.writer_generation, editor_contract_revision: "storyos.editor-contract.release-1.v3" };
    const takeover = await challenged(started.baseUrl, project.fetchImpl, project.projectId, "POST",
      "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers", takeoverRequest.command_schema,
      await digestTakeOverProjectWriter(takeoverRequest), id("e144"),
      (antiForgery) => takeOverProjectWriter({ ...base, editorSessionId: observer.editor_session.editor_session_id,
        request: takeoverRequest, idempotencyKey: id("e144"), antiForgery }));
    const request = { command_schema: "storyos.command.accept-proposal.request.v1", accept_proposal_input: {
      ...BINDING, correlation_id: id("e151"), editor_session_id: session.editor_session.editor_session_id,
      proposal_revision_id: revised.proposal.revision_id,
      validation_receipt_id: revised.proposal.validation_receipt.validation_receipt_id,
      selected_operation_ids: [revised.proposal.operation_id],
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
    started = await startServer();
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
    const waiting = (queryFragment: string) => waitFor(() => queryStoryOSPostgres(`SELECT count(*) FROM pg_stat_activity
      WHERE usename = 'storyos_runtime' AND wait_event_type = 'Lock' AND query LIKE '%${queryFragment}%'`),
    (count) => count !== "0", { attempts: 100, intervalMs: 0 });
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

// The recovery seed runs this test: it retains one altered-candidate Acceptance condition. No Rust adapter test
// alters a stored candidate, so this test changes the row with SQL.
test("acceptProposal retains altered_candidate across reload and refuses another attempt until revision", async () => {
  const server = await TestServer.start();
  try {
    await drainLeftoverWork();
    const story = await createStory(server, { title: "Altered Acceptance Novel", assistance: true });
    const chapter = async () => (await getChapter({ baseUrl: story.baseUrl, projectId: story.projectId,
      chapterId: story.chapterIds[0] ?? "", fetchImpl: story.fetchImpl })).chapter;
    const before = await chapter();
    const opened = await openProposal(story);
    const { session, revised } = await reviseCandidate(story.baseUrl, story.fetchImpl, story.projectId, opened,
      freshNamespace());
    const proposal = revised.proposal;
    if (proposal.validation_receipt.kind !== "present") throw new Error("expected a Validation Receipt");
    const tampered = "Tampered narrator voice.";
    await queryStoryOSPostgres(`UPDATE storyos.proposal_revisions SET candidate_text = '${tampered}'
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${story.projectId}'::uuid
        AND revision_id = '${proposal.revision_id}'::uuid`);
    const acceptance = { request: {
      command_schema: "storyos.command.accept-proposal.request.v1" as const,
      accept_proposal_input: {
        proposal_revision_id: proposal.revision_id,
        validation_receipt_id: proposal.validation_receipt.validation_receipt_id,
        selected_operation_ids: [proposal.operation_id],
        expected_authoritative_revision_id: before.current_revision.revision_id,
        editor_session_id: session.editor_session.editor_session_id,
        ...story.binding(),
      },
    }, path: { proposalId: proposal.proposal_id } };
    const failed = await story.send("acceptProposal", acceptance);
    assert.deepEqual(failed.response.effect, { kind: "invalid", reason: "altered_candidate" });
    assert.deepEqual(failed.response.receipt.condition_refs, []);
    const comparison = proposal.revision_comparison;
    const expected = { ...proposal, validation: "invalid", condition_refs: [], candidate_text: tampered,
      revision_comparison: comparison.kind === "present" ? { ...comparison, spans: [{ base_from: 0, base_to: 0,
        candidate_from: 0, candidate_to: tampered.length, base_text: "", candidate_text: tampered }] } : comparison };
    const inspect = async () => (await getProposal({ baseUrl: story.baseUrl, projectId: story.projectId,
      proposalId: proposal.proposal_id, fetchImpl: story.fetchImpl })).proposal;
    assert.deepEqual(await inspect(), expected);
    const replayed = await Promise.all([0, 1].map(() => failed.command.send()));
    for (const replay of replayed) assert.deepEqual(replay, { response: failed.response, body: failed.body });

    const refused = await story.send("acceptProposal", { ...acceptance, request: { ...acceptance.request,
      accept_proposal_input: { ...acceptance.request.accept_proposal_input, ...story.binding() } } });
    assert.deepEqual(refused.response.effect, { kind: "refused", reason: "not_eligible" });
    await server.restart();
    assert.deepEqual(await inspect(), expected);
    assert.deepEqual(await chapter(), before);
    assert.equal((await inspect()).operation_resolution, "pending");
  } finally {
    await server.stop();
  }
});

test("replanProposal appends a pending Revision bound to the current Head", async () => {
  const server = await TestServer.start();
  try {
    await drainLeftoverWork();
    const story = await createStory(server, { title: "Replan Proposal Novel", assistance: true });
    const chapterId = story.chapterIds[0] ?? "";
    const chapter = async () => (await getChapter({ baseUrl: story.baseUrl, projectId: story.projectId,
      chapterId, fetchImpl: story.fetchImpl })).chapter;
    const inspect = async (proposalId: string) => (await getProposal({ baseUrl: story.baseUrl,
      projectId: story.projectId, proposalId, fetchImpl: story.fetchImpl })).proposal;
    let before = await chapter();
    // openProposal refuses a decision that is not an opened prose change.
    const opened = await openProposal(story);
    assert.deepEqual(opened.proposal.source_condition, { kind: "absent" });
    const { session, revised } = await reviseCandidate(story.baseUrl, story.fetchImpl, story.projectId, opened,
      freshNamespace());
    if (revised.proposal.validation_receipt.kind !== "present") {
      throw new Error("expected validation receipt");
    }
    const headId = story.key();
    await queryStoryOSPostgres(`INSERT INTO storyos.authoritative_revisions
      SELECT owner_user_id, project_id, manuscript_object_id, '${headId}'::uuid, payload_id
      FROM storyos.authoritative_revisions WHERE project_id = '${story.projectId}'::uuid
        AND revision_id = '${before.current_revision.revision_id}'::uuid;
      INSERT INTO storyos.manuscript_revision_members
      SELECT owner_user_id, project_id, manuscript_object_id, '${headId}'::uuid,
        manuscript_block_id, block_order FROM storyos.manuscript_revision_members
      WHERE project_id = '${story.projectId}'::uuid
        AND revision_id = '${before.current_revision.revision_id}'::uuid;
      UPDATE storyos.authoritative_heads SET current_revision_id = '${headId}'::uuid
      WHERE project_id = '${story.projectId}'::uuid
        AND manuscript_object_id = '${chapterId}'::uuid`);
    before = await chapter();
    const { conflictRef, receiptId } = await seedAcceptanceCondition(story.baseUrl, story.fetchImpl,
      story.projectId, revised.proposal, session.editor_session.editor_session_id,
      before.current_revision.revision_id, "proposal_conflict", freshNamespace());
    const afterConflict = await inspect(revised.proposal.proposal_id);
    assert.equal(afterConflict.validation, "conflicted");
    assert.equal(afterConflict.revision_id, revised.proposal.revision_id);
    assert.deepEqual(afterConflict.source_condition, {
      kind: "proposal_conflict",
      proposal_conflict_ref: conflictRef,
    });
    await queryStoryOSPostgres(`UPDATE storyos.proposal_validation_conditions
      SET condition_kind = 'proposal_recovery_conflict'
      WHERE conflict_id = '${conflictRef}'::uuid`);
    assert.deepEqual((await inspect(revised.proposal.proposal_id)).source_condition, {
      kind: "proposal_recovery_conflict",
      proposal_recovery_conflict_ref: conflictRef,
    });
    await queryStoryOSPostgres(`UPDATE storyos.proposal_validation_conditions
      SET condition_kind = 'proposal_conflict'
      WHERE conflict_id = '${conflictRef}'::uuid`);
    assert.deepEqual((await inspect(revised.proposal.proposal_id)).source_condition, {
      kind: "proposal_conflict",
      proposal_conflict_ref: conflictRef,
    });
    const replan = (conflictId = conflictRef) => ({ request: {
      command_schema: "storyos.command.replan-proposal.request.v1" as const,
      replan_proposal_input: {
        conflicted_proposal_revision_id: afterConflict.revision_id,
        expected_current_proposal_head: afterConflict.revision_id,
        expected_current_target_revisions: [before.current_revision.revision_id],
        replacement_operations: [afterConflict.operation_id],
        source_condition: { kind: "proposal_conflict" as const, proposal_conflict_ref: conflictId },
        editor_session_id: session.editor_session.editor_session_id,
        ...story.binding(),
      },
    }, path: { proposalId: afterConflict.proposal_id } });
    const unavailableProof = await story.send("replanProposal", replan(story.key()));
    assert.deepEqual(unavailableProof.response.effect, { kind: "refused", reason: "unavailable_proof" });
    assert.deepEqual((await inspect(afterConflict.proposal_id)).source_condition, {
      kind: "proposal_conflict",
      proposal_conflict_ref: conflictRef,
    });
    const replanInput = replan();
    const replanning = await story.challenge("replanProposal", replanInput);
    await queryStoryOSPostgres(`CREATE FUNCTION storyos.test_replan_failure() RETURNS trigger
      LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'replan fault'; END $$;
      CREATE TRIGGER test_replan_failure BEFORE INSERT ON storyos.proposal_replans
      FOR EACH ROW EXECUTE FUNCTION storyos.test_replan_failure();`);
    try {
      await assert.rejects(() => replanning.send(), (error) => requireStoryOSProtocolError(error).status === 503);
      const stillConflicted = await inspect(afterConflict.proposal_id);
      assert.equal(stillConflicted.revision_id, afterConflict.revision_id);
      assert.equal(stillConflicted.validation, "conflicted");
    } finally {
      await queryStoryOSPostgres(`DROP TRIGGER test_replan_failure ON storyos.proposal_replans;
        DROP FUNCTION storyos.test_replan_failure();`);
    }
    const replanned = (await replanning.send()).response;
    assert.equal(replanned.effect.kind, "resolved");
    assert.equal(replanned.receipt.result, "resolved");
    if (replanned.effect.kind !== "resolved") throw new Error("expected resolved");
    assert.equal(replanned.effect.undo_disposition, "forward");
    assert.equal(replanned.effect.resulting_validation, "pending");
    assert.equal(replanned.effect.preserved_generation, afterConflict.generation);
    assert.equal(replanned.effect.preserved_closure, afterConflict.closure);
    assert.deepEqual(replanned.effect.source_condition, {
      kind: "proposal_conflict",
      proposal_conflict_ref: conflictRef,
    });
    assert.match(replanned.effect.author_action_sequence, /^\d+$/);
    assert.equal(replanned.effect.state_event_refs.length, 1);
    const replanEventId = replanned.effect.state_event_refs[0] ?? "";
    assert.match(replanEventId, UUID_V7);
    assert.notEqual(replanned.effect.resulting_proposal_revision_id, afterConflict.revision_id);
    assert.equal(replanned.receipt.source_proposal_revision_id, afterConflict.revision_id);
    assert.equal(replanned.receipt.resulting_proposal_revision_id, replanned.effect.resulting_proposal_revision_id);
    assert.deepEqual(replanned.receipt.authoritative_commit_ids, []);
    assert.deepEqual((await replanning.send()).response, replanned);
    const inspected = await inspect(afterConflict.proposal_id);
    assert.equal(inspected.operation_id, afterConflict.operation_id);
    assert.equal(inspected.operation_resolution, afterConflict.operation_resolution);
    assert.equal(inspected.validation, "pending");
    assert.equal(inspected.revision_id, replanned.effect.resulting_proposal_revision_id);
    assert.equal(inspected.base_authoritative_revision_id, before.current_revision.revision_id);
    assert.deepEqual(inspected.source_condition, { kind: "absent" });
    assert.deepEqual(inspected.validation_receipt, { kind: "absent" });
    assert.deepEqual(await chapter(), before);
    await assert.rejects(() => story.as("session-b").send("replanProposal", replanInput), (error) => {
      const protocol = requireStoryOSProtocolError(error);
      return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
    });
    // The request names the conflicted Proposal Revision, which is not the current Revision now.
    const refused = await story.send("replanProposal", replan());
    assert.deepEqual(refused.response.effect, { kind: "refused", reason: "stale_proposal_revision" });
    const archive = await story.send("exportProjectArchive", { request: {
      command_schema: "storyos.command.export-project-archive.request.v1",
      export_project_archive_input: { ...story.binding(), archive_profile: "storyos.project-export.v1",
        archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" },
    } });
    if (archive.response.effect.kind !== "admitted") throw new Error("expected admitted archive");
    const exportId = archive.response.effect.export_id;
    await settleOnce();
    assert.equal((await getExportOperation({ baseUrl: story.baseUrl, projectId: story.projectId, exportId,
      fetchImpl: story.fetchImpl })).status, "ready");
    const download = await story.fetchImpl(`${story.baseUrl}/api/v1/projects/${story.projectId}/exports/${exportId}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
    assert.equal(download.status, 200);
    const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
    const conditions = JSON.parse(new TextDecoder().decode(files.get("canonical/proposal_validation_conditions.json")));
    assert.deepEqual(conditions, [{
      owner_user_id: USER_A,
      project_id: story.projectId,
      proposal_id: afterConflict.proposal_id,
      proposal_revision_id: afterConflict.revision_id,
      acceptance_receipt_id: receiptId,
      validation: "conflicted",
      conflict_id: conflictRef,
      condition_kind: "proposal_conflict",
    }]);
    const replans = JSON.parse(new TextDecoder().decode(files.get("canonical/proposal_replans.json")));
    assert.deepEqual(replans, [{
      owner_user_id: USER_A,
      project_id: story.projectId,
      replan_event_id: replanEventId,
      proposal_id: afterConflict.proposal_id,
      source_proposal_revision_id: afterConflict.revision_id,
      resulting_proposal_revision_id: replanned.effect.resulting_proposal_revision_id,
      source_condition_kind: "proposal_conflict",
      source_condition_ref: conflictRef,
      replan_receipt_id: replanned.receipt.receipt_id,
      author_action_sequence: Number(replanned.effect.author_action_sequence),
      preserved_generation: replanned.effect.preserved_generation,
      preserved_closure: replanned.effect.preserved_closure,
    }]);
    await server.restart();
    const reloaded = await inspect(afterConflict.proposal_id);
    assert.equal(reloaded.revision_id, replanned.effect.resulting_proposal_revision_id);
    assert.equal(reloaded.validation, "pending");
    assert.equal(reloaded.base_authoritative_revision_id, before.current_revision.revision_id);
    assert.deepEqual(reloaded.source_condition, { kind: "absent" });
    assert.deepEqual(await chapter(), before);
  } finally {
    await server.stop();
  }
});

test("reopenRejectedOperations appends a pending Revision without rewriting Rejection", async () => {
  const server = await TestServer.start();
  try {
    const proposal = await openedProposal(server, "Reopen Proposal Novel");
    const { story, opened, before, editorSessionId } = proposal;
    const rejected = (await story.send("rejectProposalOperations", rejection(proposal))).response;
    assert.equal(rejected.effect.kind, "resolved");
    if (rejected.effect.kind !== "resolved") throw new Error("expected rejected");
    const resolutionEventId = rejected.effect.resolution_event_refs[0] ?? "";
    const afterReject = await proposal.inspect();
    assert.equal(afterReject.operation_resolution, "rejected");
    assert.equal(afterReject.revision_id, opened.revision_id);
    const reopening = (options: { rejectionEventId?: string } = {}): CommandInput<"reopenRejectedOperations"> => ({
      request: {
        command_schema: "storyos.command.reopen-rejected-operations.request.v1",
        reopen_rejected_operations_input: {
          proposal_revision_id: opened.revision_id,
          selected_rejected_operation_ids: [opened.operation_id],
          rejection_event_refs: [options.rejectionEventId ?? resolutionEventId],
          expected_target_revisions: [before.current_revision.revision_id],
          editor_session_id: editorSessionId,
          ...story.binding(),
        },
      }, path: { proposalId: opened.proposal_id } });
    const unavailableProof = await story.send("reopenRejectedOperations",
      reopening({ rejectionEventId: story.key() }));
    assert.deepEqual(unavailableProof.response.effect, { kind: "refused", reason: "unavailable_proof" });
    assert.equal((await proposal.inspect()).operation_resolution, "rejected");
    const reopenInput = reopening();
    const reopen = await story.challenge("reopenRejectedOperations", reopenInput);
    await queryStoryOSPostgres(`CREATE FUNCTION storyos.test_reopen_failure() RETURNS trigger
      LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'reopen fault'; END $$;
      CREATE TRIGGER test_reopen_failure BEFORE INSERT ON storyos.proposal_operation_reopenings
      FOR EACH ROW EXECUTE FUNCTION storyos.test_reopen_failure();`);
    try {
      assert.equal((await problemOf(reopen.send())).status, 503);
      assert.equal((await proposal.inspect()).operation_resolution, "rejected");
    } finally {
      await queryStoryOSPostgres(`DROP TRIGGER test_reopen_failure ON storyos.proposal_operation_reopenings;
        DROP FUNCTION storyos.test_reopen_failure();`);
    }
    const reopened = (await reopen.send()).response;
    assert.equal(reopened.effect.kind, "resolved");
    assert.equal(reopened.receipt.result, "resolved");
    if (reopened.effect.kind !== "resolved") throw new Error("expected resolved");
    assert.equal(reopened.effect.undo_disposition, "forward");
    assert.equal(reopened.effect.prior_resolution, "rejected");
    assert.equal(reopened.effect.resulting_resolution, "pending");
    assert.equal(reopened.effect.resulting_validation, "pending");
    assert.deepEqual(reopened.effect.operation_ids, [opened.operation_id]);
    assert.deepEqual(reopened.effect.rejection_event_refs, [resolutionEventId]);
    assert.match(reopened.effect.author_action_sequence, /^\d+$/);
    assert.equal(reopened.effect.state_event_refs.length, 1);
    const reopenEventId = reopened.effect.state_event_refs[0] ?? "";
    assert.match(reopenEventId, UUID_V7);
    assert.notEqual(reopened.effect.resulting_proposal_revision_id, opened.revision_id);
    assert.equal(reopened.receipt.source_proposal_revision_id, opened.revision_id);
    assert.equal(reopened.receipt.resulting_proposal_revision_id, reopened.effect.resulting_proposal_revision_id);
    assert.deepEqual(reopened.receipt.authoritative_commit_ids, []);
    assert.deepEqual((await reopen.send()).response, reopened);
    const inspected = await proposal.inspect();
    assert.equal(inspected.operation_resolution, "pending");
    assert.equal(inspected.reservation_state, "unresolved");
    assert.equal(inspected.validation, "pending");
    assert.equal(inspected.revision_id, reopened.effect.resulting_proposal_revision_id);
    assert.deepEqual(inspected.validation_receipt, { kind: "absent" });
    assert.deepEqual(await proposal.chapter(), before);
    const foreign = await problemOf(story.as("session-b").send("reopenRejectedOperations", reopenInput));
    assert.ok(foreign.status === 404 && !foreign.body.includes(USER_A));
    // The request names the source Proposal Revision, which is not the current Revision now.
    const refused = await story.send("reopenRejectedOperations", reopening());
    assert.deepEqual(refused.response.effect, { kind: "refused", reason: "stale_proposal_revision" });

    const scope = { owner_user_id: USER_A, project_id: story.projectId, proposal_id: opened.proposal_id };
    const archived = await archivedTables(story);
    assert.deepEqual(archived("proposal_operation_resolutions"), [{
      ...scope,
      resolution_event_id: resolutionEventId,
      proposal_revision_id: opened.revision_id,
      operation_id: opened.operation_id,
      prior_resolution: "pending",
      resulting_resolution: "rejected",
      rejection_receipt_id: rejected.receipt.receipt_id,
      author_action_sequence: Number(rejected.effect.author_action_sequence),
    }]);
    assert.deepEqual(archived("proposal_operation_reopenings"), [{
      ...scope,
      reopen_event_id: reopenEventId,
      source_proposal_revision_id: opened.revision_id,
      resulting_proposal_revision_id: reopened.effect.resulting_proposal_revision_id,
      operation_id: opened.operation_id,
      rejection_event_id: resolutionEventId,
      prior_resolution: "rejected",
      resulting_resolution: "pending",
      reopen_receipt_id: reopened.receipt.receipt_id,
      author_action_sequence: Number(reopened.effect.author_action_sequence),
      preserved_generation: reopened.effect.preserved_generation,
      preserved_closure: reopened.effect.preserved_closure,
    }]);
    await server.restart();
    const reloaded = await proposal.inspect();
    assert.equal(reloaded.operation_resolution, "pending");
    assert.equal(reloaded.validation, "pending");
    assert.equal(reloaded.revision_id, reopened.effect.resulting_proposal_revision_id);
    assert.deepEqual(await proposal.chapter(), before);
  } finally {
    await server.stop();
  }
});
