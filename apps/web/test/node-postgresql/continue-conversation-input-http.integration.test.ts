// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/recover-or-cancel-agent-run-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { expect, test } from "vitest";
import { writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  createAgentRun,
  digestCreateAgentRun,
  getAgentRun, getProposal, steerAgentRun, digestSteerAgentRun,
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

const FIRST = "Help with this passage.";
const CORRECTION = "I changed my mind: keep the voice.";
const FULL = "Submit the complete current request. Keep the voice.";
const EDITED = "Edited passage text.";

async function admitQueued(
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
async function admit(...args: Parameters<typeof admitQueued>) {
  const created = await admitQueued(...args);
  await settleOnce();
  return created;
}
async function retain(baseUrl: string, prepared: Awaited<ReturnType<typeof prepare>>, runId: string,
  conversationId: string, text: string, key: string, position: string) {
  const request = { command_schema: "storyos.command.steer-agent-run.request.v1" as const,
    steer_agent_run_input: { conversation_id: conversationId, author_message: { text },
      ...BINDING, correlation_id: key } };
  return challenged(baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
    "/api/v1/projects/{project_id}/agent-runs/{run_id}/steering-inputs", request.command_schema,
    await digestSteerAgentRun(request), key, async (antiForgery) => {
      const options = { baseUrl, projectId: prepared.projectId, runId, fetchImpl: prepared.fetchImpl,
        idempotencyKey: key, antiForgery, request };
      const retained = await steerAgentRun(options);
      assert.equal(retained.effect.kind, "retained");
      if (retained.effect.kind !== "retained") throw new Error("expected retained input");
      assert.equal(retained.effect.input_position, position);
      assert.deepEqual(await steerAgentRun(options), retained);
      return retained;
    });
}
async function inspect(baseUrl: string, prepared: Awaited<ReturnType<typeof prepare>>, runId: string): Promise<GetAgentRunResponse> {
  return getAgentRun({ baseUrl, projectId: prepared.projectId, runId, fetchImpl: prepared.fetchImpl });
}
function selected(queried: GetAgentRunResponse, sourceClass: string) {
  return queried.context.selected.find((item) => item.source_class === sourceClass)?.content;
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

test.each([FIRST, "Compact active context between calls."])("ordered guidance is consumed by the same active Run with exact replay: %s", async (original) => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("d411"), "Guidance Novel", "d42");
    const created = await admitQueued(started.baseUrl, prepared, id("d432"), original);
    const runId = created.effect.run_id;
    const corrections = original.startsWith("Compact") ? [CORRECTION] : [CORRECTION, "Keep the ending open."];
    for (const [index, text] of corrections.entries()) {
      await retain(started.baseUrl, prepared, runId, created.conversation_id, text, id(`d45${index}`), String(index + 1));
    }
    await settleOnce();
    const queried = await inspect(started.baseUrl, prepared, runId);
    assert.equal(queried.status, "completed");
    assert.equal(queried.conversation_id, created.conversation_id);
    assert.deepEqual(queried.steering_inputs.map((item) => [item.input_position, item.author_message, item.model_attempt_id !== null]),
      corrections.map((text, index) => [String(index + 1), text, true]));
    if (original.startsWith("Compact")) {
      assert.equal(queried.active_compaction.kind, "present");
      if (queried.active_compaction.kind !== "present") throw new Error("expected active compaction");
      assert.equal(queried.active_compaction.prior_model_attempt_id, attempt(queried).model_attempt_id);
    }
    assert.equal(selected(queried, "author_instruction"), [original, ...corrections].join("\n"));
    assert.deepEqual(queried.evidence.find((item) => item.kind === "sent_content"), {
      kind: "sent_content", attempt_id: attempt(queried).model_attempt_id,
      availability: "current", content: [original, ...corrections].join("\n"),
    });
  } finally {
    await stopRealServer(started.server);
  }
});

test("guidance after a visible stream keeps the Proposal bound to its original Decision", async () => {
  const started = await startRealServer();
  const hold = join(tmpdir(), `storyos-guidance-stream-${process.pid}`);
  let worker: Promise<void> | undefined;
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("d511"), "Stream Guidance", "d52");
    const created = await admitQueued(started.baseUrl, prepared, id("d532"), "Stream this passage: keep the voice.");
    writeFileSync(hold, "hold");
    worker = settleOnce({ STORYOS_TEST_FAKE_STREAM_HOLD_PATH: hold });
    void worker.catch(() => undefined);
    await expect.poll(async () => (await inspect(started.baseUrl, prepared, created.effect.run_id)).decision.kind).toBe("prose_change");
    const original = await inspect(started.baseUrl, prepared, created.effect.run_id);
    if (original.decision.kind !== "prose_change" || original.decision.opened_proposal.kind !== "present") throw new Error("expected streamed Proposal");
    const proposalId = original.decision.opened_proposal.proposal_id;
    const options = { baseUrl: started.baseUrl, projectId: prepared.projectId, runId: created.effect.run_id, fetchImpl: prepared.fetchImpl };
    await retain(started.baseUrl, prepared, created.effect.run_id, created.conversation_id, CORRECTION, id("d541"), "1");
    rmSync(hold); await worker;
    const current = await getAgentRun(options);
    const historical = await getAgentRun({ ...options, modelAttemptId: attempt(original).model_attempt_id });
    assert.equal(current.status, "completed");
    assert.equal(current.decision.kind, "prose_change");
    if (current.decision.kind !== "prose_change") throw new Error("expected new prose Decision");
    assert.notEqual(current.decision.decision_id, original.decision.decision_id);
    assert.deepEqual(current.decision.opened_proposal, { kind: "absent" });
    assert.equal(attempt(historical).model_attempt_id, attempt(original).model_attempt_id);
    assert.equal(selected(historical, "author_instruction"), "Stream this passage: keep the voice.");
    const proposal = await getProposal({ ...options, proposalId });
    assert.deepEqual(proposal.proposal.source, { kind: "agent_run_decision", run_id: created.effect.run_id,
      decision_id: original.decision.decision_id });
  } finally {
    rmSync(hold, { force: true });
    await worker?.catch(() => undefined);
    await stopRealServer(started.server);
  }
});

test("continuation consumes an eligible prior binding and keeps current input inspectable", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("c411"), "Continue Novel", "c42");
    const first = await admit(started.baseUrl, prepared, id("c431"), FIRST);
    const firstInspect = await inspect(started.baseUrl, prepared, first.effect.run_id);
    const firstBinding = produced(firstInspect);
    assert.equal(firstInspect.status, "completed");
    assert.equal(attempt(firstInspect).input_mapping, "none");
    assert.equal(attempt(firstInspect).prior_continuation.kind, "absent");
    assert.equal(selected(firstInspect, "author_instruction"), FIRST);
    assert.equal(selected(firstInspect, "working_target"), "");
    await rewriteChapter(prepared.projectId, prepared.chapterId, `'${EDITED}'`);
    const second = await admit(started.baseUrl, prepared, id("c433"), CORRECTION, {
      kind: "existing",
      conversation_id: first.conversation_id,
    });
    const secondInspect = await inspect(started.baseUrl, prepared, second.effect.run_id);
    assert.equal(second.conversation_id, first.conversation_id);
    assert.equal(second.effect.project_agent_id, first.effect.project_agent_id);
    assert.equal(attempt(secondInspect).input_mapping, "incremental");
    assert.deepEqual(attempt(secondInspect).prior_continuation, {
      kind: "present",
      continuation_binding_id: firstBinding,
    });
    assert.equal(attempt(secondInspect).admission.adapter_mapping, "storyos.host-fake.mapping.v1");
    assert.equal(selected(secondInspect, "author_instruction"), CORRECTION);
    assert.equal(selected(secondInspect, "working_target"), EDITED);
    assert.equal(secondInspect.evidence.some((item) => item.kind === "stored_reference" && item.reference_id === firstBinding), true);
    const afterEdit = await inspect(started.baseUrl, prepared, first.effect.run_id);
    assert.equal(selected(afterEdit, "author_instruction"), FIRST);
    assert.equal(selected(afterEdit, "working_target"), "");
    const other = await admit(started.baseUrl, prepared, id("c435"), FIRST);
    assert.notEqual(other.conversation_id, first.conversation_id);
    assert.equal(other.effect.project_agent_id, first.effect.project_agent_id);
    assert.equal(attempt(await inspect(started.baseUrl, prepared, other.effect.run_id)).input_mapping, "none");
    const full = await admit(started.baseUrl, prepared, id("c437"), FULL, {
      kind: "existing",
      conversation_id: first.conversation_id,
    });
    const fullInspect = await inspect(started.baseUrl, prepared, full.effect.run_id);
    assert.equal(attempt(fullInspect).input_mapping, "full");
    assert.equal(attempt(fullInspect).prior_continuation.kind, "absent");
    assert.deepEqual(attempt(fullInspect).known_prior_continuation, {
      kind: "present",
      continuation_binding_id: produced(secondInspect),
    });
    await queryPostgres(`
      UPDATE storyos.model_attempts
         SET payload = jsonb_set(payload, '{produced_binding,processing_destination_identity}', '"018f0000-0000-7001-8000-00000000dead"')
       WHERE project_id = '${prepared.projectId}'::uuid
         AND continuation_binding_id = '${produced(fullInspect)}'::uuid;
    `);
    const stale = await admit(started.baseUrl, prepared, id("c439"), CORRECTION, {
      kind: "existing",
      conversation_id: first.conversation_id,
    });
    const staleInspect = await inspect(started.baseUrl, prepared, stale.effect.run_id);
    assert.equal(attempt(staleInspect).input_mapping, "new_transport");
    assert.equal(attempt(staleInspect).prior_continuation.kind, "absent");
    await queryPostgres(`
      UPDATE storyos.model_attempts
         SET payload = jsonb_set(payload, '{produced_binding,covered_copy_restricted}', 'true')
       WHERE project_id = '${prepared.projectId}'::uuid
         AND continuation_binding_id = '${produced(staleInspect)}'::uuid;
    `);
    const restricted = await admit(started.baseUrl, prepared, id("c43b"), CORRECTION, {
      kind: "existing",
      conversation_id: first.conversation_id,
    });
    assert.equal(attempt(await inspect(started.baseUrl, prepared, restricted.effect.run_id)).input_mapping, "new_transport");
    await rewriteChapter(prepared.projectId, prepared.chapterId, "repeat('a', 10001)");
    const blocked = await admit(started.baseUrl, prepared, id("c43d"), CORRECTION, {
      kind: "existing",
      conversation_id: first.conversation_id,
    });
    const blockedInspect = await inspect(started.baseUrl, prepared, blocked.effect.run_id);
    assert.equal(blockedInspect.status, "refused");
    assert.deepEqual(blockedInspect.decision, { kind: "execution_refused", capability: "blocked_context" });
    const original = await inspect(started.baseUrl, prepared, first.effect.run_id);
    assert.equal(original.status, "completed");
    assert.equal(selected(original, "author_instruction"), FIRST);
    assert.equal(produced(original), firstBinding);
    await assert.rejects(
      () => getAgentRun({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        runId: first.effect.run_id,
        fetchImpl: browserFetch(started.baseUrl, "session-b"),
      }),
      (error) => requireStoryOSProtocolError(error).status === 404
        && !String(requireStoryOSProtocolError(error).responseBody).includes(USER_A),
    );
  } finally {
    await stopRealServer(started.server);
  }
});
