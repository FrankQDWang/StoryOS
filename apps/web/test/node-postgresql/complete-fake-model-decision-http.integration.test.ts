import assert from "node:assert/strict";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createAgentRun,
  createChapter,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  digestCreateAgentRun,
  digestCreateChapter,
  digestCreateVolume,
  digestUpdateProjectAssistance,
  getAgentRun,
  updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateAgentRunRequest,
  GetAgentRunResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  runStoryOSWorker,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeRetry,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const bin = (name: string) => join(repositoryRoot, "target", "release-package", process.platform === "win32" ? `${name}.exe` : name);
const USER_A = "018f0000-0000-7001-8000-000000000001";
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};
const ADVISORY = "This passage is inspectable Host-fake advice. It is not Authoritative State.";
const PROSE = "Guard the narrator voice in this passage.";
const CLARIFICATION = "Which wording should stay in this sentence?";
const REFUSALS = [
  ["Please invoke a tool for this passage.", "tool"],
  ["Please open an MCP connector.", "mcp"],
  ["Please run a research fetch.", "research"],
  ["Please embed this passage.", "embedding"],
  ["Please extract conversation memory.", "memory"],
  ["Please load a skill for this.", "skill"],
  ["Please start a subrun now.", "subrun"],
  ["Please run an eval on this.", "eval"],
] as const;

function id(suffix: string): string {
  return `018f0000-0000-7001-8000-${suffix.padStart(12, "0")}`;
}

function runRequest(
  conversation: CreateAgentRunRequest["create_agent_run_input"]["conversation"],
  chapterId: string,
  correlationId: string,
  text: string,
): CreateAgentRunRequest {
  return {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation,
      author_message: { text },
      working_target: { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

async function startRealServer() {
  return startStoryOSServer({
    repositoryRoot,
    serverBinary: bin("storyos-server"),
    sessions: { "session-a": USER_A, "session-b": "018f0000-0000-7001-8000-000000000101" },
  });
}

async function settleOnce(extraEnv?: Readonly<Record<string, string>>) {
  await runStoryOSWorker({
    repositoryRoot,
    workerBinary: bin("storyos-worker"),
    args: ["--once"],
    ...(extraEnv === undefined ? {} : { extraEnv }),
  });
}

async function drainLeftoverWork() {
  for (let attempt = 0; attempt < 16; attempt += 1) {
    const leftover = await queryPostgres(`
      SELECT count(*)::text
        FROM storyos.agent_runs
       WHERE status IN ('queued', 'claimed');
    `);
    if (leftover === "0") return;
    await settleOnce();
  }
  throw new Error("leftover AgentRun work did not drain");
}

async function challenged<T>(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  method: string,
  route: string,
  schema: string,
  digest: Awaited<ReturnType<typeof digestCreateAgentRun>>,
  key: string,
  send: (antiForgery: string) => Promise<T>,
): Promise<T> {
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: { method, route_template: route, command_schema: schema, canonical_command_digest: digest, idempotency_key: key },
  }));
  return send(challenge.nonce);
}

async function prepare(baseUrl: string, createKey: string, title: string, ns: string) {
  const fetchImpl = browserFetch(baseUrl, "session-a");
  const createRequest = {
    command_schema: "storyos.command.create-project.request.v1" as const,
    create_project_input: { title, ...BINDING, correlation_id: id(`${ns}0`) },
    idempotency_key: createKey,
  };
  const created = await createProjectChallenge({ baseUrl, request: createRequest, fetchImpl });
  await createProject({
    baseUrl,
    fetchImpl,
    idempotencyKey: createKey,
    antiForgery: created.nonce,
    request: {
      command_schema: createRequest.command_schema,
      prospective_project_id: created.prospective_project_id,
      create_project_input: createRequest.create_project_input,
    },
  });
  const projectId = created.prospective_project_id;
  const assistance = {
    command_schema: "storyos.command.update-project-assistance.request.v1" as const,
    update_project_assistance_input: {
      availability: "available" as const,
      expected_assistance_revision: "0",
      ...BINDING,
      correlation_id: id(`${ns}1`),
    },
  };
  await challenged(baseUrl, fetchImpl, projectId, "PUT", "/api/v1/projects/{project_id}/assistance", assistance.command_schema, await digestUpdateProjectAssistance(assistance), id(`${ns}2`), (antiForgery) => updateProjectAssistance({ baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}2`), antiForgery, request: assistance }));
  const volume = {
    command_schema: "storyos.command.create-volume.request.v1" as const,
    create_volume_input: { title: "Volume A", expected_tree_revision: "1", ...BINDING, correlation_id: id(`${ns}3`) },
  };
  const createdVolume = await challenged(baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/volumes", volume.command_schema, await digestCreateVolume(volume), id(`${ns}4`), (antiForgery) => createVolume({ baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}4`), antiForgery, request: volume }));
  if (createdVolume.effect.kind !== "authoritative_applied") throw new Error("Create Volume must apply");
  const volumeId = createdVolume.effect.volume_id;
  const chapter = {
    command_schema: "storyos.command.create-chapter.request.v1" as const,
    create_chapter_input: { title: "Chapter A", expected_tree_revision: "2", ...BINDING, correlation_id: id(`${ns}5`) },
  };
  const createdChapter = await challenged(baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters", chapter.command_schema, await digestCreateChapter(chapter), id(`${ns}6`), (antiForgery) => createChapter({ baseUrl, projectId, volumeId, fetchImpl, idempotencyKey: id(`${ns}6`), antiForgery, request: chapter }));
  if (createdChapter.effect.kind !== "authoritative_applied") throw new Error("Create Chapter must apply");
  return { fetchImpl, projectId, chapterId: createdChapter.effect.chapter_id };
}

async function postRun(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  key: string,
  request: CreateAgentRunRequest,
) {
  return challenged(baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/agent-runs", request.command_schema, await digestCreateAgentRun(request), key, (antiForgery) => createAgentRun({ baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery, request }));
}

async function inspectRun(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  runId: string,
  modelAttemptId?: string,
): Promise<GetAgentRunResponse> {
  return getAgentRun({ baseUrl, projectId, runId, fetchImpl, ...(modelAttemptId === undefined ? {} : { modelAttemptId }) });
}

async function admit(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  key: string,
  text: string,
  conversation: CreateAgentRunRequest["create_agent_run_input"]["conversation"] = { kind: "new" },
) {
  const created = await postRun(baseUrl, fetchImpl, projectId, key, runRequest(conversation, chapterId, id(key.slice(-4)), text));
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  return created;
}

function assertEvidence(queried: GetAgentRunResponse, sentContent: string) {
  if (queried.model_attempt.kind !== "present") throw new Error("expected attempt");
  const attemptId = queried.model_attempt.model_attempt_id;
  assert.deepEqual(queried.evidence, [
    { kind: "sent_content", attempt_id: attemptId, availability: "current", content: sentContent },
    { kind: "stored_reference", attempt_id: attemptId, availability: "current", reference_id: queried.context.assembly_manifest_id },
    { kind: "provider_report", attempt_id: attemptId, availability: "current", report: "host_fake_no_provider_usage" },
    { kind: "provider_opaque", attempt_id: attemptId, availability: "unknown", unknown_facts: ["provider_internal_content"] },
  ]);
}

async function waitFor(
  probe: () => Promise<GetAgentRunResponse>,
  match: (value: GetAgentRunResponse) => boolean,
): Promise<GetAgentRunResponse> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    const current = await probe();
    if (match(current)) return current;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error("hold inspect timed out");
}

test("Worker completes one Host-fake advisory Decision", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e111"), "Fake Decision Novel", "e2");
    const created = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("e131"), "Help with this passage.");
    await settleOnce();
    const queried = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, created.effect.run_id);
    assert.equal(queried.status, "completed");
    assert.equal(queried.context.destination_io.kind, "host_fake");
    assert.equal(queried.context.destination_context_manifest.kind, "present");
    assert.equal(queried.context.outbound_disclosure_manifest.kind, "present");
    assert.equal(queried.context.host_control.distinct_from_destination, true);
    assert.equal(queried.context.host_control.destination_visible, true);
    assert.equal(queried.decision.kind, "advisory");
    if (queried.decision.kind !== "advisory") throw new Error("expected advisory");
    assert.equal(queried.decision.selected, true);
    assert.equal(queried.decision.text, ADVISORY);
    assert.equal(queried.decision.continuation.kind, "present");
    assert.match(queried.decision.decision_id, UUID_V7);
    assert.equal(queried.model_attempt.kind, "present");
    if (queried.model_attempt.kind !== "present") throw new Error("expected attempt");
    assert.match(queried.model_attempt.model_attempt_id, UUID_V7);
    assert.equal(queried.model_attempt.dispatch_state, "settled");
    assertEvidence(queried, "Help with this passage.");
    assert.equal(queried.redaction_profile, "storyos.author.v1");
    assert.equal(queried.items[0]?.state, "complete");
    assert.equal(queried.items[0]?.phase, "complete");
    assert.equal(queried.usage.kind, "unknown");
    const matched = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, created.effect.run_id, queried.model_attempt.model_attempt_id);
    assert.equal(matched.status, "completed");
    const second = await admit(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      prepared.chapterId,
      id("e133"),
      "Help with this passage.",
      { kind: "existing", conversation_id: created.conversation_id },
    );
    await settleOnce();
    const secondInspect = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, second.effect.run_id);
    if (secondInspect.model_attempt.kind !== "present") throw new Error("expected second attempt");
    const foreignAttemptId = secondInspect.model_attempt.model_attempt_id;
    await assert.rejects(
      () => inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, created.effect.run_id, foreignAttemptId),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );
    await assert.rejects(
      () => inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, created.effect.run_id, id("dead")),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );
    await assert.rejects(
      () => inspectRun(started.baseUrl, browserFetch(started.baseUrl, "session-b"), prepared.projectId, created.effect.run_id),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    assert.equal(second.effect.kind, "admitted");
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.model_attempts WHERE project_id = '${prepared.projectId}'::uuid AND run_id = '${created.effect.run_id}'::uuid;`), "1");
  } finally {
    await stopRealServer(started.server);
  }
});

test("Worker refuses Tool, MCP, research, embedding, Memory, Skill, Subrun, and Eval without dispatch", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e141"), "Refuse Novel", "e5");
    let conversationId: string | undefined;
    for (const [index, [text, capability]] of REFUSALS.entries()) {
      const created = await admit(
        started.baseUrl,
        prepared.fetchImpl,
        prepared.projectId,
        prepared.chapterId,
        id(`e5${index}`),
        text,
        conversationId === undefined ? { kind: "new" } : { kind: "existing", conversation_id: conversationId },
      );
      conversationId = created.conversation_id;
      await settleOnce();
      const queried = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, created.effect.run_id);
      assert.equal(queried.status, "refused");
      assert.deepEqual(queried.decision, { kind: "execution_refused", capability });
      assert.equal(queried.model_attempt.kind, "absent");
      assert.deepEqual(queried.evidence, []);
      assert.equal(queried.context.destination_io.kind, "none");
    }
    const blocked = await prepare(started.baseUrl, id("e161"), "Blocked Novel", "e7");
    await queryPostgres(`
      UPDATE storyos.authoritative_payloads AS payload
         SET canonical_bytes = convert_to(repeat('a', 10001), 'UTF8')
        FROM storyos.authoritative_heads AS head
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id, revision.revision_id) =
             (head.owner_user_id, head.project_id, head.manuscript_object_id, head.current_revision_id)
       WHERE (payload.owner_user_id, payload.project_id, payload.payload_id) =
             (revision.owner_user_id, revision.project_id, revision.payload_id)
         AND head.project_id = '${blocked.projectId}'::uuid
         AND head.manuscript_object_id = '${blocked.chapterId}'::uuid;
    `);
    const createdBlocked = await admit(started.baseUrl, blocked.fetchImpl, blocked.projectId, blocked.chapterId, id("e181"), "Help with this passage.");
    await settleOnce();
    const refused = await inspectRun(started.baseUrl, blocked.fetchImpl, blocked.projectId, createdBlocked.effect.run_id);
    assert.equal(refused.status, "refused");
    assert.deepEqual(refused.decision, { kind: "execution_refused", capability: "blocked_context" });
    assert.equal(refused.model_attempt.kind, "absent");
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.model_attempts WHERE project_id IN ('${prepared.projectId}'::uuid, '${blocked.projectId}'::uuid);`), "0");
  } finally {
    await stopRealServer(started.server);
  }
});

test("Worker distinguishes prose-change, clarification, and CFP holds", async () => {
  const started = await startRealServer();
  const dispatchHold = join(tmpdir(), `storyos-fake-dispatch-${process.pid}`);
  const streamHold = join(tmpdir(), `storyos-fake-stream-${process.pid}`);
  const decisionHold = join(tmpdir(), `storyos-fake-decision-${process.pid}`);
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e191"), "Hold Novel", "ea");
    const prose = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("eb11"), "Revise this passage: keep the voice.");
    await settleOnce();
    const proseInspect = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, prose.effect.run_id);
    assert.equal(proseInspect.status, "completed");
    assert.equal(proseInspect.decision.kind, "prose_change");
    if (proseInspect.decision.kind !== "prose_change") throw new Error("expected prose");
    assert.equal(proseInspect.decision.selected, true);
    assert.equal(proseInspect.decision.text, PROSE);
    assert.equal(proseInspect.decision.producer_input, PROSE);
    assert.equal(proseInspect.decision.authoritative, false);
    assert.equal(proseInspect.decision.continuation.kind, "present");

    const clarification = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("eb13"), "Which wording should I keep? The first clause.");
    await settleOnce();
    const clarified = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, clarification.effect.run_id);
    assert.equal(clarified.status, "completed");
    assert.equal(clarified.decision.kind, "clarification");
    if (clarified.decision.kind !== "clarification") throw new Error("expected clarification");
    assert.equal(clarified.decision.selected, true);
    assert.equal(clarified.decision.question, CLARIFICATION);
    assert.equal(clarified.decision.required_reply, CLARIFICATION);
    assert.equal(clarified.decision.continuation.kind, "absent");

    writeFileSync(dispatchHold, "hold");
    const dispatchRun = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("eb15"), "Help with this passage.");
    const dispatchWorker = settleOnce({ STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: dispatchHold });
    const beforeDecision = await waitFor(
      () => inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, dispatchRun.effect.run_id),
      (current) => current.status === "claimed" && current.model_attempt.kind === "present",
    );
    assert.equal(beforeDecision.decision.kind, "absent");
    assert.equal(beforeDecision.context.destination_io.kind, "host_fake");
    assertEvidence(beforeDecision, "Help with this passage.");
    unlinkSync(dispatchHold);
    await dispatchWorker;
    writeFileSync(streamHold, "hold");
    const streamRun = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("eb16"), "Help with this passage.");
    const streamWorker = settleOnce({ STORYOS_TEST_FAKE_STREAM_HOLD_PATH: streamHold });
    const beforeDecisionCut = await waitFor(
      () => inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, streamRun.effect.run_id),
      (current) => current.items.length > 0 && current.decision.kind === "absent",
    );
    assert.equal(beforeDecisionCut.status, "claimed");
    unlinkSync(streamHold);
    await streamWorker;
    assert.equal((await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, streamRun.effect.run_id)).status, "completed");
    const unselected = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("eb18"), "SCRIPT:unselected");
    await settleOnce();
    const unselectedInspect = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, unselected.effect.run_id);
    assert.equal(unselectedInspect.status, "completed");
    assert.equal(unselectedInspect.decision.kind, "absent");
    assert.equal(unselectedInspect.items[0]?.state, "complete");
    assert.equal(unselectedInspect.items[0]?.phase, "complete");
    const scriptCases = [
      ["SCRIPT:partial", "provisional", "assistant"],
      ["SCRIPT:incomplete", "incomplete", "assistant"],
      ["SCRIPT:failed", "failed", "assistant"],
      ["SCRIPT:cancelled", "cancelled", "assistant"],
      ["SCRIPT:unknown", "unknown", "assistant"],
      ["SCRIPT:invalid", "complete", "assistant"],
      ["SCRIPT:tool_partial", "provisional", "tool"],
      ["SCRIPT:hosted", "complete", "hosted"],
      ["SCRIPT:refusal", "complete", "assistant"],
    ] as const;
    let scriptIndex = 0;
    for (const [text, state, role] of scriptCases) {
      const scripted = await admit(
        started.baseUrl,
        prepared.fetchImpl,
        prepared.projectId,
        prepared.chapterId,
        id(`ec1${scriptIndex}`),
        text,
      );
      scriptIndex += 1;
      await settleOnce();
      const inspect = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, scripted.effect.run_id);
      assert.equal(inspect.status, "completed");
      assert.equal(inspect.decision.kind, "absent");
      assert.equal(inspect.items[0]?.state, state);
      assert.equal(inspect.items[0]?.phase, state);
      assert.equal(inspect.items[0]?.role, role);
      if (text === "SCRIPT:refusal") {
        assert.equal(inspect.items[0]?.refusal, "host_fake_refusal");
      }
    }

    writeFileSync(decisionHold, "hold");
    const decisionRun = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("eb17"), "Help with this passage.");
    const decisionWorker = settleOnce({ STORYOS_TEST_FAKE_DECISION_HOLD_PATH: decisionHold });
    const beforeContinuation = await waitFor(
      () => inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, decisionRun.effect.run_id),
      (current) => current.decision.kind === "advisory" && current.decision.continuation.kind === "absent",
    );
    assert.equal(beforeContinuation.status, "claimed");
    unlinkSync(decisionHold);
    await decisionWorker;
    const afterDecision = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, decisionRun.effect.run_id);
    assert.equal(afterDecision.status, "completed");
    assert.equal(afterDecision.decision.kind, "advisory");
    if (afterDecision.decision.kind !== "advisory") throw new Error("expected advisory");
    assert.equal(afterDecision.decision.continuation.kind, "present");
  } finally {
    if (existsSync(dispatchHold)) unlinkSync(dispatchHold);
    if (existsSync(streamHold)) unlinkSync(streamHold);
    if (existsSync(decisionHold)) unlinkSync(decisionHold);
    await stopRealServer(started.server);
  }
});
