// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/compact-active-context-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { test } from "vitest";

import {
  createAgentRun,
  digestCreateAgentRun,
  getAgentRun,
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
const FIRST = "Help with this passage.";
const CORRECTION = "I changed my mind: keep the voice.";

async function admit(
  baseUrl: string,
  prepared: Awaited<ReturnType<typeof prepare>>,
  key: string,
  text: string,
  conversation: CreateAgentRunRequest["create_agent_run_input"]["conversation"] = { kind: "new" },
  settle = true,
) {
  const request: CreateAgentRunRequest = {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation,
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
  if (settle) await settleOnce();
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

async function waitFor(
  probe: () => Promise<GetAgentRunResponse>,
  match: (value: GetAgentRunResponse) => boolean,
): Promise<GetAgentRunResponse> {
  let last: GetAgentRunResponse | undefined;
  for (let attempt = 0; attempt < 200; attempt += 1) {
    last = await probe();
    if (match(last)) return last;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  const state = last?.reference_recovery.kind ?? "none";
  throw new Error(`hold inspect timed out: status=${last?.status ?? "none"} recovery=${state}`);
}

async function recoveryCount(projectId: string, runId: string) {
  return queryPostgres(`
    SELECT count(*)::text
      FROM storyos.context_reference_recoveries
     WHERE project_id = '${projectId}'::uuid
       AND successor_run_id = '${runId}'::uuid;
  `);
}

async function inspect(
  baseUrl: string,
  prepared: Awaited<ReturnType<typeof prepare>>,
  runId: string,
): Promise<GetAgentRunResponse> {
  return getAgentRun({
    baseUrl,
    projectId: prepared.projectId,
    runId,
    fetchImpl: prepared.fetchImpl,
  });
}

function attempt(queried: GetAgentRunResponse) {
  if (queried.model_attempt.kind !== "present") throw new Error("expected attempt");
  return queried.model_attempt;
}

function produced(queried: GetAgentRunResponse) {
  if (queried.decision.kind !== "advisory" || queried.decision.continuation.kind !== "present") {
    throw new Error("expected produced continuation");
  }
  return queried.decision.continuation.continuation_binding_id;
}

function recovery(queried: GetAgentRunResponse) {
  if (queried.reference_recovery.kind !== "present") throw new Error("expected reference recovery");
  return queried.reference_recovery;
}

function selected(queried: GetAgentRunResponse, sourceClass: string) {
  return queried.context.selected.find((item) => item.source_class === sourceClass)?.content;
}

async function setBinding(projectId: string, bindingId: string, patch: string) {
  await queryPostgres(`
    UPDATE storyos.model_attempts
       SET payload = payload || jsonb_build_object(
             'produced_binding',
             COALESCE(payload->'produced_binding', '{}'::jsonb) || '${patch}'::jsonb
           )
     WHERE project_id = '${projectId}'::uuid
       AND continuation_binding_id = '${bindingId}'::uuid;
  `);
}

test("confirmed reference expiry rebuilds eligible context and keeps the old run", async () => {
  const started = await startRealServer();
  const hold = join(tmpdir(), `storyos-fake-expiry-${process.pid}`);
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e410"), "Expiry Novel", "e42");
    const first = await admit(started.baseUrl, prepared, id("e431"), FIRST);
    const firstInspect = await inspect(started.baseUrl, prepared, first.effect.run_id);
    const firstBinding = produced(firstInspect);
    assert.equal(firstInspect.status, "completed");
    assert.equal(firstInspect.reference_recovery.kind, "absent");
    assert.equal(attempt(firstInspect).input_mapping, "none");
    await setBinding(prepared.projectId, firstBinding, '{"reference_condition":"confirmed_expired"}');
    const rebuilt = await admit(started.baseUrl, prepared, id("e433"), CORRECTION, {
      kind: "existing",
      conversation_id: first.conversation_id,
    });
    const rebuiltInspect = await inspect(started.baseUrl, prepared, rebuilt.effect.run_id);
    const rebuiltRecovery = recovery(rebuiltInspect);
    assert.equal(rebuilt.conversation_id, first.conversation_id);
    assert.equal(rebuilt.effect.project_agent_id, first.effect.project_agent_id);
    assert.equal(rebuiltInspect.status, "completed");
    assert.equal(rebuiltRecovery.disposition, "rebuilt");
    assert.equal(rebuiltRecovery.block_reason, undefined);
    assert.equal(rebuiltRecovery.predecessor_run_id, first.effect.run_id);
    assert.equal(rebuiltRecovery.predecessor_continuation_binding_id, firstBinding);
    assert.equal(rebuiltRecovery.predecessor_terminal, true);
    assert.equal(rebuiltRecovery.lossless_provider_reconstruction, false);
    assert.equal(rebuiltRecovery.semantic_erasure, false);
    assert.equal(rebuiltRecovery.opaque_reused, false);
    assert.equal(rebuiltRecovery.covered_content_included, true);
    assert.equal(rebuiltRecovery.assembly_manifest_id, rebuiltInspect.context.assembly_manifest_id);
    assert.equal(rebuiltRecovery.model_attempt_id, attempt(rebuiltInspect).model_attempt_id);
    assert.equal(rebuiltRecovery.model_invocation_id, attempt(rebuiltInspect).model_invocation_id);
    assert.notEqual(rebuiltRecovery.model_invocation_id, attempt(firstInspect).model_invocation_id);
    assert.match(rebuiltRecovery.run_step_id ?? "", /^[0-9a-f-]{36}$/);
    assert.equal(attempt(rebuiltInspect).input_mapping, "full");
    assert.equal(attempt(rebuiltInspect).prior_continuation.kind, "absent");
    assert.deepEqual(attempt(rebuiltInspect).known_prior_continuation, {
      kind: "present",
      continuation_binding_id: firstBinding,
    });
    assert.equal(selected(rebuiltInspect, "author_instruction"), CORRECTION);
    const predecessor = await inspect(started.baseUrl, prepared, first.effect.run_id);
    assert.equal(predecessor.status, "completed");
    assert.equal(selected(predecessor, "author_instruction"), FIRST);
    assert.equal(produced(predecessor), firstBinding);

    const restartFirst = await admit(started.baseUrl, prepared, id("e449"), FIRST);
    const restartBinding = produced(await inspect(started.baseUrl, prepared, restartFirst.effect.run_id));
    await setBinding(prepared.projectId, restartBinding, '{"reference_condition":"confirmed_expired"}');
    writeFileSync(hold, "hold");
    const restart = await admit(started.baseUrl, prepared, id("e44b"), CORRECTION, {
      kind: "existing",
      conversation_id: restartFirst.conversation_id,
    }, false);
    const restartWorker = settleHeld({ STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: hold });
    const held = await waitFor(
      () => inspect(started.baseUrl, prepared, restart.effect.run_id),
      (current) => current.status === "claimed" && current.reference_recovery.kind === "present",
    );
    const heldRecovery = recovery(held);
    assert.equal(heldRecovery.disposition, "rebuilt");
    assert.equal(held.model_attempt.kind, "present");
    assert.equal(held.decision.kind, "absent");
    assert.equal(await recoveryCount(prepared.projectId, restart.effect.run_id), "1");
    unlinkSync(hold);
    await restartWorker;
    const resumed = await inspect(started.baseUrl, prepared, restart.effect.run_id);
    assert.equal(resumed.status, "completed");
    assert.equal(recovery(resumed).recovery_id, heldRecovery.recovery_id);
    assert.equal(recovery(resumed).model_invocation_id, heldRecovery.model_invocation_id);
    assert.equal(await recoveryCount(prepared.projectId, restart.effect.run_id), "1");
    assert.equal((await inspect(started.baseUrl, prepared, restartFirst.effect.run_id)).status, "completed");

    const unknownFirst = await admit(started.baseUrl, prepared, id("e435"), FIRST);
    const unknownBinding = produced(await inspect(started.baseUrl, prepared, unknownFirst.effect.run_id));
    await setBinding(prepared.projectId, unknownBinding, '{"reference_condition":"confirmed_expired"}');
    await queryPostgres(`
      UPDATE storyos.model_attempts
         SET dispatch_state = 'uncertain'
       WHERE project_id = '${prepared.projectId}'::uuid
         AND continuation_binding_id = '${unknownBinding}'::uuid;
    `);
    const unknown = await admit(started.baseUrl, prepared, id("e437"), CORRECTION, {
      kind: "existing",
      conversation_id: unknownFirst.conversation_id,
    });
    const unknownInspect = await inspect(started.baseUrl, prepared, unknown.effect.run_id);
    assert.equal(unknownInspect.status, "refused");
    assert.deepEqual(unknownInspect.decision, {
      kind: "execution_refused",
      capability: "expiry_rebuild_unknown_create",
    });
    assert.equal(unknownInspect.model_attempt.kind, "absent");
    assert.equal(recovery(unknownInspect).disposition, "unknown_create");
    assert.equal(recovery(unknownInspect).opaque_reused, false);

    const budgetFirst = await admit(started.baseUrl, prepared, id("e439"), FIRST);
    const budgetBinding = produced(await inspect(started.baseUrl, prepared, budgetFirst.effect.run_id));
    await setBinding(
      prepared.projectId,
      budgetBinding,
      '{"reference_condition":"confirmed_expired","budget_exhausted":true}',
    );
    const budget = await admit(started.baseUrl, prepared, id("e43b"), CORRECTION, {
      kind: "existing",
      conversation_id: budgetFirst.conversation_id,
    });
    const budgetInspect = await inspect(started.baseUrl, prepared, budget.effect.run_id);
    assert.equal(budgetInspect.status, "refused");
    assert.deepEqual(budgetInspect.decision, {
      kind: "execution_refused",
      capability: "expiry_rebuild_budget_insufficient",
    });
    assert.equal(budgetInspect.model_attempt.kind, "absent");
    assert.equal(recovery(budgetInspect).disposition, "blocked");
    assert.equal(recovery(budgetInspect).block_reason, "budget_insufficient");

    const boundaryFirst = await admit(started.baseUrl, prepared, id("e43d"), FIRST);
    const boundaryBinding = produced(await inspect(started.baseUrl, prepared, boundaryFirst.effect.run_id));
    await setBinding(
      prepared.projectId,
      boundaryBinding,
      '{"reference_condition":"confirmed_unusable","processing_destination_identity":"018f0000-0000-7001-8000-00000000dead"}',
    );
    const boundary = await admit(started.baseUrl, prepared, id("e43f"), CORRECTION, {
      kind: "existing",
      conversation_id: boundaryFirst.conversation_id,
    });
    const boundaryInspect = await inspect(started.baseUrl, prepared, boundary.effect.run_id);
    assert.equal(boundaryInspect.status, "refused");
    assert.deepEqual(boundaryInspect.decision, {
      kind: "execution_refused",
      capability: "expiry_rebuild_processing_boundary_changed",
    });
    assert.equal(recovery(boundaryInspect).disposition, "blocked");
    assert.equal(recovery(boundaryInspect).block_reason, "processing_boundary_changed");
    assert.equal(boundaryInspect.model_attempt.kind, "absent");

    const copyFirst = await admit(started.baseUrl, prepared, id("e441"), FIRST);
    const copyBinding = produced(await inspect(started.baseUrl, prepared, copyFirst.effect.run_id));
    await setBinding(
      prepared.projectId,
      copyBinding,
      '{"reference_condition":"confirmed_expired","covered_copy_restricted":true}',
    );
    const copied = await admit(started.baseUrl, prepared, id("e443"), CORRECTION, {
      kind: "existing",
      conversation_id: copyFirst.conversation_id,
    });
    const copyInspect = await inspect(started.baseUrl, prepared, copied.effect.run_id);
    const copyRecovery = recovery(copyInspect);
    assert.equal(copyInspect.status, "completed");
    assert.equal(copyRecovery.disposition, "rebuilt");
    assert.equal(copyRecovery.opaque_reused, false);
    assert.equal(copyRecovery.covered_content_included, false);
    assert.equal(copyRecovery.semantic_erasure, false);
    assert.equal(attempt(copyInspect).prior_continuation.kind, "absent");
    assert.equal(selected(copyInspect, "author_instruction"), CORRECTION);

    const missingFirst = await admit(started.baseUrl, prepared, id("e445"), FIRST);
    const missingBinding = produced(await inspect(started.baseUrl, prepared, missingFirst.effect.run_id));
    await setBinding(prepared.projectId, missingBinding, '{"reference_condition":"confirmed_expired"}');
    await queryPostgres(`
      UPDATE storyos.authoritative_payloads AS payload
         SET canonical_bytes = convert_to(repeat('a', 10001), 'UTF8')
        FROM storyos.authoritative_heads AS head
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id, revision.revision_id)
           = (head.owner_user_id, head.project_id, head.manuscript_object_id, head.current_revision_id)
       WHERE (payload.owner_user_id, payload.project_id, payload.payload_id)
           = (revision.owner_user_id, revision.project_id, revision.payload_id)
         AND head.project_id = '${prepared.projectId}'::uuid
         AND head.manuscript_object_id = '${prepared.chapterId}'::uuid;
    `);
    const missing = await admit(started.baseUrl, prepared, id("e447"), CORRECTION, {
      kind: "existing",
      conversation_id: missingFirst.conversation_id,
    });
    const missingInspect = await inspect(started.baseUrl, prepared, missing.effect.run_id);
    assert.equal(missingInspect.status, "refused");
    assert.equal(recovery(missingInspect).disposition, "blocked");
    assert.equal(recovery(missingInspect).block_reason, "required_input_missing");
    assert.equal(missingInspect.model_attempt.kind, "absent");

    await assert.rejects(
      () => getAgentRun({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        runId: rebuilt.effect.run_id,
        fetchImpl: browserFetch(started.baseUrl, "session-b"),
      }),
      (error) => requireStoryOSProtocolError(error).status === 404
        && !String(requireStoryOSProtocolError(error).responseBody).includes(USER_A),
    );
  } finally {
    if (existsSync(hold)) unlinkSync(hold);
    await stopRealServer(started.server);
  }
});
