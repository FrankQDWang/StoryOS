// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/complete-fake-model-decision-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import {
  cancelAgentRun,
  createProjectCommandChallenge,
  digestCancelAgentRun,
  digestPauseAgentRun,
  digestUpdateProjectAssistance,
  activityStream,
  getChapter,
  getManuscriptTree,
  getProposal,
  pauseAgentRun,
  updateProjectAssistance,
  withdrawProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CancelAgentRunRequest,
  CreateAgentRunRequest,
  PauseAgentRunRequest,
  UpdateProjectAssistanceRequest,
  WithdrawProposalRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";
import { BINDING, USER_A, UUID_V7, admitProse, challenged, drainLeftoverWork, id, inspectRun, issueChallenge,
  postRun, prepare, problemCode, runRequest, settleHeld, settleOnce, startServer, waitFor } from "../support/http-harness.ts";

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

async function admit(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  key: string,
  text = "Help with this passage.",
  conversation: CreateAgentRunRequest["create_agent_run_input"]["conversation"] = { kind: "new" },
) {
  const created = await postRun(baseUrl, fetchImpl, projectId, key, runRequest(chapterId, id(key.slice(-4)), text, conversation));
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
  let nonce = "";
  const response = await challenged(baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/agent-runs/{run_id}/pause", request.command_schema, await digestPauseAgentRun(request),
    key, (antiForgery) => pauseAgentRun({ baseUrl, projectId, runId, fetchImpl, idempotencyKey: key,
      antiForgery: (nonce = antiForgery), request }));
  return { nonce, response };
}

async function cancelChallenge(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  key: string,
  correlationId: string,
) {
  const request = cancelRequest(correlationId);
  return issueChallenge(baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
    request.command_schema, await digestCancelAgentRun(request), key);
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

function activityEvent(body: string, eventKind: string) {
  return body.split("\n\n").map((block) => {
    const data = block.split("\n").find((line) => line.startsWith("data:"));
    return data === undefined ? undefined : JSON.parse(data.slice(5).trim()) as {
      event_kind?: string;
      event_schema?: string;
    };
  }).find((event) => event?.event_kind === eventKind);
}

test("pauseAgentRun and cancelAgentRun stay distinct and keep a terminal Run immutable", async () => {
  const started = await startServer();
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
      antiForgery: paused.nonce,
      request: pauseRequest(id("c142")),
    });
    assert.equal(pauseReplay.command_id, paused.response.command_id);
    assert.equal(pauseReplay.receipt.receipt_id, paused.response.receipt.receipt_id);
    const pausedInspect = await inspectRun(started.baseUrl, prepared, created.effect.run_id);
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
    const cancelledInspect = await inspectRun(started.baseUrl, prepared, created.effect.run_id);
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
    const successorInspect = await inspectRun(started.baseUrl, prepared, successor.effect.run_id);
    assert.equal(successorInspect.status, "completed");
    const terminal = await postCancel(started.baseUrl, prepared.fetchImpl, prepared.projectId, successor.effect.run_id, id("c191"), id("c192"));
    assert.equal(terminal.response.effect.kind, "conflicted");
    if (terminal.response.effect.kind !== "conflicted") throw new Error("expected terminal");
    assert.equal(terminal.response.effect.reason, "terminal_run");
    assert.equal((await inspectRun(started.baseUrl, prepared, successor.effect.run_id)).status, "completed");
  } finally {
    await stopRealServer(started.server);
  }
});

test("a rate-limited cancellation Challenge completes before a Worker is held", async () => {
  const dispatchHold = join(tmpdir(), "storyos-s3-17-rate-limit.hold");
  const started = await startServer();
  let worker: Promise<void> | undefined;
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("c311"), "Challenge Window Novel", "c4");
    const run = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("c321"));
    let challengeAttempts = 0;
    const rateLimitedFetch: typeof fetch = (input, init) => {
      const path = new URL(input instanceof Request ? input.url : String(input)).pathname;
      if (path.endsWith("/anti-forgery-challenges")) {
        assert.equal(existsSync(dispatchHold), false, "Challenge preparation must complete before the Worker hold");
        challengeAttempts += 1;
      }
      return prepared.fetchImpl(input, init);
    };
    // Fill the current and the next minute window. Earlier Challenges can leave rows on two sides of a
    // minute boundary, and the minute can change before the refused request.
    await queryPostgres(`INSERT INTO storyos.project_command_challenge_rate_windows
        (owner_user_id, project_id, client_session_generation, policy_revision, window_started_at, issued_count)
      SELECT DISTINCT rate.owner_user_id, rate.project_id, rate.client_session_generation, rate.policy_revision,
        date_trunc('minute', now()) + window_offset.value, 10
      FROM storyos.project_command_challenge_rate_windows AS rate
      CROSS JOIN (VALUES (interval '0 minutes'), (interval '1 minute')) AS window_offset(value)
      WHERE rate.project_id = '${prepared.projectId}'::uuid
      ON CONFLICT (owner_user_id, project_id, client_session_generation, policy_revision, window_started_at)
      DO UPDATE SET issued_count = 10;`);
    const request = cancelRequest(id("c332"));
    const digest = await digestCancelAgentRun(request);
    await assert.rejects(() => createProjectCommandChallenge({
      baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: rateLimitedFetch,
      request: { method: "POST", route_template: "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
        command_schema: request.command_schema, canonical_command_digest: digest, idempotency_key: id("c331") },
    }), (error) => requireStoryOSProtocolError(error).status === 429);
    const challenge = await cancelChallenge(started.baseUrl, rateLimitedFetch, prepared.projectId, id("c331"), id("c332"));
    assert.equal(challengeAttempts, 2);
    writeFileSync(dispatchHold, "hold");
    worker = settleHeld({ STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: dispatchHold });
    await waitFor(
      () => inspectRun(started.baseUrl, prepared, run.effect.run_id),
      (current) => current.status === "claimed" && current.model_attempt.kind === "present",
    );
    const cancelled = await postCancel(started.baseUrl, rateLimitedFetch, prepared.projectId, run.effect.run_id, id("c331"), id("c332"), challenge);
    assert.equal(cancelled.response.effect.kind, "applied");
    unlinkSync(dispatchHold);
    await worker;
    await settleOnce();
    assert.equal((await inspectRun(started.baseUrl, prepared, run.effect.run_id)).status, "cancelled");
  } finally {
    if (existsSync(dispatchHold)) unlinkSync(dispatchHold);
    if (worker !== undefined) await Promise.allSettled([worker]);
    await stopRealServer(started.server);
  }
});

test("cancellation fences late Worker output and does not hide a Proposal", async () => {
  const dispatchHold = join(tmpdir(), "storyos-s3-17-dispatch.hold");
  const decisionHold = join(tmpdir(), "storyos-s3-17-decision.hold");
  const started = await startServer();
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
      () => inspectRun(started.baseUrl, prepared, dispatchRun.effect.run_id),
      (current) => current.status === "claimed" && current.model_attempt.kind === "present",
    );
    const cancelledHold = await postCancel(started.baseUrl, cancelFetch, prepared.projectId, dispatchRun.effect.run_id, id("c231"), id("c232"), dispatchCancelChallenge);
    assert.equal(cancelledHold.response.effect.kind, "applied");
    unlinkSync(dispatchHold);
    await dispatchWorker;
    await settleOnce();
    const afterCancel = await inspectRun(started.baseUrl, prepared, dispatchRun.effect.run_id);
    assert.equal(afterCancel.status, "cancelled");
    assert.equal(afterCancel.decision.kind, "absent");
    assert.equal(afterCancel.usage.kind, "unknown");
    assert.equal(afterCancel.model_attempt.kind, "present");
    if (afterCancel.model_attempt.kind !== "present") throw new Error("expected attempt");
    assert.equal(afterCancel.model_attempt.dispatch_state, "uncertain");
    assert.equal(await queryPostgres(`
      SELECT count(*)::text || ' ' || count(DISTINCT outbound_disclosure_event_id)::text
          || ' ' || min(payload->>'result')
        FROM storyos.model_attempts
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${dispatchRun.effect.run_id}'::uuid
         AND attempt_role = 'abort';
    `), "1 1 acknowledged");
    await settleOnce();
    assert.equal((await inspectRun(started.baseUrl, prepared, dispatchRun.effect.run_id)).status, "cancelled");

    writeFileSync(dispatchHold, "hold");
    const recoverRun = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("c241"));
    const staleWorker = settleHeld({
      STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: dispatchHold,
      STORYOS_EXPORT_LEASE_TTL_SECS: "0",
    });
    workers.push(staleWorker);
    const beforeRecover = await waitFor(
      () => inspectRun(started.baseUrl, prepared, recoverRun.effect.run_id),
      (current) => current.status === "claimed" && current.model_attempt.kind === "present",
    );
    if (beforeRecover.model_attempt.kind !== "present") throw new Error("expected attempt");
    const attemptId = beforeRecover.model_attempt.model_attempt_id;
    const recoverer = settleOnce({ STORYOS_EXPORT_LEASE_TTL_SECS: "0" });
    workers.push(recoverer);
    const recovered = await waitFor(
      () => inspectRun(started.baseUrl, prepared, recoverRun.effect.run_id),
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
    const blockedInspect = await inspectRun(started.baseUrl, prepared, blocked.effect.run_id);
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
      () => inspectRun(started.baseUrl, prepared, proposalRun.effect.run_id),
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
    const cancelledProposalInspect = await inspectRun(started.baseUrl, prepared, proposalRun.effect.run_id);
    assert.equal(cancelledProposalInspect.status, "cancelled");
    assert.equal(cancelledProposalInspect.decision.kind, "prose_change");
  } finally {
    if (existsSync(dispatchHold)) unlinkSync(dispatchHold);
    if (existsSync(decisionHold)) unlinkSync(decisionHold);
    await Promise.allSettled(workers);
    await stopRealServer(started.server);
  }
});

test("a Pause before the Cancel still sends one Abort for the in-flight Attempt", async () => {
  const dispatchHold = join(tmpdir(), "storyos-g03-pause-cancel.hold");
  const started = await startServer();
  let worker: Promise<void> | undefined;
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("c611"), "Pause Cancel Novel", "c6");
    const run = await admit(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("c621"));
    const challenge = await cancelChallenge(started.baseUrl, prepared.fetchImpl, prepared.projectId, id("c631"), id("c632"));
    writeFileSync(dispatchHold, "hold");
    worker = settleHeld({ STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: dispatchHold });
    await waitFor(
      () => inspectRun(started.baseUrl, prepared, run.effect.run_id),
      (current) => current.status === "claimed" && current.model_attempt.kind === "present",
    );
    const paused = await postPause(started.baseUrl, prepared.fetchImpl, prepared.projectId, run.effect.run_id, id("c641"), id("c642"));
    assert.equal(paused.response.effect.kind, "applied");
    const cancelled = await postCancel(started.baseUrl, prepared.fetchImpl, prepared.projectId, run.effect.run_id, id("c631"), id("c632"), challenge);
    assert.equal(cancelled.response.effect.kind, "applied");
    unlinkSync(dispatchHold);
    await worker;
    await settleOnce();
    const after = await inspectRun(started.baseUrl, prepared, run.effect.run_id);
    assert.equal(after.status, "cancelled");
    assert.equal(after.decision.kind, "absent");
    assert.equal(await queryPostgres(`
      SELECT count(*)::text || ' ' || count(DISTINCT outbound_disclosure_event_id)::text
          || ' ' || min(payload->>'result')
        FROM storyos.model_attempts
       WHERE project_id = '${prepared.projectId}'::uuid
         AND run_id = '${run.effect.run_id}'::uuid
         AND attempt_role = 'abort';
    `), "1 1 acknowledged");
  } finally {
    if (existsSync(dispatchHold)) unlinkSync(dispatchHold);
    if (worker !== undefined) await Promise.allSettled([worker]);
    await stopRealServer(started.server);
  }
});

test("current producer withdraw closes the Proposal with zero author actions", async () => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("1309e8f00211"), "Producer Withdraw Novel", "e8f3");
    const before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const queried = await admitProse(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("e8f00231"),
    );
    if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
      throw new Error("expected opened prose");
    }
    const opened = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: queried.decision.opened_proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(opened.proposal.closure, "open");
    if (opened.proposal.source.kind !== "agent_run_decision") {
      throw new Error("expected an agent-run producer");
    }
    const mismatch: WithdrawProposalRequest = {
      command_schema: "storyos.command.withdraw-proposal.request.v1",
      withdraw_proposal_input: {
        cause: "current_producer",
        producer: {
          kind: "agent_run_decision",
          run_id: id("e8f00299"),
          decision_id: id("e8f00298"),
        },
        proposal_revision_id: opened.proposal.revision_id,
        expected_closure: "open",
        expected_target_revisions: [before.chapter.current_revision.revision_id],
        withdrawal_reason: { kind: "current_producer_withdrew" },
        ...BINDING,
        correlation_id: id("e8f00241"),
      },
    };
    const mismatched = await withdrawProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("e8f00242"),
      request: mismatch,
    });
    assert.deepEqual(mismatched.effect, { kind: "no_effect", reason: "unsupported_cause" });
    assert.equal(mismatched.author_command_admission_id, undefined);
    const stillOpen = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(stillOpen.proposal.closure, "open");
    const withdrawRequest: WithdrawProposalRequest = {
      command_schema: "storyos.command.withdraw-proposal.request.v1",
      withdraw_proposal_input: {
        cause: "current_producer",
        producer: {
          kind: "agent_run_decision",
          run_id: opened.proposal.source.run_id,
          decision_id: opened.proposal.source.decision_id,
        },
        proposal_revision_id: opened.proposal.revision_id,
        expected_closure: "open",
        expected_target_revisions: [before.chapter.current_revision.revision_id],
        withdrawal_reason: { kind: "current_producer_withdrew" },
        ...BINDING,
        correlation_id: id("e8f00251"),
      },
    };
    const withdrawn = await withdrawProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("e8f00252"),
      request: withdrawRequest,
    });
    assert.equal(withdrawn.effect.kind, "resolved");
    if (withdrawn.effect.kind !== "resolved") throw new Error("expected resolved withdrawal");
    assert.equal(withdrawn.effect.prior_closure, "open");
    assert.equal(withdrawn.effect.resulting_closure, "withdrawn");
    assert.equal(withdrawn.effect.withdrawal_reason.kind, "current_producer_withdrew");
    assert.equal(withdrawn.effect.author_action_sequence, undefined);
    assert.equal(withdrawn.effect.undo_disposition, undefined);
    assert.equal(withdrawn.author_command_admission_id, undefined);
    assert.equal(withdrawn.receipt.author_command_admission_id, undefined);
    assert.deepEqual(withdrawn.receipt.authoritative_commit_ids, []);
    const repeated = await withdrawProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("e8f00252"),
      request: withdrawRequest,
    });
    assert.equal(repeated.receipt.receipt_id, withdrawn.receipt.receipt_id);
    assert.equal(repeated.effect.kind, "resolved");
    const inspected = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(inspected.proposal.closure, "withdrawn");
    assert.equal(inspected.proposal.operation_resolution, "pending");
    assert.equal(inspected.proposal.revision_id, opened.proposal.revision_id);
    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(after.chapter, before.chapter);
    const closedRequest: WithdrawProposalRequest = {
      ...withdrawRequest,
      withdraw_proposal_input: {
        ...withdrawRequest.withdraw_proposal_input,
        correlation_id: id("e8f00261"),
      },
    };
    const closed = await withdrawProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("e8f00262"),
      request: closedRequest,
    });
    assert.deepEqual(closed.effect, { kind: "no_effect", reason: "closure_not_open" });
  } finally {
    await stopRealServer(started.server);
  }
});
