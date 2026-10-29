// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/continue-conversation-input-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
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
  settleHeld,
  settleOnce,
  startServer,
  waitFor,
} from "../support/http-harness.ts";

const ORDINARY = "Help with this passage.";
const COMPACT = "Compact active context between calls.";
const ADVICE = "This passage is inspectable Host-fake advice. It is not Authoritative State.";
const OUTPUT = "Bounded later-request summary. Semantic preservation is unknown.";

async function admit(
  baseUrl: string,
  prepared: Awaited<ReturnType<typeof prepare>>,
  key: string,
  text: string,
  conversation: CreateAgentRunRequest["create_agent_run_input"]["conversation"] = { kind: "new" },
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
  return created;
}

async function inspect(
  baseUrl: string,
  prepared: Awaited<ReturnType<typeof prepare>>,
  runId: string,
  modelAttemptId?: string,
): Promise<GetAgentRunResponse> {
  return getAgentRun({
    baseUrl,
    projectId: prepared.projectId,
    runId,
    fetchImpl: prepared.fetchImpl,
    ...(modelAttemptId === undefined ? {} : { modelAttemptId }),
  });
}

function selected(queried: GetAgentRunResponse, sourceClass: string) {
  return queried.context.selected.find((item) => item.source_class === sourceClass)?.content;
}

function compaction(queried: GetAgentRunResponse) {
  if (queried.active_compaction.kind !== "present") throw new Error("expected compaction");
  return queried.active_compaction;
}

function attempt(queried: GetAgentRunResponse) {
  if (queried.model_attempt.kind !== "present") throw new Error("expected attempt");
  return queried.model_attempt;
}

function decisionAttempt(queried: GetAgentRunResponse) {
  return attempt(queried).model_attempt_id;
}

async function rewriteChapter(projectId: string, chapterId: string, body: string) {
  await queryPostgres(`
    UPDATE storyos.authoritative_payloads AS payload
       SET canonical_bytes = convert_to(${body}, 'UTF8')
      FROM storyos.authoritative_heads AS head
      JOIN storyos.authoritative_revisions AS revision
        ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id, revision.revision_id)
         = (head.owner_user_id, head.project_id, head.manuscript_object_id, head.current_revision_id)
     WHERE (payload.owner_user_id, payload.project_id, payload.payload_id)
         = (revision.owner_user_id, revision.project_id, revision.payload_id)
       AND head.project_id = '${projectId}'::uuid
       AND head.manuscript_object_id = '${chapterId}'::uuid;
  `);
}

async function attemptRoles(projectId: string, runId: string) {
  return queryPostgres(`
    SELECT string_agg(attempt_role, ',' ORDER BY attempt_role)
      FROM storyos.model_attempts
     WHERE project_id = '${projectId}'::uuid
       AND run_id = '${runId}'::uuid;
  `);
}

// The role and the first selected source class of the Context Assembly Manifest that the
// decision request of one AgentRun disclosed.
async function sentAssembly(projectId: string, runId: string) {
  return queryPostgres(`
    SELECT assembly.manifest_role || ',' || (assembly.payload->'selected'->0->>'source_class')
      FROM storyos.model_attempts AS attempt
      JOIN storyos.context_assembly_manifests AS assembly
        ON assembly.destination_context_manifest_id = attempt.destination_context_manifest_id
     WHERE attempt.project_id = '${projectId}'::uuid
       AND attempt.run_id = '${runId}'::uuid
       AND attempt.attempt_role = 'decision';
  `);
}

test("active context compaction summarizes the prior response, then installs or refuses for the decision request", async () => {
  const started = await startServer();
  const hold = join(tmpdir(), `storyos-fake-compaction-${process.pid}`);
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("d210"), "Compact Novel", "d21");
    const first = await admit(started.baseUrl, prepared, id("d217"), COMPACT);
    await settleOnce();
    const firstInspect = await inspect(started.baseUrl, prepared, first.effect.run_id);
    assert.equal(firstInspect.status, "completed");
    assert.equal(firstInspect.active_compaction.kind, "absent");
    assert.equal(await attemptRoles(prepared.projectId, first.effect.run_id), "decision");
    const conversation = { kind: "existing" as const, conversation_id: first.conversation_id };
    const ordinary = await admit(started.baseUrl, prepared, id("d211"), ORDINARY, conversation);
    await settleOnce();
    const ordinaryInspect = await inspect(started.baseUrl, prepared, ordinary.effect.run_id);
    assert.equal(ordinaryInspect.status, "completed");
    assert.equal(ordinaryInspect.active_compaction.kind, "absent");
    assert.equal(attempt(ordinaryInspect).input_mapping, "incremental");

    // Compact, and hold after the summary stages.
    async function heldCompaction(key: string) {
      writeFileSync(hold, "hold");
      const run = await admit(started.baseUrl, prepared, id(key), COMPACT, conversation);
      const worker = settleHeld({ STORYOS_TEST_FAKE_COMPACTION_STAGE_HOLD_PATH: hold });
      const staged = await waitFor(
        () => inspect(started.baseUrl, prepared, run.effect.run_id),
        (current) => current.status === "claimed" && current.active_compaction.kind === "present" && current.active_compaction.install_state === "staged",
      );
      return { run, worker, staged };
    }

    const happy = await heldCompaction("d212");
    const stagedCompaction = compaction(happy.staged);
    assert.equal(happy.staged.model_attempt.kind, "absent");
    assert.equal(stagedCompaction.installed.kind, "absent");
    assert.equal(stagedCompaction.prior_model_attempt_id, decisionAttempt(ordinaryInspect));
    assert.equal(await attemptRoles(prepared.projectId, happy.run.effect.run_id), "compaction");
    unlinkSync(hold);
    await happy.worker;
    const installed = await inspect(started.baseUrl, prepared, happy.run.effect.run_id);
    const installedCompaction = compaction(installed);
    assert.equal(installed.status, "completed");
    assert.equal(installedCompaction.install_state, "installed");
    assert.equal(installedCompaction.producer, "host_fake_summary");
    assert.equal(installedCompaction.mapping_kind, "host_managed");
    assert.equal(installedCompaction.mapping_revision, "storyos.host-fake.mapping.v1");
    assert.equal(installedCompaction.output_text, OUTPUT);
    assert.equal(installedCompaction.usage.kind, "unknown");
    assert.deepEqual(installedCompaction.loss_facts, ["semantic_preservation_unknown"]);
    assert.equal(installedCompaction.admission.adapter_mapping, "storyos.host-fake.mapping.v1");
    assert.deepEqual(installedCompaction.preserved_item_ids, ["1"]);
    if (installedCompaction.installed.kind !== "present") throw new Error("expected install");
    assert.equal(installedCompaction.installed.model_attempt_id, decisionAttempt(installed));
    assert.equal(installedCompaction.installed.model_invocation_id, attempt(installed).model_invocation_id);
    assert.notEqual(installedCompaction.installed.model_attempt_id, installedCompaction.producer_model_attempt_id);
    assert.notEqual(installedCompaction.installed.run_step_id, installedCompaction.prior_run_step_id);
    assert.notEqual(installedCompaction.installed.model_invocation_id, installedCompaction.producer_invocation_id);
    assert.equal(attempt(installed).input_mapping, "new_transport");
    assert.equal(attempt(installed).prior_continuation.kind, "absent");
    assert.equal(selected(installed, "author_instruction"), COMPACT);
    assert.equal(selected(installed, "working_target"), "");
    assert.equal(selected(installed, "conversation_summary"), undefined);
    assert.equal(installed.decision.kind, "advisory");
    assert.equal(installed.items[0]?.text, ADVICE);
    assert.equal(installed.memory_settings_revision, happy.staged.memory_settings_revision);
    assert.equal(await attemptRoles(prepared.projectId, happy.run.effect.run_id), "compaction,decision");
    assert.equal(await sentAssembly(prepared.projectId, happy.run.effect.run_id), "later_request,conversation_summary");
    await assert.rejects(
      () => inspect(started.baseUrl, prepared, happy.run.effect.run_id, installedCompaction.producer_model_attempt_id),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );
    await assert.rejects(
      () => getAgentRun({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        runId: happy.run.effect.run_id,
        fetchImpl: browserFetch(started.baseUrl, "session-b"),
      }),
      (error) => requireStoryOSProtocolError(error).status === 404
        && !String(requireStoryOSProtocolError(error).responseBody).includes(USER_A),
    );

    const changed = await heldCompaction("d213");
    await rewriteChapter(prepared.projectId, prepared.chapterId, "'Short rewrite.'");
    unlinkSync(hold);
    await changed.worker;
    const changedInspect = await inspect(started.baseUrl, prepared, changed.run.effect.run_id);
    const changedCompaction = compaction(changedInspect);
    assert.equal(changedInspect.status, "completed");
    assert.equal(changedCompaction.install_state, "refused");
    assert.equal(changedCompaction.refusal_reason, "changed_input");
    assert.equal(changedCompaction.installed.kind, "absent");
    assert.equal(attempt(changedInspect).input_mapping, "incremental");
    assert.equal(selected(changedInspect, "author_instruction"), COMPACT);
    assert.equal(await attemptRoles(prepared.projectId, changed.run.effect.run_id), "compaction,decision");
    assert.equal(await sentAssembly(prepared.projectId, changed.run.effect.run_id), "decision,author_instruction");

    const restricted = await heldCompaction("d214");
    await queryPostgres(`
      UPDATE storyos.model_attempts
         SET payload = jsonb_set(payload, '{produced_binding,covered_copy_restricted}', 'true')
       WHERE project_id = '${prepared.projectId}'::uuid
         AND model_attempt_id = '${compaction(restricted.staged).prior_model_attempt_id}'::uuid;
    `);
    unlinkSync(hold);
    await restricted.worker;
    const restrictedInspect = await inspect(started.baseUrl, prepared, restricted.run.effect.run_id);
    assert.equal(restrictedInspect.status, "completed");
    assert.equal(compaction(restrictedInspect).install_state, "refused");
    assert.equal(compaction(restrictedInspect).refusal_reason, "restricted_source");
    assert.equal(compaction(restrictedInspect).installed.kind, "absent");
    assert.equal(attempt(restrictedInspect).input_mapping, "new_transport");

    const exact = await heldCompaction("d215");
    await rewriteChapter(prepared.projectId, prepared.chapterId, "repeat('a', 32001)");
    unlinkSync(hold);
    await exact.worker;
    const exactInspect = await inspect(started.baseUrl, prepared, exact.run.effect.run_id);
    assert.equal(exactInspect.status, "completed");
    assert.equal(compaction(exactInspect).install_state, "refused");
    assert.equal(compaction(exactInspect).refusal_reason, "exact_required_unsatisfied");
    assert.equal(compaction(exactInspect).installed.kind, "absent");

    await rewriteChapter(prepared.projectId, prepared.chapterId, "''");
    const stale = await heldCompaction("d216");
    await queryPostgres(`
      UPDATE storyos.agent_runs
         SET fence_token = fence_token + 1,
             lease_expires_at = clock_timestamp() - interval '1 second'
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${stale.run.effect.run_id}'::uuid;
    `);
    unlinkSync(hold);
    await stale.worker;
    const stillStaged = await inspect(started.baseUrl, prepared, stale.run.effect.run_id);
    assert.equal(stillStaged.status, "claimed");
    assert.equal(compaction(stillStaged).install_state, "staged");
    assert.equal(stillStaged.model_attempt.kind, "absent");
    await settleOnce();
    const reclaimed = await inspect(started.baseUrl, prepared, stale.run.effect.run_id);
    assert.equal(reclaimed.status, "completed");
    assert.equal(compaction(reclaimed).install_state, "installed");
    assert.equal(attempt(reclaimed).input_mapping, "new_transport");
    assert.equal(selected(reclaimed, "author_instruction"), COMPACT);
  } finally {
    if (existsSync(hold)) unlinkSync(hold);
    await stopRealServer(started.server);
  }
});
