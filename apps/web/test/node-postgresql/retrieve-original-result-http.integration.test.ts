// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/rebuild-expired-reference-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
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

function settleHeld(extraEnv: Readonly<Record<string, string>>): Promise<void> {
  const env = { ...process.env, ...extraEnv };
  if (process.env.STORYOS_TEST_DATABASE_URL !== undefined) {
    env.STORYOS_DATABASE_URL = process.env.STORYOS_TEST_DATABASE_URL;
  }
  const settled = execFileAsync(workerBin, ["--once"], {
    cwd: repositoryRoot,
    env,
    timeout: 60_000,
    killSignal: "SIGKILL",
  }).then(() => undefined);
  void settled.catch(() => undefined);
  return settled;
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

function retrieval(response: GetAgentRunResponse) {
  if (response.original_result_retrieval.kind !== "present") {
    throw new Error("expected original-result retrieval");
  }
  return response.original_result_retrieval;
}

async function attemptCounts(projectId: string, runId: string) {
  return queryPostgres(`
    SELECT count(*) FILTER (WHERE attempt_role = 'decision')::text
        || ' ' || count(*) FILTER (WHERE attempt_role = 'retrieval')::text
      FROM storyos.model_attempts
     WHERE project_id = '${projectId}'::uuid
       AND run_id = '${runId}'::uuid;
  `);
}

test("a retained complete result settles once and a repeat keeps that identity", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("a101"), "Retrieve Complete Novel", "a1");
    const created = await admit(started.baseUrl, prepared, id("a111"), "SCRIPT:retrieve-complete");
    await settleOnce();
    const first = await inspect(started.baseUrl, prepared, created.effect.run_id);
    const found = retrieval(first);
    assert.equal(first.status, "completed");
    assert.equal(first.usage.kind, "unknown");
    assert.equal(first.decision.kind, "advisory");
    if (first.decision.kind !== "advisory") throw new Error("expected advisory");
    assert.equal(first.decision.text, ADVISORY);
    assert.equal(first.decision.continuation.kind, "present");
    assert.equal(first.model_attempt.kind, "present");
    if (first.model_attempt.kind !== "present") throw new Error("expected attempt");
    assert.equal(first.model_attempt.dispatch_state, "settled");
    assert.equal(found.disposition, "settled");
    assert.equal(found.keep_reason, undefined);
    assert.equal(found.original_model_attempt_id, first.model_attempt.model_attempt_id);
    assert.match(found.response_reference_id ?? "", /^[0-9a-f-]{36}$/);
    assert.match(found.retrieval_attempt_id ?? "", /^[0-9a-f-]{36}$/);
    assert.notEqual(found.retrieval_attempt_id, found.original_model_attempt_id);
    assert.equal(found.repeats_original_create, false);
    assert.equal(found.resumes_stream, false);
    assert.equal(found.proves_create_idempotency, false);
    assert.equal(found.supplies_decision, true);
    assert.equal(found.supplies_tool_call, false);
    assert.equal(found.advances_continuation, true);
    assert.equal(found.reservation_released, true);
    assert.equal(found.usage_kind, "unknown");
    assert.equal(first.items.every((item) => item.call_id === null), true);
    assert.equal(await attemptCounts(prepared.projectId, created.effect.run_id), "1 1");
    assert.equal(await queryPostgres(`
      SELECT payload->'wire'->>'author_message'
        FROM storyos.model_attempts
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${created.effect.run_id}'::uuid
         AND attempt_role = 'decision';
    `), "SCRIPT:retrieve-complete");
    assert.equal(await queryPostgres(`
      SELECT (payload->>'operation') || ' ' || (payload->>'repeats_original_create')
        FROM storyos.model_attempts
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${created.effect.run_id}'::uuid
         AND attempt_role = 'retrieval';
    `), "retrieve_original_result false");
    assert.equal(await queryPostgres(`
      SELECT (payload->'evidence'->0->>'kind')
          || ' ' || (payload->'evidence'->1->>'kind')
          || ' ' || (payload->'evidence'->2->>'kind')
          || ' ' || (payload->>'bounds')
        FROM storyos.model_attempts
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${created.effect.run_id}'::uuid
         AND attempt_role = 'retrieval';
    `), "stored_reference provider_report provider_opaque declared_read_only");
    const again = await inspect(started.baseUrl, prepared, created.effect.run_id);
    assert.equal(retrieval(again).reconciliation_id, found.reconciliation_id);
    await queryPostgres(`
      UPDATE storyos.agent_runs
         SET status = 'queued', lease_expires_at = NULL, wakeup_pending = true
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${created.effect.run_id}'::uuid;
    `);
    await settleOnce();
    const repeated = await inspect(started.baseUrl, prepared, created.effect.run_id);
    assert.equal(repeated.status, "completed");
    assert.equal(retrieval(repeated).reconciliation_id, found.reconciliation_id);
    assert.equal(retrieval(repeated).retrieval_attempt_id, found.retrieval_attempt_id);
    if (repeated.decision.kind !== "advisory") throw new Error("expected advisory");
    assert.equal(repeated.decision.decision_id, first.decision.decision_id);
    assert.equal(await attemptCounts(prepared.projectId, created.effect.run_id), "1 1");
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

test("missing, unsupported, unbounded, foreign, incomplete, and unknown results stay unresolved", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("a201"), "Retrieve Unknown Novel", "a2");
    const cases = [
      ["SCRIPT:retrieve-missing", "missing_reference", "0"],
      ["SCRIPT:retrieve-unsupported", "unsupported_retrieval", "0"],
      ["SCRIPT:retrieve-unbounded", "unknown_bounds", "0"],
      ["SCRIPT:retrieve-foreign-scope", "scope_mismatch", "0"],
      ["SCRIPT:retrieve-foreign-conversation", "conversation_mismatch", "0"],
      ["SCRIPT:retrieve-foreign-destination", "destination_mismatch", "0"],
      ["SCRIPT:retrieve-foreign-mapping", "mapping_mismatch", "0"],
      ["SCRIPT:retrieve-incomplete", "incomplete_result", "1"],
      ["SCRIPT:retrieve-unknown", "unknown_result", "1"],
    ] as const;
    for (const [index, [script, reason, retrievalCount]] of cases.entries()) {
      const created = await admit(started.baseUrl, prepared, id(`a2b${index}`), script);
      await settleOnce();
      const inspected = await inspect(started.baseUrl, prepared, created.effect.run_id);
      const found = retrieval(inspected);
      assert.equal(inspected.status, "waiting", script);
      assert.equal(inspected.decision.kind, "absent", script);
      assert.equal(inspected.usage.kind, "unknown", script);
      assert.equal(inspected.model_attempt.kind, "present", script);
      if (inspected.model_attempt.kind !== "present") throw new Error(script);
      assert.equal(inspected.model_attempt.dispatch_state, "uncertain", script);
      assert.equal(found.disposition, "kept_unknown", script);
      assert.equal(found.keep_reason, reason, script);
      assert.equal(found.supplies_decision, false, script);
      assert.equal(found.supplies_tool_call, false, script);
      assert.equal(found.advances_continuation, false, script);
      assert.equal(found.reservation_released, false, script);
      assert.equal(found.repeats_original_create, false, script);
      assert.equal(found.original_model_attempt_id, inspected.model_attempt.model_attempt_id, script);
      assert.equal(await attemptCounts(prepared.projectId, created.effect.run_id), `1 ${retrievalCount}`, script);
      assert.equal(await queryPostgres(`
        SELECT payload->'reservation'->>'released'
          FROM storyos.model_attempts
         WHERE project_id = '${prepared.projectId}'::uuid
           AND run_id = '${created.effect.run_id}'::uuid
           AND attempt_role = 'decision';
      `), "false", script);
      const reloaded = await inspect(started.baseUrl, prepared, created.effect.run_id);
      assert.equal(retrieval(reloaded).reconciliation_id, found.reconciliation_id, script);
      await queryPostgres(`
        UPDATE storyos.agent_runs
           SET status = 'cancelled', wakeup_pending = false, lease_expires_at = NULL
         WHERE project_id = '${prepared.projectId}'::uuid
           AND run_id = '${created.effect.run_id}'::uuid;
      `);
    }
  } finally {
    await stopRealServer(started.server);
  }
});

test("a cancelled Run reconciles the late result as evidence only", async () => {
  const hold = join(tmpdir(), "storyos-s3-23-dispatch.hold");
  const started = await startRealServer();
  let worker: Promise<void> | undefined;
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("a301"), "Retrieve Fence Novel", "a3");
    const created = await admit(started.baseUrl, prepared, id("a311"), "SCRIPT:retrieve-complete");
    writeFileSync(hold, "hold");
    worker = settleHeld({ STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: hold });
    let held: GetAgentRunResponse | undefined;
    for (let attempt = 0; attempt < 200; attempt += 1) {
      held = await inspect(started.baseUrl, prepared, created.effect.run_id);
      if (held.status === "claimed" && held.model_attempt.kind === "present"
        && held.original_result_retrieval.kind === "absent") break;
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
    if (held?.model_attempt.kind !== "present") throw new Error("expected the original Attempt");
    const originalAttemptId = held.model_attempt.model_attempt_id;
    const request: CancelAgentRunRequest = {
      command_schema: "storyos.command.cancel-agent-run.request.v1",
      cancel_agent_run_input: { ...BINDING, correlation_id: id("a321") },
    };
    const cancelled = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
      request.command_schema,
      await digestCancelAgentRun(request),
      id("a331"),
      (antiForgery) => cancelAgentRun({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        runId: created.effect.run_id,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("a331"),
        antiForgery,
        request,
      }),
    );
    assert.equal(cancelled.effect.kind, "applied");
    unlinkSync(hold);
    await worker;
    const after = await inspect(started.baseUrl, prepared, created.effect.run_id);
    const found = retrieval(after);
    assert.equal(after.status, "cancelled");
    assert.equal(after.decision.kind, "absent");
    assert.equal(after.items.length, 0);
    assert.equal(after.usage.kind, "unknown");
    if (after.model_attempt.kind !== "present") throw new Error("expected original Attempt");
    assert.equal(after.model_attempt.model_attempt_id, originalAttemptId);
    assert.equal(after.model_attempt.dispatch_state, "uncertain");
    assert.equal(found.disposition, "evidence_only");
    assert.equal(found.supplies_decision, false);
    assert.equal(found.supplies_tool_call, false);
    assert.equal(found.advances_continuation, false);
    assert.equal(found.reservation_released, false);
    assert.equal(found.repeats_original_create, false);
    assert.equal(found.resumes_stream, false);
    assert.equal(await attemptCounts(prepared.projectId, created.effect.run_id), "1 1");
    assert.equal(await queryPostgres(`
      SELECT payload->'wire'->>'author_message'
          || ' ' || coalesce(continuation_binding_id::text, 'none')
        FROM storyos.model_attempts
       WHERE project_id = '${prepared.projectId}'::uuid
         AND model_attempt_id = '${originalAttemptId}'::uuid;
    `), "SCRIPT:retrieve-complete none");
    assert.equal(await queryPostgres(`
      SELECT payload->'items'->0->>'text'
        FROM storyos.model_attempts
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${created.effect.run_id}'::uuid
         AND attempt_role = 'retrieval';
    `), ADVISORY);
    const repeatRequest: CancelAgentRunRequest = {
      command_schema: "storyos.command.cancel-agent-run.request.v1",
      cancel_agent_run_input: { ...BINDING, correlation_id: id("a341") },
    };
    const repeated = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
      repeatRequest.command_schema,
      await digestCancelAgentRun(repeatRequest),
      id("a351"),
      (antiForgery) => cancelAgentRun({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        runId: created.effect.run_id,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("a351"),
        antiForgery,
        request: repeatRequest,
      }),
    );
    assert.equal(repeated.effect.kind, "no_effect");
    const reloaded = await inspect(started.baseUrl, prepared, created.effect.run_id);
    assert.equal(retrieval(reloaded).reconciliation_id, found.reconciliation_id);
    assert.equal(await attemptCounts(prepared.projectId, created.effect.run_id), "1 1");
  } finally {
    if (existsSync(hold)) unlinkSync(hold);
    if (worker !== undefined) await Promise.allSettled([worker]);
    await stopRealServer(started.server);
  }
});
