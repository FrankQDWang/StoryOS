// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/recover-or-cancel-agent-run-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import {
  cancelAgentRun,
  digestCancelAgentRun,
  getAgentRun,
  getProposal,
  steerAgentRun,
  digestSteerAgentRun,
  pauseAgentRun,
  digestPauseAgentRun,
  type CancelAgentRunRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";
import { BINDING, USER_A, admitRun, attempt, challenged, drainLeftoverWork, id, inspectRun, prepare, produced,
  rewriteChapter, selected, settleOnce, startServer } from "../support/http-harness.ts";

const FIRST = "Help with this passage.";
const CORRECTION = "I changed my mind: keep the voice.";
const FULL = "Submit the complete current request. Keep the voice.";
const EDITED = "Edited passage text.";

async function admit(...args: Parameters<typeof admitRun>) {
  const created = await admitRun(...args);
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

test.each([FIRST, "Compact active context between calls."])("ordered guidance is consumed by the same active Run with exact replay: %s", async (original) => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const ns = original.startsWith("Compact") ? "d6" : "d4";
    const prepared = await prepare(started.baseUrl, id(`${ns}11`), "Guidance Novel", `${ns}2`);
    const created = await admitRun(started.baseUrl, prepared, id(`${ns}32`), original);
    const runId = created.effect.run_id;
    if (original === FIRST) {
      const request = { command_schema: "storyos.command.pause-agent-run.request.v1" as const,
        pause_agent_run_input: { ...BINDING, correlation_id: id("d460") } };
      await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
        "/api/v1/projects/{project_id}/agent-runs/{run_id}/pause", request.command_schema,
        await digestPauseAgentRun(request), id("d461"), (antiForgery) => pauseAgentRun({
          baseUrl: started.baseUrl, projectId: prepared.projectId, runId, fetchImpl: prepared.fetchImpl,
          request, idempotencyKey: id("d461"), antiForgery }));
      assert.equal((await inspectRun(started.baseUrl, prepared, runId)).status, "paused");
    }
    const corrections = original.startsWith("Compact") ? [CORRECTION] : [CORRECTION, "Keep the ending open."];
    for (const [index, text] of corrections.entries()) {
      await retain(started.baseUrl, prepared, runId, created.conversation_id, text, id(`${ns}5${index}`), String(index + 1));
    }
    await settleOnce();
    const queried = await inspectRun(started.baseUrl, prepared, runId);
    assert.equal(queried.status, "completed");
    assert.equal(queried.conversation_id, created.conversation_id);
    assert.deepEqual(queried.steering_inputs.map((item) => [item.input_position, item.author_message, item.model_attempt_id !== null]),
      corrections.map((text, index) => [String(index + 1), text, true]));
    // Compaction occurs before the first decision request, and a new Conversation has no prior
    // response to compact.
    assert.equal(queried.active_compaction.kind, "absent");
    assert.equal(selected(queried, "author_instruction"), [original, ...corrections].join("\n"));
    assert.deepEqual(queried.evidence.find((item) => item.kind === "sent_content"), {
      kind: "sent_content", attempt_id: attempt(queried).model_attempt_id,
      availability: "current", content: [original, ...corrections].join("\n"),
    });
  } finally {
    await stopRealServer(started.server);
  }
});

test("author messages are limited by characters, not UTF-8 bytes", async () => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("d911"), "Long Message Novel", "d92");
    const within = "字".repeat(3000);
    const over = "字".repeat(8001);
    const conflict = (message: string) => (error: unknown) => {
      const protocol = requireStoryOSProtocolError(error);
      assert.equal(protocol.status, 409);
      assert.deepEqual(JSON.parse(protocol.responseBody ?? "{}"), {
        schema_id: "storyos.problem.v1", code: "idempotency_binding_conflict", message });
      return true;
    };
    await assert.rejects(() => admitRun(started.baseUrl, prepared, id("d931"), over),
      conflict("The createAgentRun binding conflicts."));
    const created = await admitRun(started.baseUrl, prepared, id("d932"), within);
    const runId = created.effect.run_id;
    const request = { command_schema: "storyos.command.steer-agent-run.request.v1" as const,
      steer_agent_run_input: { conversation_id: created.conversation_id, author_message: { text: over },
        ...BINDING, correlation_id: id("d940") } };
    const digest = await digestSteerAgentRun(request);
    await assert.rejects(() => challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/agent-runs/{run_id}/steering-inputs", request.command_schema,
      digest, id("d941"), (antiForgery) => steerAgentRun({ baseUrl: started.baseUrl,
        projectId: prepared.projectId, runId, fetchImpl: prepared.fetchImpl, idempotencyKey: id("d941"),
        antiForgery, request })),
      conflict("The AgentRun control binding conflicts."));
    await retain(started.baseUrl, prepared, runId, created.conversation_id, within, id("d950"), "1");
    await settleOnce();
    assert.equal(selected(await inspectRun(started.baseUrl, prepared, runId), "author_instruction"), `${within}\n${within}`);
  } finally {
    await stopRealServer(started.server);
  }
});

test("guidance after a visible stream keeps the Proposal bound to its original Decision", async () => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("d511"), "Stream Guidance", "d52");
    const created = await admitRun(started.baseUrl, prepared, id("d532"), "Stream this passage: keep the voice.");
    await settleOnce();
    const original = await inspectRun(started.baseUrl, prepared, created.effect.run_id);
    if (original.decision.kind !== "prose_change" || original.decision.opened_proposal.kind !== "present") throw new Error("expected streamed Proposal");
    const proposalId = original.decision.opened_proposal.proposal_id;
    const options = { baseUrl: started.baseUrl, projectId: prepared.projectId, runId: created.effect.run_id, fetchImpl: prepared.fetchImpl };
    await retain(started.baseUrl, prepared, created.effect.run_id, created.conversation_id, CORRECTION, id("d541"), "1");
    await drainLeftoverWork();
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
    await stopRealServer(started.server);
  }
});

test("continuation consumes an eligible prior binding and keeps current input inspectable", async () => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("c411"), "Continue Novel", "c42");
    const first = await admit(started.baseUrl, prepared, id("c431"), FIRST);
    const firstInspect = await inspectRun(started.baseUrl, prepared, first.effect.run_id);
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
    const secondInspect = await inspectRun(started.baseUrl, prepared, second.effect.run_id);
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
    const afterEdit = await inspectRun(started.baseUrl, prepared, first.effect.run_id);
    assert.equal(selected(afterEdit, "author_instruction"), FIRST);
    assert.equal(selected(afterEdit, "working_target"), "");
    const other = await admit(started.baseUrl, prepared, id("c435"), FIRST);
    assert.notEqual(other.conversation_id, first.conversation_id);
    assert.equal(other.effect.project_agent_id, first.effect.project_agent_id);
    assert.equal(attempt(await inspectRun(started.baseUrl, prepared, other.effect.run_id)).input_mapping, "none");
    const full = await admit(started.baseUrl, prepared, id("c437"), FULL, {
      kind: "existing",
      conversation_id: first.conversation_id,
    });
    const fullInspect = await inspectRun(started.baseUrl, prepared, full.effect.run_id);
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
    const staleInspect = await inspectRun(started.baseUrl, prepared, stale.effect.run_id);
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
    assert.equal(attempt(await inspectRun(started.baseUrl, prepared, restricted.effect.run_id)).input_mapping, "new_transport");
    await rewriteChapter(prepared.projectId, prepared.chapterId, "repeat('a', 32001)");
    const blocked = await admit(started.baseUrl, prepared, id("c43d"), CORRECTION, {
      kind: "existing",
      conversation_id: first.conversation_id,
    });
    const blockedInspect = await inspectRun(started.baseUrl, prepared, blocked.effect.run_id);
    assert.equal(blockedInspect.status, "refused");
    assert.deepEqual(blockedInspect.decision, { kind: "execution_refused", capability: "blocked_context" });
    const original = await inspectRun(started.baseUrl, prepared, first.effect.run_id);
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

test("createAgentRun admits a Working Target at the item limit and blocks one above it before dispatch", async () => {
  const started = await startServer();
  try {
    const prepared = await prepare(started.baseUrl, id("1309ac30"), "Limit Novel", "c2");
    const chapterId = prepared.chapterId;
    const setChapterLength = (length: number) =>
      rewriteChapter(prepared.projectId, chapterId, `repeat('a', ${length})`);
    await setChapterLength(32001);
    const created = await admitRun(started.baseUrl, prepared, id("c32"), FIRST);
    if (created.effect.kind !== "admitted") throw new Error("expected admitted");
    const queried = await inspectRun(started.baseUrl, prepared, created.effect.run_id);
    assert.equal(queried.context.sufficiency.kind, "blocked");
    if (queried.context.sufficiency.kind !== "blocked") throw new Error("expected blocked");
    assert.deepEqual(queried.context.sufficiency.reasons, [{
      kind: "exact_required_over_limit",
      source_class: "working_target",
    }]);
    assert.equal(queried.context.destination_io.kind, "none");
    assert.equal(queried.context.destination_context_manifest.kind, "absent");
    assert.equal(queried.context.outbound_disclosure_manifest.kind, "absent");
    assert.deepEqual(
      queried.context.selected.map((item) => item.source_class),
      ["author_instruction"],
    );
    assert.deepEqual(
      queried.context.rejected.map((item) => [item.source_class, item.reason.kind, item.token_count]),
      [["working_target", "over_item_token_limit", "32001"]],
    );
    const assembly = await queryPostgres(`
      SELECT sufficiency || ' ' ||
             (destination_context_manifest_id IS NULL)::text || ' ' ||
             (outbound_disclosure_manifest_id IS NULL)::text || ' ' ||
             (SELECT count(*) FROM storyos.model_attempts AS attempt
               WHERE attempt.run_id = assembly.run_id)::text
        FROM storyos.context_assembly_manifests AS assembly
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${created.effect.run_id}'::uuid;
    `);
    assert.equal(assembly, "blocked true true 0");

    await setChapterLength(32000);
    const atLimit = await admitRun(started.baseUrl, prepared, id("c34"), FIRST);
    if (atLimit.effect.kind !== "admitted") throw new Error("expected admitted");
    const admitted = await inspectRun(started.baseUrl, prepared, atLimit.effect.run_id);
    assert.equal(admitted.context.sufficiency.kind, "complete");
    assert.deepEqual(
      admitted.context.selected.map((item) => [item.source_class, item.token_count]),
      [["author_instruction", "23"], ["working_target", "32000"]],
    );
  } finally {
    await stopRealServer(started.server);
  }
});

test("Run settings stay captured when a legacy current revision is restored or evidence is unavailable", async () => {
  const started = await startServer();
  try {
    const project = await prepare(started.baseUrl, id("1309da00"), "Captured Memory", "d0");
    const created = await admitRun(started.baseUrl, project, id("da11"), FIRST);
    const options = { baseUrl: started.baseUrl, fetchImpl: project.fetchImpl, projectId: project.projectId };
    const original = await getAgentRun({ ...options, runId: created.effect.run_id });
    const cancel: CancelAgentRunRequest = {
      command_schema: "storyos.command.cancel-agent-run.request.v1",
      cancel_agent_run_input: { ...BINDING, correlation_id: id("da13") },
    };
    await challenged(started.baseUrl, project.fetchImpl, project.projectId, "POST",
      "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
      cancel.command_schema, await digestCancelAgentRun(cancel), id("da12"),
      (antiForgery) => cancelAgentRun({ ...options, runId: original.run_id,
        idempotencyKey: id("da12"), antiForgery, request: cancel }));
    assert.equal((await getAgentRun({ ...options, runId: original.run_id })).status, "cancelled");
    await queryPostgres(`
      UPDATE storyos.conversation_memory_settings SET is_current = FALSE
       WHERE project_id = '${project.projectId}'::uuid;
      INSERT INTO storyos.conversation_memory_settings
      SELECT (jsonb_populate_record(NULL::storyos.conversation_memory_settings,
        (to_jsonb(settings) - 'is_current') || jsonb_build_object(
          'memory_settings_revision', '${id("da14")}', 'use_enabled', false,
          'contribution_enabled', true))).*
        FROM storyos.conversation_memory_settings AS settings
       WHERE project_id = '${project.projectId}'::uuid;
    `);
    const next = await admitRun(started.baseUrl, project, id("da15"), FIRST,
      { kind: "existing", conversation_id: original.conversation_id });
    const current = await getAgentRun({ ...options, runId: next.effect.run_id });
    assert.equal(next.memory_settings_revision, id("da14"));
    assert.deepEqual(current.captured_memory_settings, {
      kind: "available", memory_settings_revision: id("da14"),
      use_enabled: false, contribution_enabled: true,
    });
    assert.deepEqual((await getAgentRun({ ...options, runId: original.run_id })).captured_memory_settings,
      original.captured_memory_settings);
    await assert.rejects(() => queryPostgres(`
      INSERT INTO storyos.conversation_memory_settings
      SELECT (jsonb_populate_record(NULL::storyos.conversation_memory_settings,
        to_jsonb(settings) || jsonb_build_object('memory_settings_revision', '${id("da17")}'))).*
        FROM storyos.conversation_memory_settings AS settings
       WHERE project_id = '${project.projectId}'::uuid AND is_current IS NULL;
    `), (error) => String(error).includes("conversation_memory_settings_one_current"));
    await queryPostgres(`
      CREATE POLICY issue_876_withheld_settings ON storyos.conversation_memory_settings
        AS RESTRICTIVE FOR SELECT TO storyos_runtime USING (
          NOT (project_id = '${project.projectId}'::uuid
            AND memory_settings_revision = '${original.memory_settings_revision}'::uuid));
    `);
    assert.deepEqual((await getAgentRun({ ...options, runId: original.run_id })).captured_memory_settings,
      { kind: "unavailable" });
    assert.deepEqual((await getAgentRun({ ...options, runId: current.run_id })).captured_memory_settings,
      current.captured_memory_settings);
    await assert.rejects(() => getAgentRun({ ...options, runId: original.run_id,
      fetchImpl: browserFetch(started.baseUrl, "session-b") }), (error) => {
      const protocol = requireStoryOSProtocolError(error);
      return protocol.status === 404 && !String(protocol.responseBody).includes(original.conversation_id);
    });
  } finally {
    try {
      await queryPostgres("DROP POLICY IF EXISTS issue_876_withheld_settings ON storyos.conversation_memory_settings");
    } finally {
      await stopRealServer(started.server);
    }
  }
});
