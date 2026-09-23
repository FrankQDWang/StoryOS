// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/complete-fake-model-decision-http.integration.test.ts"]}
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
  createChapter,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  digestCancelAgentRun,
  digestCreateAgentRun,
  digestCreateChapter,
  digestCreateVolume,
  digestPauseAgentRun,
  digestUpdateProjectAssistance,
  activityStream,
  getAgentRun,
  getManuscriptTree,
  getProposal,
  pauseAgentRun,
  updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CancelAgentRunRequest,
  CreateAgentRunRequest,
  GetAgentRunResponse,
  PauseAgentRunRequest,
  UpdateProjectAssistanceRequest,
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
const execFileAsync = promisify(execFile);
const USER_A = "018f0000-0000-7001-8000-000000000001";
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};

function id(suffix: string): string {
  return `018f0000-0000-7001-8000-${suffix.padStart(12, "0")}`;
}

function runRequest(
  conversation: CreateAgentRunRequest["create_agent_run_input"]["conversation"],
  chapterId: string,
  correlationId: string,
  text = "Help with this passage.",
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

function pauseRequest(correlationId: string): PauseAgentRunRequest {
  return {
    command_schema: "storyos.command.pause-agent-run.request.v1",
    pause_agent_run_input: { ...BINDING, correlation_id: correlationId },
  };
}

function cancelRequest(correlationId: string): CancelAgentRunRequest {
  return {
    command_schema: "storyos.command.cancel-agent-run.request.v1",
    cancel_agent_run_input: { ...BINDING, correlation_id: correlationId },
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

function settleHeld(extraEnv: Readonly<Record<string, string>>): Promise<void> {
  const env = { ...process.env, ...extraEnv };
  if (process.env.STORYOS_TEST_DATABASE_URL !== undefined) {
    env.STORYOS_DATABASE_URL = process.env.STORYOS_TEST_DATABASE_URL;
  }
  const held = execFileAsync(bin("storyos-worker"), ["--once"], {
    cwd: repositoryRoot,
    env,
    timeout: 60_000,
    killSignal: "SIGKILL",
  }).then(() => undefined);
  void held.catch(() => undefined);
  return held;
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

async function admit(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  key: string,
  text = "Help with this passage.",
  conversation: CreateAgentRunRequest["create_agent_run_input"]["conversation"] = { kind: "new" },
) {
  const created = await postRun(baseUrl, fetchImpl, projectId, key, runRequest(conversation, chapterId, id(key.slice(-4)), text));
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  return created;
}

async function postPause(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  runId: string,
  key: string,
  correlationId: string,
) {
  const request = pauseRequest(correlationId);
  const digest = await digestPauseAgentRun(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/agent-runs/{run_id}/pause",
      command_schema: request.command_schema,
      canonical_command_digest: digest,
      idempotency_key: key,
    },
  }));
  const response = await pauseAgentRun({
    baseUrl,
    projectId,
    runId,
    fetchImpl,
    idempotencyKey: key,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, response };
}

async function cancelChallenge(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  key: string,
  correlationId: string,
) {
  const request = cancelRequest(correlationId);
  const digest = await digestCancelAgentRun(request);
  return withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
      command_schema: request.command_schema,
      canonical_command_digest: digest,
      idempotency_key: key,
    },
  }));
}

async function postCancel(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  runId: string,
  key: string,
  correlationId: string,
  preissuedChallenge?: Awaited<ReturnType<typeof cancelChallenge>>,
) {
  const request = cancelRequest(correlationId);
  const challenge = preissuedChallenge ?? await cancelChallenge(baseUrl, fetchImpl, projectId, key, correlationId);
  const response = await cancelAgentRun({
    baseUrl,
    projectId,
    runId,
    fetchImpl,
    idempotencyKey: key,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, response };
}

async function inspectRun(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  runId: string,
): Promise<GetAgentRunResponse> {
  return getAgentRun({ baseUrl, projectId, runId, fetchImpl });
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
  throw new Error(`hold inspect timed out: status=${last?.status ?? "none"} decision=${last?.decision.kind ?? "none"}`);
}

function activityEvent(body: string, eventKind: string) {
  return body.split("\n\n").map((block) => {
    const data = block.split("\n").find((line) => line.startsWith("data:"));
    return data === undefined ? undefined : JSON.parse(data.slice(5).trim()) as {
      event_kind?: string;
      event_schema?: string;
    };
  }).find((event) => event?.event_kind === eventKind);
}

function problemCode(error: unknown): string | undefined {
  const protocol = requireStoryOSProtocolError(error);
  if (protocol.responseBody === undefined) return undefined;
  try {
    return (JSON.parse(protocol.responseBody) as { code?: string }).code;
  } catch {
    return undefined;
  }
}

test("pauseAgentRun and cancelAgentRun stay distinct and keep a terminal Run immutable", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("c111"), "Recover Cancel Novel", "c2");
    const created = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("c131"));
    const paused = await postPause(started.baseUrl, prepared.fetchImpl, prepared.projectId, created.effect.run_id, id("c141"), id("c142"));
    assert.equal(paused.response.effect.kind, "applied");
    if (paused.response.effect.kind !== "applied") throw new Error("expected pause applied");
    assert.equal(paused.response.effect.status, "paused");
    assert.equal(paused.response.effect.run_id, created.effect.run_id);
    assert.match(paused.response.effect.fence_generation, /^[1-9][0-9]*$/);
    assert.match(paused.response.receipt.receipt_id, UUID_V7);
    const pauseReplay = await pauseAgentRun({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      runId: created.effect.run_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("c141"),
      antiForgery: paused.challenge.nonce,
      request: pauseRequest(id("c142")),
    });
    assert.equal(pauseReplay.command_id, paused.response.command_id);
    assert.equal(pauseReplay.receipt.receipt_id, paused.response.receipt.receipt_id);
    const pausedInspect = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, created.effect.run_id);
    assert.equal(pausedInspect.status, "paused");
    assert.equal(pausedInspect.decision.kind, "absent");
    assert.equal(pausedInspect.usage.kind, "unknown");
    const tree = await getManuscriptTree({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl,
    });
    const pausedActivity = activityEvent(await activityStream({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      snapshotId: tree.snapshot.snapshot_id,
      protocolRelease: "storyos.public.release.1",
      fetchImpl: prepared.fetchImpl,
    }), "agent_run_paused");
    assert.equal(pausedActivity?.event_schema, "storyos.event.agent-run-paused.v1");
    const alreadyPaused = await postPause(started.baseUrl, prepared.fetchImpl, prepared.projectId, created.effect.run_id, id("c143"), id("c144"));
    assert.equal(alreadyPaused.response.effect.kind, "no_effect");
    if (alreadyPaused.response.effect.kind !== "no_effect") throw new Error("expected already paused");
    assert.equal(alreadyPaused.response.effect.reason, "already_paused");
    await assert.rejects(
      () => admit(
        started.baseUrl,
        prepared.fetchImpl,
        prepared.projectId,
        prepared.chapterId,
        id("c151"),
        "Help with this passage.",
        { kind: "existing", conversation_id: created.conversation_id },
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 422 && problemCode(error) === "conversation_busy";
      },
    );
    await assert.rejects(
      () => postPause(started.baseUrl, browserFetch(started.baseUrl, "session-b"), prepared.projectId, created.effect.run_id, id("c161"), id("c162")),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    await assert.rejects(
      () => postCancel(started.baseUrl, browserFetch(started.baseUrl, "session-b"), prepared.projectId, created.effect.run_id, id("c163"), id("c164")),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    const cancelled = await postCancel(started.baseUrl, prepared.fetchImpl, prepared.projectId, created.effect.run_id, id("c171"), id("c172"));
    assert.equal(cancelled.response.effect.kind, "applied");
    if (cancelled.response.effect.kind !== "applied") throw new Error("expected cancel applied");
    assert.equal(cancelled.response.effect.status, "cancelled");
    assert.notEqual(cancelled.response.effect.fence_generation, paused.response.effect.kind === "applied" ? paused.response.effect.fence_generation : "");
    const cancelReplay = await cancelAgentRun({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      runId: created.effect.run_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("c171"),
      antiForgery: cancelled.challenge.nonce,
      request: cancelRequest(id("c172")),
    });
    assert.equal(cancelReplay.command_id, cancelled.response.command_id);
    assert.equal(cancelReplay.receipt.receipt_id, cancelled.response.receipt.receipt_id);
    const cancelledInspect = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, created.effect.run_id);
    assert.equal(cancelledInspect.status, "cancelled");
    assert.equal(cancelledInspect.decision.kind, "absent");
    assert.equal(cancelledInspect.usage.kind, "unknown");
    const cancelledActivity = activityEvent(await activityStream({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      snapshotId: tree.snapshot.snapshot_id,
      protocolRelease: "storyos.public.release.1",
      fetchImpl: prepared.fetchImpl,
    }), "agent_run_cancelled");
    assert.equal(cancelledActivity?.event_schema, "storyos.event.agent-run-cancelled.v1");
    const alreadyCancelled = await postCancel(started.baseUrl, prepared.fetchImpl, prepared.projectId, created.effect.run_id, id("c173"), id("c174"));
    assert.equal(alreadyCancelled.response.effect.kind, "no_effect");
    if (alreadyCancelled.response.effect.kind !== "no_effect") throw new Error("expected already cancelled");
    assert.equal(alreadyCancelled.response.effect.reason, "already_cancelled");
    const successor = await admit(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      prepared.chapterId,
      id("c181"),
      "Help with this passage.",
      { kind: "existing", conversation_id: created.conversation_id },
    );
    assert.notEqual(successor.effect.run_id, created.effect.run_id);
    await settleOnce();
    const successorInspect = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, successor.effect.run_id);
    assert.equal(successorInspect.status, "completed");
    const terminal = await postCancel(started.baseUrl, prepared.fetchImpl, prepared.projectId, successor.effect.run_id, id("c191"), id("c192"));
    assert.equal(terminal.response.effect.kind, "conflicted");
    if (terminal.response.effect.kind !== "conflicted") throw new Error("expected terminal");
    assert.equal(terminal.response.effect.reason, "terminal_run");
    assert.equal((await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, successor.effect.run_id)).status, "completed");
  } finally {
    await stopRealServer(started.server);
  }
});

test("a rate-limited cancellation Challenge completes before a Worker is held", async () => {
  const dispatchHold = join(tmpdir(), "storyos-s3-17-rate-limit.hold");
  const started = await startRealServer();
  let worker: Promise<void> | undefined;
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("c311"), "Challenge Window Novel", "c4");
    const run = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("c321"));
    let challengeAttempts = 0;
    const rateLimitedFetch: typeof fetch = (input, init) => {
      const path = new URL(input instanceof Request ? input.url : String(input)).pathname;
      if (path.endsWith("/anti-forgery-challenges")) {
        assert.equal(existsSync(dispatchHold), false, "Challenge retry must complete before the Worker hold");
        challengeAttempts += 1;
        if (challengeAttempts === 1) {
          return Promise.resolve(Response.json({ code: "challenge_rate_limited" }, {
            status: 429,
            headers: { "retry-after": "1" },
          }));
        }
      }
      return prepared.fetchImpl(input, init);
    };
    const challenge = await cancelChallenge(started.baseUrl, rateLimitedFetch, prepared.projectId, id("c331"), id("c332"));
    assert.equal(challengeAttempts, 2);
    writeFileSync(dispatchHold, "hold");
    worker = settleHeld({ STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: dispatchHold });
    await waitFor(
      () => inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, run.effect.run_id),
      (current) => current.status === "claimed" && current.model_attempt.kind === "present",
    );
    const cancelled = await postCancel(started.baseUrl, rateLimitedFetch, prepared.projectId, run.effect.run_id, id("c331"), id("c332"), challenge);
    assert.equal(cancelled.response.effect.kind, "applied");
    unlinkSync(dispatchHold);
    await worker;
    assert.equal((await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, run.effect.run_id)).status, "cancelled");
  } finally {
    if (existsSync(dispatchHold)) unlinkSync(dispatchHold);
    if (worker !== undefined) await Promise.allSettled([worker]);
    await stopRealServer(started.server);
  }
});

test("cancellation fences late Worker output and does not hide a Proposal", async () => {
  const dispatchHold = join(tmpdir(), "storyos-s3-17-dispatch.hold");
  const decisionHold = join(tmpdir(), "storyos-s3-17-decision.hold");
  const started = await startRealServer();
  const workers: Array<Promise<void>> = [];
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("c211"), "Fence Recover Novel", "c3");
    const cancelFetch: typeof fetch = (input, init) => {
      const path = new URL(input instanceof Request ? input.url : String(input)).pathname;
      if (path.endsWith("/anti-forgery-challenges")) {
        assert.ok(!existsSync(dispatchHold) && !existsSync(decisionHold),
          "cancellation Challenge requested while a Worker is held");
      }
      return prepared.fetchImpl(input, init);
    };
    const dispatchRun = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("c221"));
    const dispatchCancelChallenge = await cancelChallenge(started.baseUrl, cancelFetch, prepared.projectId, id("c231"), id("c232"));
    writeFileSync(dispatchHold, "hold");
    const dispatchWorker = settleHeld({
      STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: dispatchHold,
      STORYOS_EXPORT_LEASE_TTL_SECS: "0",
    });
    workers.push(dispatchWorker);
    await waitFor(
      () => inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, dispatchRun.effect.run_id),
      (current) => current.status === "claimed" && current.model_attempt.kind === "present",
    );
    const cancelledHold = await postCancel(started.baseUrl, cancelFetch, prepared.projectId, dispatchRun.effect.run_id, id("c231"), id("c232"), dispatchCancelChallenge);
    assert.equal(cancelledHold.response.effect.kind, "applied");
    unlinkSync(dispatchHold);
    await dispatchWorker;
    const afterCancel = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, dispatchRun.effect.run_id);
    assert.equal(afterCancel.status, "cancelled");
    assert.equal(afterCancel.decision.kind, "absent");
    assert.equal(afterCancel.usage.kind, "unknown");
    assert.equal(afterCancel.model_attempt.kind, "present");
    if (afterCancel.model_attempt.kind !== "present") throw new Error("expected attempt");
    assert.equal(afterCancel.model_attempt.dispatch_state, "uncertain");
    await settleOnce();
    assert.equal((await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, dispatchRun.effect.run_id)).status, "cancelled");

    writeFileSync(dispatchHold, "hold");
    const recoverRun = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("c241"));
    const staleWorker = settleHeld({
      STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: dispatchHold,
      STORYOS_EXPORT_LEASE_TTL_SECS: "0",
    });
    workers.push(staleWorker);
    const beforeRecover = await waitFor(
      () => inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, recoverRun.effect.run_id),
      (current) => current.status === "claimed" && current.model_attempt.kind === "present",
    );
    if (beforeRecover.model_attempt.kind !== "present") throw new Error("expected attempt");
    const attemptId = beforeRecover.model_attempt.model_attempt_id;
    const recoverer = settleOnce({ STORYOS_EXPORT_LEASE_TTL_SECS: "0" });
    workers.push(recoverer);
    const recovered = await waitFor(
      () => inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, recoverRun.effect.run_id),
      (current) => current.status === "completed" && current.model_attempt.kind === "present",
    );
    unlinkSync(dispatchHold);
    await Promise.all([staleWorker, recoverer]);
    assert.equal(recovered.status, "completed");
    if (recovered.model_attempt.kind !== "present") throw new Error("expected recovered attempt");
    assert.equal(recovered.model_attempt.model_attempt_id, attemptId);
    assert.equal(recovered.decision.kind, "advisory");

    const blocked = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("c251"));
    const unavailable: UpdateProjectAssistanceRequest = {
      command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: {
        availability: "unavailable",
        expected_assistance_revision: "1",
        ...BINDING,
        correlation_id: id("c252"),
      },
    };
    await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "PUT", "/api/v1/projects/{project_id}/assistance", unavailable.command_schema, await digestUpdateProjectAssistance(unavailable), id("c253"), (antiForgery) => updateProjectAssistance({ baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl, idempotencyKey: id("c253"), antiForgery, request: unavailable }));
    await settleOnce();
    const blockedInspect = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, blocked.effect.run_id);
    assert.equal(blockedInspect.status, "refused");
    assert.notEqual(blockedInspect.status, "cancelled");

    const restore: UpdateProjectAssistanceRequest = {
      command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: {
        availability: "available",
        expected_assistance_revision: "2",
        ...BINDING,
        correlation_id: id("c254"),
      },
    };
    await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "PUT", "/api/v1/projects/{project_id}/assistance", restore.command_schema, await digestUpdateProjectAssistance(restore), id("c255"), (antiForgery) => updateProjectAssistance({ baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl, idempotencyKey: id("c255"), antiForgery, request: restore }));

    const proposalRun = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("c261"), "Revise this passage: keep the voice.");
    const proposalCancelChallenge = await cancelChallenge(started.baseUrl, cancelFetch, prepared.projectId, id("c271"), id("c272"));
    writeFileSync(decisionHold, "hold");
    const decisionWorker = settleHeld({ STORYOS_TEST_FAKE_DECISION_HOLD_PATH: decisionHold });
    workers.push(decisionWorker);
    const beforeCancelProposal = await waitFor(
      () => inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, proposalRun.effect.run_id),
      (current) => current.decision.kind === "prose_change" && current.decision.opened_proposal.kind === "present",
    );
    if (beforeCancelProposal.decision.kind !== "prose_change") throw new Error("expected prose");
    if (beforeCancelProposal.decision.opened_proposal.kind !== "present") throw new Error("expected proposal");
    const proposalId = beforeCancelProposal.decision.opened_proposal.proposal_id;
    const cancelledProposal = await postCancel(started.baseUrl, cancelFetch, prepared.projectId, proposalRun.effect.run_id, id("c271"), id("c272"), proposalCancelChallenge);
    assert.equal(cancelledProposal.response.effect.kind, "applied");
    unlinkSync(decisionHold);
    await decisionWorker;
    const proposal = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(proposal.proposal.closure, "open");
    assert.equal(proposal.proposal.operation_resolution, "pending");
    assert.equal(proposal.proposal.reservation_state, "unresolved");
    const cancelledProposalInspect = await inspectRun(started.baseUrl, prepared.fetchImpl, prepared.projectId, proposalRun.effect.run_id);
    assert.equal(cancelledProposalInspect.status, "cancelled");
    assert.equal(cancelledProposalInspect.decision.kind, "prose_change");
  } finally {
    if (existsSync(dispatchHold)) unlinkSync(dispatchHold);
    if (existsSync(decisionHold)) unlinkSync(decisionHold);
    await Promise.allSettled(workers);
    await stopRealServer(started.server);
  }
});
