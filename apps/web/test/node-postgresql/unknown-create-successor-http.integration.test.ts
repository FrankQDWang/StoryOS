// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/retrieve-original-result-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { execFile, type ChildProcess } from "node:child_process";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { test } from "vitest";

import {
  cancelAgentRun,
  createAgentRun,
  digestCancelAgentRun,
  digestCreateAgentRun,
  getAgentRun,
  type CancelAgentRunRequest,
  type CreateAgentRunRequest,
  type GetAgentRunResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";
import {
  BINDING,
  USER_A,
  challenged,
  drainLeftoverWork,
  id,
  prepare,
  settleOnce,
  startRealServer,
} from "../support/acceptance.ts";

const execFileAsync = promisify(execFile);
const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const workerBin = join(repositoryRoot, "target/release-package/storyos-worker");
const ADVISORY = "This passage is inspectable Host-fake advice. It is not Authoritative State.";

async function admit(
  baseUrl: string,
  prepared: Awaited<ReturnType<typeof prepare>>,
  key: string,
  text: string,
) {
  const request: CreateAgentRunRequest = {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation: { kind: "new" },
      author_message: { text },
      working_target: { kind: "current_chapter", chapter_id: prepared.chapterId },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...BINDING,
      correlation_id: id(key.slice(-4)),
    },
  };
  const created = await challenged(
    baseUrl,
    prepared.fetchImpl,
    prepared.projectId,
    "POST",
    "/api/v1/projects/{project_id}/agent-runs",
    request.command_schema,
    await digestCreateAgentRun(request),
    key,
    (antiForgery) => createAgentRun({
      baseUrl,
      projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: key,
      antiForgery,
      request,
    }),
  );
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  return created;
}

function workerEnv(extraEnv: Readonly<Record<string, string>>) {
  const env: NodeJS.ProcessEnv = { ...process.env, ...extraEnv };
  if (process.env.STORYOS_TEST_DATABASE_URL !== undefined) {
    env.STORYOS_DATABASE_URL = process.env.STORYOS_TEST_DATABASE_URL;
  }
  return env;
}

function settleHeld(extraEnv: Readonly<Record<string, string>>): Promise<void> {
  const settled = execFileAsync(workerBin, ["--once"], {
    cwd: repositoryRoot,
    env: workerEnv(extraEnv),
    timeout: 60_000,
    killSignal: "SIGKILL",
  }).then(() => undefined);
  void settled.catch(() => undefined);
  return settled;
}

function heldWorker(extraEnv: Readonly<Record<string, string>>) {
  const child: ChildProcess = execFile(workerBin, ["--once"], {
    cwd: repositoryRoot,
    env: workerEnv(extraEnv),
    timeout: 60_000,
    killSignal: "SIGKILL",
  });
  const exited = new Promise<void>((resolve) => {
    child.once("exit", () => resolve());
    child.once("error", () => resolve());
  });
  return { child, exited };
}

async function inspect(
  baseUrl: string,
  prepared: Awaited<ReturnType<typeof prepare>>,
  runId: string,
) {
  return getAgentRun({
    baseUrl,
    projectId: prepared.projectId,
    runId,
    fetchImpl: prepared.fetchImpl,
  });
}

async function poll(
  baseUrl: string,
  prepared: Awaited<ReturnType<typeof prepare>>,
  runId: string,
  ready: (response: GetAgentRunResponse) => boolean,
) {
  let held: GetAgentRunResponse | undefined;
  for (let attempt = 0; attempt < 200; attempt += 1) {
    held = await inspect(baseUrl, prepared, runId);
    if (ready(held)) return held;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`worker did not reach the hold: ${held?.status ?? "missing"}`);
}

function successor(response: GetAgentRunResponse) {
  if (response.unknown_create_successor.kind !== "present") {
    throw new Error("expected unknown-create successor");
  }
  return response.unknown_create_successor;
}

function presentAttempt(response: GetAgentRunResponse) {
  if (response.model_attempt.kind !== "present") throw new Error("expected attempt");
  return response.model_attempt;
}

async function roles(projectId: string, runId: string) {
  return queryPostgres(`
    SELECT count(DISTINCT model_invocation_id)::text
        || ' ' || count(*) FILTER (WHERE attempt_role = 'decision')::text
        || ' ' || count(*) FILTER (WHERE attempt_role = 'retrieval')::text
        || ' ' || count(*) FILTER (WHERE attempt_role = 'successor')::text
      FROM storyos.model_attempts
     WHERE project_id = '${projectId}'::uuid
       AND run_id = '${runId}'::uuid;
  `);
}

async function predecessor(projectId: string, runId: string) {
  return queryPostgres(`
    SELECT coalesce(decision_id::text, 'none')
        || ' ' || dispatch_state
        || ' ' || (payload->'usage'->>'kind')
        || ' ' || (payload->'reservation'->>'released')
      FROM storyos.model_attempts
     WHERE project_id = '${projectId}'::uuid
       AND run_id = '${runId}'::uuid
       AND attempt_role = 'decision';
  `);
}

async function requeue(projectId: string, runId: string) {
  await queryPostgres(`
    UPDATE storyos.agent_runs
       SET status = 'queued', lease_expires_at = NULL, wakeup_pending = true
     WHERE project_id = '${projectId}'::uuid
       AND run_id = '${runId}'::uuid;
  `);
}

function assertClosedSuccessor(found: ReturnType<typeof successor>) {
  assert.equal(found.successor_settles_predecessor, false);
  assert.equal(found.supplies_tool_call, false);
  assert.equal(found.advances_predecessor_continuation, false);
  assert.equal(found.reuses_changed_context, false);
}

test("one successor shares the invocation and a restart does not add another", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("a401"), "Successor Once Novel", "a4");
    const created = await admit(started.baseUrl, prepared, id("a411"), "SCRIPT:successor-once");
    await settleOnce();
    const first = await inspect(started.baseUrl, prepared, created.effect.run_id);
    const found = successor(first);
    const attempt = presentAttempt(first);
    assert.equal(first.status, "completed");
    assert.equal(first.usage.kind, "unknown");
    assert.equal(first.decision.kind, "advisory");
    if (first.decision.kind !== "advisory") throw new Error("expected advisory");
    assert.equal(first.decision.text, ADVISORY);
    assert.equal(first.items.every((item) => item.call_id === null), true);
    assert.equal(attempt.model_attempt_id, found.predecessor_model_attempt_id);
    assert.equal(attempt.dispatch_state, "uncertain");
    assert.equal(attempt.model_invocation_id, found.model_invocation_id);
    assert.notEqual(found.successor_model_attempt_id, found.predecessor_model_attempt_id);
    assert.equal(found.disposition, "dispatched");
    assert.equal(found.allowance_consumed, true);
    assert.equal(found.predecessor_fenced, true);
    assert.equal(found.predecessor_usage_kind, "unknown");
    assert.equal(found.predecessor_reservation_released, false);
    assert.equal(found.lookup_unavailable_reason ?? null, null);
    assertClosedSuccessor(found);
    if (first.original_result_retrieval.kind !== "present") throw new Error("expected retrieval");
    assert.equal(first.original_result_retrieval.disposition, "kept_unknown");
    assert.equal(first.original_result_retrieval.keep_reason, "unknown_result");
    assert.equal(first.original_result_retrieval.supplies_decision, false);
    assert.equal(await roles(prepared.projectId, created.effect.run_id), "1 1 1 1");
    assert.equal(await predecessor(prepared.projectId, created.effect.run_id), "none uncertain unknown false");
    assert.equal(await queryPostgres(`
      SELECT (decision.model_invocation_id = successor.model_invocation_id
              AND decision.model_attempt_id <> successor.model_attempt_id
              AND decision.outbound_disclosure_event_id <> successor.outbound_disclosure_event_id
              AND successor.decision_id IS NOT NULL)::text
          || ' ' || (successor.payload->>'operation')
          || ' ' || (successor.payload->>'settles_predecessor')
          || ' ' || (successor.payload->'wire'->>'author_message')
        FROM storyos.model_attempts AS decision
        JOIN storyos.model_attempts AS successor
          ON successor.project_id = decision.project_id
         AND successor.run_id = decision.run_id
         AND successor.attempt_role = 'successor'
       WHERE decision.project_id = '${prepared.projectId}'::uuid
         AND decision.run_id = '${created.effect.run_id}'::uuid
         AND decision.attempt_role = 'decision';
    `), "true unknown_create_successor false SCRIPT:successor-once");
    assert.equal(await queryPostgres(`
      SELECT (SELECT count(*)
                FROM storyos.context_assembly_manifests
               WHERE project_id = '${prepared.projectId}'::uuid
                 AND run_id = '${created.effect.run_id}'::uuid
                 AND manifest_role = 'successor')::text
          || ' ' ||
             (SELECT count(*)
                FROM storyos.operation_requirements
               WHERE project_id = '${prepared.projectId}'::uuid
                 AND run_id = '${created.effect.run_id}'::uuid
                 AND requirement_role = 'successor')::text;
    `), "1 1");
    await requeue(prepared.projectId, created.effect.run_id);
    await settleOnce();
    const repeated = await inspect(started.baseUrl, prepared, created.effect.run_id);
    const again = successor(repeated);
    assert.equal(repeated.status, "completed");
    assert.equal(again.recovery_id, found.recovery_id);
    assert.equal(again.successor_model_attempt_id, found.successor_model_attempt_id);
    if (repeated.decision.kind !== "advisory") throw new Error("expected advisory");
    assert.equal(repeated.decision.decision_id, first.decision.decision_id);
    assert.equal(await roles(prepared.projectId, created.effect.run_id), "1 1 1 1");
    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    await assert.rejects(
      () => getAgentRun({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        runId: created.effect.run_id,
        fetchImpl: foreignFetch,
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
  } finally {
    await stopRealServer(started.server);
  }
});

test("missing and unsupported lookup still permit one successor", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("a501"), "Successor Lookup Novel", "a5");
    const cases = [
      ["SCRIPT:successor-missing", "missing_reference", id("a511")],
      ["SCRIPT:successor-unsupported", "unsupported_retrieval", id("a512")],
    ] as const;
    for (const [script, reason, key] of cases) {
      const created = await admit(started.baseUrl, prepared, key, script);
      await settleOnce();
      const inspected = await inspect(started.baseUrl, prepared, created.effect.run_id);
      const found = successor(inspected);
      assert.equal(inspected.status, "completed", script);
      assert.equal(found.lookup_unavailable_reason, reason, script);
      assert.equal(found.disposition, "dispatched", script);
      assert.equal(found.allowance_consumed, true, script);
      assert.equal(await roles(prepared.projectId, created.effect.run_id), "1 1 0 1", script);
      if (inspected.original_result_retrieval.kind !== "present") throw new Error(script);
      assert.equal(inspected.original_result_retrieval.keep_reason, reason, script);
      assert.equal(inspected.original_result_retrieval.supplies_decision, false, script);
      assert.equal(await predecessor(prepared.projectId, created.effect.run_id), "none uncertain unknown false", script);
    }
  } finally {
    await stopRealServer(started.server);
  }
});

test("failed gates pause and a restart keeps that pause", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("a601"), "Successor Pause Novel", "a6");
    const cases = [
      ["SCRIPT:successor-budget", "budget_insufficient", id("a6c0")],
      ["SCRIPT:successor-effect", "unresolved_effect", id("a6c1")],
      ["SCRIPT:successor-absent-effect", "unsupported_absent_effect", id("a6c2")],
      ["SCRIPT:successor-context", "changed_effective_model_context", id("a6c3")],
      ["SCRIPT:successor-request", "request_changed", id("a6c4")],
      ["SCRIPT:successor-route", "route_changed", id("a6c5")],
      ["SCRIPT:successor-authority", "authority_unavailable", id("a6c6")],
    ] as const;
    for (const [script, reason, key] of cases) {
      const created = await admit(started.baseUrl, prepared, key, script);
      await settleOnce();
      const inspected = await inspect(started.baseUrl, prepared, created.effect.run_id);
      const found = successor(inspected);
      assert.equal(inspected.status, "paused", script);
      assert.equal(inspected.decision.kind, "absent", script);
      assert.equal(found.disposition, "paused", script);
      assert.equal(found.pause_reason, reason, script);
      assert.equal(found.allowance_consumed, false, script);
      assert.equal(found.predecessor_fenced, false, script);
      assert.equal(found.successor_model_attempt_id ?? null, null, script);
      assert.equal(found.reuses_changed_context, false, script);
      assert.equal(await roles(prepared.projectId, created.effect.run_id), "1 1 1 0", script);
      if (script === "SCRIPT:successor-context") {
        await requeue(prepared.projectId, created.effect.run_id);
        await settleOnce();
        const repeated = await inspect(started.baseUrl, prepared, created.effect.run_id);
        const again = successor(repeated);
        assert.equal(repeated.status, "paused", script);
        assert.equal(again.recovery_id, found.recovery_id, script);
        assert.equal(again.pause_reason, reason, script);
        assert.equal(again.allowance_consumed, false, script);
        assert.equal(await roles(prepared.projectId, created.effect.run_id), "1 1 1 0", script);
      }
    }
  } finally {
    await stopRealServer(started.server);
  }
});

test("cancellation after the fence prohibits the successor", async () => {
  const hold = join(tmpdir(), "storyos-s3-24-fence.hold");
  const started = await startRealServer();
  let worker: Promise<void> | undefined;
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("a701"), "Successor Fence Novel", "a7");
    const created = await admit(started.baseUrl, prepared, id("a711"), "SCRIPT:successor-once");
    writeFileSync(hold, "hold");
    worker = settleHeld({ STORYOS_TEST_FAKE_SUCCESSOR_FENCE_HOLD_PATH: hold });
    const held = await poll(started.baseUrl, prepared, created.effect.run_id, (response) => {
      const found = response.unknown_create_successor;
      return found.kind === "present" && found.disposition === "fenced"
        && found.successor_model_attempt_id == null;
    });
    const fenced = successor(held);
    assert.equal(held.status, "claimed");
    assert.equal(fenced.allowance_consumed, true);
    assert.equal(fenced.predecessor_fenced, true);
    assert.equal(await roles(prepared.projectId, created.effect.run_id), "1 1 1 0");
    const request: CancelAgentRunRequest = {
      command_schema: "storyos.command.cancel-agent-run.request.v1",
      cancel_agent_run_input: { ...BINDING, correlation_id: id("a721") },
    };
    const cancelled = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
      request.command_schema,
      await digestCancelAgentRun(request),
      id("a731"),
      (antiForgery) => cancelAgentRun({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        runId: created.effect.run_id,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("a731"),
        antiForgery,
        request,
      }),
    );
    assert.equal(cancelled.effect.kind, "applied");
    unlinkSync(hold);
    await worker;
    await settleOnce();
    const after = await inspect(started.baseUrl, prepared, created.effect.run_id);
    const found = successor(after);
    assert.equal(after.status, "cancelled");
    assert.equal(after.decision.kind, "absent");
    assert.equal(after.items.length, 0);
    assert.equal(found.disposition, "prohibited");
    assert.equal(found.recovery_id, fenced.recovery_id);
    assert.equal(found.allowance_consumed, true);
    assert.equal(found.predecessor_fenced, true);
    assert.equal(found.successor_model_attempt_id ?? null, null);
    assert.equal(found.predecessor_usage_kind, "unknown");
    assert.equal(found.predecessor_reservation_released, false);
    assertClosedSuccessor(found);
    if (after.original_result_retrieval.kind !== "present") throw new Error("expected retrieval");
    assert.equal(after.original_result_retrieval.disposition, "kept_unknown");
    assert.equal(await predecessor(prepared.projectId, created.effect.run_id), "none uncertain unknown false");
    assert.equal(await roles(prepared.projectId, created.effect.run_id), "1 1 1 0");
  } finally {
    if (existsSync(hold)) unlinkSync(hold);
    if (worker !== undefined) await Promise.allSettled([worker]);
    await stopRealServer(started.server);
  }
});

test("a crash after the fence resumes the same successor once", async () => {
  const hold = join(tmpdir(), "storyos-s3-24-crash.hold");
  const started = await startRealServer();
  let worker: ReturnType<typeof heldWorker> | undefined;
  let projectId = "";
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("a801"), "Successor Crash Novel", "a8");
    projectId = prepared.projectId;
    const created = await admit(started.baseUrl, prepared, id("a811"), "SCRIPT:successor-once");
    writeFileSync(hold, "hold");
    worker = heldWorker({ STORYOS_TEST_FAKE_SUCCESSOR_FENCE_HOLD_PATH: hold });
    const held = await poll(started.baseUrl, prepared, created.effect.run_id, (response) => {
      const found = response.unknown_create_successor;
      return found.kind === "present" && found.disposition === "fenced"
        && found.successor_model_attempt_id == null;
    });
    const fenced = successor(held);
    worker.child.kill("SIGKILL");
    await worker.exited;
    worker = undefined;
    await queryPostgres(`
      UPDATE storyos.agent_runs
         SET lease_expires_at = clock_timestamp() - interval '1 second'
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${created.effect.run_id}'::uuid
         AND status = 'claimed';
    `);
    unlinkSync(hold);
    await settleOnce();
    const after = await inspect(started.baseUrl, prepared, created.effect.run_id);
    const found = successor(after);
    assert.equal(after.status, "completed");
    assert.equal(found.recovery_id, fenced.recovery_id);
    assert.equal(found.disposition, "dispatched");
    assert.equal(found.allowance_consumed, true);
    assert.equal(found.predecessor_fenced, true);
    assert.match(found.successor_model_attempt_id ?? "", /^[0-9a-f-]{36}$/);
    assert.equal(found.predecessor_reservation_released, false);
    assert.equal(await roles(prepared.projectId, created.effect.run_id), "1 1 1 1");
    assert.equal(await predecessor(prepared.projectId, created.effect.run_id), "none uncertain unknown false");
    await requeue(prepared.projectId, created.effect.run_id);
    await settleOnce();
    assert.equal(await roles(prepared.projectId, created.effect.run_id), "1 1 1 1");
  } finally {
    if (existsSync(hold)) unlinkSync(hold);
    if (worker !== undefined) {
      worker.child.kill("SIGKILL");
      await worker.exited;
    }
    if (projectId !== "") {
      await queryPostgres(`
        UPDATE storyos.agent_runs
           SET lease_expires_at = clock_timestamp() - interval '1 second'
         WHERE project_id = '${projectId}'::uuid
           AND status = 'claimed';
      `);
    }
    await stopRealServer(started.server);
  }
});

test("a late predecessor updates evidence and usage only", async () => {
  const hold = join(tmpdir(), "storyos-s3-24-late.hold");
  const started = await startRealServer();
  let worker: Promise<void> | undefined;
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("a901"), "Successor Late Novel", "a9");
    const created = await admit(started.baseUrl, prepared, id("a911"), "SCRIPT:successor-late");
    writeFileSync(hold, "hold");
    worker = settleHeld({ STORYOS_TEST_FAKE_SUCCESSOR_LATE_HOLD_PATH: hold });
    const held = await poll(started.baseUrl, prepared, created.effect.run_id, (response) => {
      const found = response.unknown_create_successor;
      return response.status === "claimed" && found.kind === "present"
        && found.successor_model_attempt_id != null
        && found.predecessor_reservation_released === false;
    });
    const before = successor(held);
    if (held.decision.kind !== "advisory") throw new Error("expected advisory");
    const decisionId = held.decision.decision_id;
    assert.equal(held.decision.text, ADVISORY);
    assert.equal(before.disposition, "dispatched");
    assert.equal(before.predecessor_usage_kind, "unknown");
    assertClosedSuccessor(before);
    assert.equal(await predecessor(prepared.projectId, created.effect.run_id), "none uncertain unknown false");
    assert.equal(await queryPostgres(`
      SELECT coalesce(bool_or(item->>'report' = 'host_fake_late_predecessor'), false)::text
        FROM storyos.model_attempts
       CROSS JOIN LATERAL jsonb_array_elements(coalesce(payload->'evidence', '[]'::jsonb)) AS item
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${created.effect.run_id}'::uuid
         AND attempt_role = 'decision';
    `), "false");
    unlinkSync(hold);
    await worker;
    const after = await inspect(started.baseUrl, prepared, created.effect.run_id);
    const found = successor(after);
    assert.equal(after.status, "completed");
    assert.equal(after.usage.kind, "unknown");
    if (after.decision.kind !== "advisory") throw new Error("expected advisory");
    assert.equal(after.decision.decision_id, decisionId);
    assert.equal(after.decision.text, ADVISORY);
    assert.equal(after.items.every((item) => item.call_id === null), true);
    assert.equal(found.predecessor_usage_kind, "reported");
    assert.equal(found.predecessor_reservation_released, true);
    assert.equal(found.successor_model_attempt_id, before.successor_model_attempt_id);
    assertClosedSuccessor(found);
    assert.equal(await predecessor(prepared.projectId, created.effect.run_id), "none uncertain reported true");
    assert.equal(await roles(prepared.projectId, created.effect.run_id), "1 1 1 1");
    assert.equal(await queryPostgres(`
      SELECT coalesce(bool_or(item->>'report' = 'host_fake_late_predecessor'), false)::text
        FROM storyos.model_attempts
       CROSS JOIN LATERAL jsonb_array_elements(coalesce(payload->'evidence', '[]'::jsonb)) AS item
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${created.effect.run_id}'::uuid
         AND attempt_role = 'decision';
    `), "true");
    assert.equal(await queryPostgres(`
      SELECT coalesce(decision_id::text, 'none')
        FROM storyos.model_attempts
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${created.effect.run_id}'::uuid
         AND attempt_role = 'decision';
    `), "none");
  } finally {
    if (existsSync(hold)) unlinkSync(hold);
    if (worker !== undefined) await Promise.allSettled([worker]);
    await stopRealServer(started.server);
  }
});
