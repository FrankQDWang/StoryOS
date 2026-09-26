import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect } from "playwright/test";
import { cancelAgentRun, createAgentRun, digestCancelAgentRun, digestCreateAgentRun, getAgentRun }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ApplyAuthorEditResponse, CancelAgentRunRequest, CreateAgentRunRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, challenged, id } from "./acceptance.ts";
import { queryStoryOSPostgres as query, runStoryOSWorker, sessionFetch } from "./node-integration.ts";

export async function verifyProductionRetryReservationRace({ origin, projectId, chapterId, blockId, retry }: {
  origin: string; projectId: string; chapterId: string; blockId: string;
  retry: () => Promise<ApplyAuthorEditResponse>;
}): Promise<ApplyAuthorEditResponse> {
  const options = { baseUrl: origin, projectId, fetchImpl: sessionFetch(origin, "session-a") };
  const request: CreateAgentRunRequest = { command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: { ...BINDING, correlation_id: id("e0c9911"), conversation: { kind: "new" },
      author_message: { text: "Revise this passage: keep the voice." },
      working_target: { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" }, cause: { kind: "author_request" } } };
  const admitted = await challenged(origin, options.fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/agent-runs", request.command_schema, await digestCreateAgentRun(request),
    id("e0c9912"), (antiForgery) => createAgentRun({ ...options, request, antiForgery, idempotencyKey: id("e0c9912") }));
  assert.ok(admitted.effect.kind === "admitted");
  const runId = admitted.effect.run_id;
  const tables = ["proposals", "proposal_heads", "proposal_revisions", "proposal_operations", "proposal_anchors",
    "validation_receipts", "model_attempts", "domain_receipts", "author_action_entries"];
  const readPhase = async (): Promise<Record<string, Record<string, unknown>[]>> => JSON.parse(await query(
    `SELECT jsonb_build_object(${tables.map((table) => `'${table}', (SELECT coalesce(
      jsonb_agg(to_jsonb(row) ORDER BY to_jsonb(row)::text), '[]'::jsonb) FROM storyos.${table} AS row
      WHERE row.owner_user_id='018f0000-0000-7001-8000-000000000001'::uuid
        AND row.project_id='${projectId}'::uuid)`).join(",")})::text`));
  const admittedState = await readPhase();
  const lockKey = createHash("sha256").update(runId).digest().readUInt32BE(0) & 0x7fffffff;
  const guardName = `storyos_retry_guard_${runId}`;
  const lockPredicate = `locktype='advisory' AND classid=826 AND objid=${lockKey} AND objsubid=2`;
  const barrier = `hold_retry_${lockKey}`;
  const container = process.env.STORYOS_TEST_POSTGRES_CONTAINER;
  assert.ok(container);
  const guard = spawn("docker", ["exec", "-i", container, "psql", "-X", "-v", "ON_ERROR_STOP=1",
    "-U", "postgres", "-Atq"], { stdio: ["pipe", "pipe", "pipe"] });
  let guardError: unknown;
  guard.on("error", (error) => { guardError = error; });
  guard.stdout.resume(); guard.stderr.resume();
  guard.stdin.write(`SET application_name='${guardName}'; SELECT pg_advisory_lock(826,${lockKey});\n`);
  let guardPid: number | undefined;
  const release = async () => {
    if (guardPid === undefined) {
      const pid = await query(`SELECT activity.pid FROM pg_stat_activity AS activity JOIN pg_locks AS lock USING(pid)
        WHERE activity.application_name='${guardName}' AND ${lockPredicate} AND granted`);
      if (/^\d+$/.test(pid)) guardPid = Number(pid);
    }
    if (guardPid !== undefined) await query(`SELECT pg_terminate_backend(${guardPid})
      WHERE EXISTS (SELECT 1 FROM pg_stat_activity WHERE pid=${guardPid} AND application_name='${guardName}')
        AND EXISTS (SELECT 1 FROM pg_locks WHERE pid=${guardPid} AND ${lockPredicate} AND granted)`);
  };
  let work: Promise<unknown> | undefined;
  let installed = false;
  try {
    await expect.poll(async () => {
      if (guardError) throw guardError;
      const pid = await query(`SELECT activity.pid FROM pg_stat_activity AS activity JOIN pg_locks AS lock USING(pid)
        WHERE activity.application_name='${guardName}' AND ${lockPredicate} AND granted`);
      guardPid = /^\d+$/.test(pid) ? Number(pid) : undefined;
      return guardPid === undefined ? "0" : "1";
    }, { timeout: 5000 }).toBe("1");
    await query(`CREATE FUNCTION storyos.${barrier}() RETURNS trigger LANGUAGE plpgsql AS $fault$
      BEGIN IF NEW.owner_user_id='018f0000-0000-7001-8000-000000000001'::uuid
        AND NEW.project_id='${projectId}'::uuid AND NEW.manuscript_block_id='${blockId}'::uuid
        AND EXISTS (SELECT 1 FROM storyos.proposals AS proposal WHERE proposal.owner_user_id=NEW.owner_user_id
          AND proposal.project_id=NEW.project_id AND proposal.proposal_id=NEW.proposal_id
          AND proposal.source_run_id='${runId}'::uuid) THEN PERFORM pg_advisory_xact_lock(826,${lockKey});
      END IF; RETURN NEW; END $fault$;
      CREATE TRIGGER ${barrier} BEFORE INSERT ON storyos.proposal_operations
        FOR EACH ROW EXECUTE FUNCTION storyos.${barrier}();`);
    installed = true;
    const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
    work = runStoryOSWorker({ repositoryRoot, workerBinary: join(repositoryRoot, "target/release-package/storyos-worker"),
      args: ["--once"] }).then(() => undefined, (error: unknown) => error);
    await expect.poll(() => query(`SELECT count(*) FROM pg_locks
      WHERE ${lockPredicate} AND NOT granted AND pg_blocking_pids(pid)=ARRAY[${guardPid}]`),
      { timeout: 5000 }).toBe("1");
    const before = await readPhase();
    const dispatched = before.model_attempts!.filter((row) =>
      !admittedState.model_attempts!.some((prior) => prior.model_attempt_id === row.model_attempt_id));
    assert.equal(dispatched.length, 1);
    assert.deepEqual([dispatched[0]!.owner_user_id, dispatched[0]!.project_id, dispatched[0]!.run_id],
      ["018f0000-0000-7001-8000-000000000001", projectId, runId]);
    assert.deepEqual({ ...before, model_attempts: admittedState.model_attempts }, admittedState);
    const settled = await retry();
    assert.equal(settled.effect.kind, "authoritative_applied");
    await release();
    const failure = await work;
    assert.ok(failure instanceof Error);
    assert.match(String(Reflect.get(failure, "stderr")), /SqlState\(E40001\)/);
    const after = await readPhase();
    const receiptRows = after.domain_receipts!.filter((row) => row.receipt_id === settled.receipt.receipt_id);
    const actionRows = after.author_action_entries!.filter((row) => row.receipt_id === settled.receipt.receipt_id);
    assert.equal(receiptRows.length, 1); assert.equal(actionRows.length, 1);
    const { project_scope, command_digest, author_action_sequence, result, created_at, ...receiptFields } = settled.receipt;
    assert.deepEqual({ ...receiptRows[0], created_at: new Date(String(receiptRows[0]!.created_at)).toISOString() },
      { ...project_scope, ...receiptFields,
        command_digest: `${command_digest.algorithm}:${command_digest.profile}:${command_digest.value_hex_lowercase}`,
        command_id: settled.command_id, result_kind: result, result_payload: {},
        created_at: new Date(created_at).toISOString(), source_draft_disposition: settled.source_draft_disposition });
    assert.deepEqual(actionRows, [{ ...project_scope, author_action_sequence: Number(author_action_sequence),
      disposition: "forward", authoritative_commit_id: settled.effect.authoritative_commit_id,
      receipt_id: settled.receipt.receipt_id, receipt_result_kind: "authoritative_applied", compensated_source_sequence: null }]);
    for (const table of ["domain_receipts", "author_action_entries"])
      after[table] = after[table]!.filter((row) => row.receipt_id !== settled.receipt.receipt_id);
    assert.deepEqual(after, before);
    const claimed = await getAgentRun({ ...options, runId });
    assert.equal(claimed.status, "claimed");
    const cancel: CancelAgentRunRequest = { command_schema: "storyos.command.cancel-agent-run.request.v1",
      cancel_agent_run_input: { ...BINDING, correlation_id: id("e0c9921") } };
    const cancelled = await challenged(origin, options.fetchImpl, projectId, "POST",
      "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel", cancel.command_schema,
      await digestCancelAgentRun(cancel), id("e0c9922"), (antiForgery) => cancelAgentRun({ ...options, runId,
        request: cancel, antiForgery, idempotencyKey: id("e0c9922") }));
    assert.equal(cancelled.effect.kind, "applied");
    assert.ok(cancelled.receipt.author_action_sequence == null);
    return settled;
  } finally {
    const failures: unknown[] = [];
    for (const cleanup of [release, async () => { guard.stdin.destroy(); guard.kill("SIGKILL"); },
      async () => { await work; }, async () => { if (installed) await query(
        `DROP TRIGGER ${barrier} ON storyos.proposal_operations; DROP FUNCTION storyos.${barrier}();`); }]) {
      try { await cleanup(); } catch (error) { failures.push(error); }
    }
    if (failures.length > 0) throw new AggregateError(failures, "Retry reservation race cleanup failed");
  }
}
