// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/continue-conversation-input-http.integration.test.ts"]}
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
const ORDINARY = "Help with this passage.";
const COMPACT = "Compact active context between calls.";
const ADVICE = "This passage is inspectable Host-fake advice. It is not Authoritative State.";
const OUTPUT = "Bounded later-request summary. Semantic preservation is unknown.";

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

function decisionAttempt(queried: GetAgentRunResponse) {
  if (queried.model_attempt.kind !== "present") throw new Error("expected attempt");
  return queried.model_attempt.model_attempt_id;
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
  const state = last?.active_compaction.kind === "present" ? last.active_compaction.install_state : "absent";
  throw new Error(`hold inspect timed out: status=${last?.status ?? "none"} compaction=${state}`);
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

async function attemptRoles(projectId: string, runId: string) {
  return queryPostgres(`
    SELECT string_agg(attempt_role, ',' ORDER BY attempt_role)
      FROM storyos.model_attempts
     WHERE project_id = '${projectId}'::uuid
       AND run_id = '${runId}'::uuid;
  `);
}

test("active context compaction stages, then installs or refuses for a later request", async () => {
  const started = await startRealServer();
  const hold = join(tmpdir(), `storyos-fake-compaction-${process.pid}`);
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("d210"), "Compact Novel", "d21");
    const ordinary = await admit(started.baseUrl, prepared, id("d211"), ORDINARY);
    await settleOnce();
    const ordinaryInspect = await inspect(started.baseUrl, prepared, ordinary.effect.run_id);
    assert.equal(ordinaryInspect.status, "completed");
    assert.equal(ordinaryInspect.active_compaction.kind, "absent");
    assert.equal(await attemptRoles(prepared.projectId, ordinary.effect.run_id), "decision");

    writeFileSync(hold, "hold");
    const happy = await admit(started.baseUrl, prepared, id("d212"), COMPACT);
    const happyWorker = settleHeld({ STORYOS_TEST_FAKE_COMPACTION_STAGE_HOLD_PATH: hold });
    const staged = await waitFor(
      () => inspect(started.baseUrl, prepared, happy.effect.run_id),
      (current) => current.status === "claimed" && current.active_compaction.kind === "present" && current.active_compaction.install_state === "staged",
    );
    const stagedCompaction = compaction(staged);
    const decisionAttemptId = decisionAttempt(staged);
    assert.equal(stagedCompaction.installed.kind, "absent");
    assert.equal(stagedCompaction.prior_model_attempt_id, decisionAttemptId);
    assert.equal(staged.decision.kind, "advisory");
    assert.equal(selected(staged, "author_instruction"), COMPACT);
    assert.equal(selected(staged, "working_target"), "");
    assert.equal(staged.items[0]?.text, ADVICE);
    unlinkSync(hold);
    await happyWorker;
    const installed = await inspect(started.baseUrl, prepared, happy.effect.run_id);
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
    assert.equal(installedCompaction.installed.kind, "present");
    if (installedCompaction.installed.kind !== "present") throw new Error("expected install");
    assert.notEqual(installedCompaction.producer_model_attempt_id, decisionAttemptId);
    assert.notEqual(installedCompaction.installed.model_attempt_id, decisionAttemptId);
    assert.notEqual(installedCompaction.installed.model_attempt_id, installedCompaction.producer_model_attempt_id);
    assert.notEqual(installedCompaction.installed.run_step_id, installedCompaction.prior_run_step_id);
    assert.notEqual(installedCompaction.installed.model_invocation_id, installedCompaction.producer_invocation_id);
    assert.equal(decisionAttempt(installed), decisionAttemptId);
    assert.equal(selected(installed, "author_instruction"), COMPACT);
    assert.equal(selected(installed, "working_target"), "");
    assert.equal(installed.decision.kind, "advisory");
    assert.equal(installed.items[0]?.text, ADVICE);
    assert.equal(installed.memory_settings_revision, staged.memory_settings_revision);
    assert.equal(await attemptRoles(prepared.projectId, happy.effect.run_id), "compaction,decision,later_request");
    await assert.rejects(
      () => inspect(started.baseUrl, prepared, happy.effect.run_id, installedCompaction.producer_model_attempt_id),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );
    await assert.rejects(
      () => getAgentRun({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        runId: happy.effect.run_id,
        fetchImpl: browserFetch(started.baseUrl, "session-b"),
      }),
      (error) => requireStoryOSProtocolError(error).status === 404
        && !String(requireStoryOSProtocolError(error).responseBody).includes(USER_A),
    );

    writeFileSync(hold, "hold");
    const changed = await admit(started.baseUrl, prepared, id("d213"), COMPACT);
    const changedWorker = settleHeld({ STORYOS_TEST_FAKE_COMPACTION_STAGE_HOLD_PATH: hold });
    await waitFor(
      () => inspect(started.baseUrl, prepared, changed.effect.run_id),
      (current) => current.active_compaction.kind === "present" && current.active_compaction.install_state === "staged",
    );
    await rewriteChapter(prepared.projectId, prepared.chapterId, "'Short rewrite.'");
    unlinkSync(hold);
    await changedWorker;
    const changedInspect = await inspect(started.baseUrl, prepared, changed.effect.run_id);
    const changedCompaction = compaction(changedInspect);
    assert.equal(changedInspect.status, "completed");
    assert.equal(changedCompaction.install_state, "refused");
    assert.equal(changedCompaction.refusal_reason, "changed_input");
    assert.equal(changedCompaction.installed.kind, "absent");
    assert.equal(selected(changedInspect, "author_instruction"), COMPACT);
    assert.equal(await attemptRoles(prepared.projectId, changed.effect.run_id), "compaction,decision");

    writeFileSync(hold, "hold");
    const restricted = await admit(started.baseUrl, prepared, id("d214"), COMPACT);
    const restrictedWorker = settleHeld({ STORYOS_TEST_FAKE_COMPACTION_STAGE_HOLD_PATH: hold });
    await waitFor(
      () => inspect(started.baseUrl, prepared, restricted.effect.run_id),
      (current) => current.active_compaction.kind === "present" && current.active_compaction.install_state === "staged",
    );
    await queryPostgres(`
      UPDATE storyos.model_attempts
         SET payload = jsonb_set(payload, '{produced_binding,covered_copy_restricted}', 'true')
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${restricted.effect.run_id}'::uuid
         AND attempt_role = 'decision';
    `);
    unlinkSync(hold);
    await restrictedWorker;
    const restrictedInspect = await inspect(started.baseUrl, prepared, restricted.effect.run_id);
    assert.equal(compaction(restrictedInspect).install_state, "refused");
    assert.equal(compaction(restrictedInspect).refusal_reason, "restricted_source");
    assert.equal(compaction(restrictedInspect).installed.kind, "absent");

    writeFileSync(hold, "hold");
    const exact = await admit(started.baseUrl, prepared, id("d215"), COMPACT);
    const exactWorker = settleHeld({ STORYOS_TEST_FAKE_COMPACTION_STAGE_HOLD_PATH: hold });
    await waitFor(
      () => inspect(started.baseUrl, prepared, exact.effect.run_id),
      (current) => current.active_compaction.kind === "present" && current.active_compaction.install_state === "staged",
    );
    await rewriteChapter(prepared.projectId, prepared.chapterId, "repeat('a', 10001)");
    unlinkSync(hold);
    await exactWorker;
    const exactInspect = await inspect(started.baseUrl, prepared, exact.effect.run_id);
    assert.equal(exactInspect.status, "completed");
    assert.equal(compaction(exactInspect).install_state, "refused");
    assert.equal(compaction(exactInspect).refusal_reason, "exact_required_unsatisfied");
    assert.equal(compaction(exactInspect).installed.kind, "absent");
    assert.equal(decisionAttempt(exactInspect), compaction(exactInspect).prior_model_attempt_id);

    await rewriteChapter(prepared.projectId, prepared.chapterId, "''");
    writeFileSync(hold, "hold");
    const stale = await admit(started.baseUrl, prepared, id("d216"), COMPACT);
    const staleWorker = settleHeld({ STORYOS_TEST_FAKE_COMPACTION_STAGE_HOLD_PATH: hold });
    await waitFor(
      () => inspect(started.baseUrl, prepared, stale.effect.run_id),
      (current) => current.status === "claimed" && current.active_compaction.kind === "present" && current.active_compaction.install_state === "staged",
    );
    await queryPostgres(`
      UPDATE storyos.agent_runs
         SET fence_token = fence_token + 1,
             lease_expires_at = clock_timestamp() - interval '1 second'
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${stale.effect.run_id}'::uuid;
    `);
    unlinkSync(hold);
    await staleWorker;
    const stillStaged = await inspect(started.baseUrl, prepared, stale.effect.run_id);
    assert.equal(stillStaged.status, "claimed");
    assert.equal(compaction(stillStaged).install_state, "staged");
    await settleOnce();
    const reclaimed = await inspect(started.baseUrl, prepared, stale.effect.run_id);
    assert.equal(reclaimed.status, "completed");
    assert.equal(compaction(reclaimed).install_state, "installed");
    assert.equal(decisionAttempt(reclaimed), compaction(reclaimed).prior_model_attempt_id);
    assert.equal(selected(reclaimed, "author_instruction"), COMPACT);
  } finally {
    if (existsSync(hold)) unlinkSync(hold);
    await stopRealServer(started.server);
  }
});
