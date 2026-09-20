import assert from "node:assert/strict";
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

const FIRST = "Help with this passage.";
const CORRECTION = "I changed my mind: keep the voice.";
const FULL = "Submit the complete current request. Keep the voice.";
const EDITED = "Edited passage text.";

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
  await settleOnce();
  return created;
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

test("continuation consumes an eligible prior binding and keeps current input inspectable", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("c211"), "Continue Novel", "c2");
    const first = await admit(started.baseUrl, prepared, id("c221"), FIRST);
    const firstInspect = await inspect(started.baseUrl, prepared, first.effect.run_id);
    const firstBinding = produced(firstInspect);
    assert.equal(firstInspect.status, "completed");
    assert.equal(attempt(firstInspect).input_mapping, "none");
    assert.equal(attempt(firstInspect).prior_continuation.kind, "absent");
    assert.equal(selected(firstInspect, "author_instruction"), FIRST);
    assert.equal(selected(firstInspect, "working_target"), "");
    await rewriteChapter(prepared.projectId, prepared.chapterId, `'${EDITED}'`);
    const second = await admit(started.baseUrl, prepared, id("c223"), CORRECTION, {
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
    const other = await admit(started.baseUrl, prepared, id("c225"), FIRST);
    assert.notEqual(other.conversation_id, first.conversation_id);
    assert.equal(other.effect.project_agent_id, first.effect.project_agent_id);
    assert.equal(attempt(await inspect(started.baseUrl, prepared, other.effect.run_id)).input_mapping, "none");
    const full = await admit(started.baseUrl, prepared, id("c227"), FULL, {
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
    const stale = await admit(started.baseUrl, prepared, id("c229"), CORRECTION, {
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
    const restricted = await admit(started.baseUrl, prepared, id("c22b"), CORRECTION, {
      kind: "existing",
      conversation_id: first.conversation_id,
    });
    assert.equal(attempt(await inspect(started.baseUrl, prepared, restricted.effect.run_id)).input_mapping, "new_transport");
    await rewriteChapter(prepared.projectId, prepared.chapterId, "repeat('a', 10001)");
    const blocked = await admit(started.baseUrl, prepared, id("c22d"), CORRECTION, {
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
