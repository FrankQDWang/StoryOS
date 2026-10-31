// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/update-project-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import {
  createAgentRun,
  digestCreateAgentRun,
  digestUpdateProjectAssistance,
  getProject,
  getProjectAssistance,
  updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateAgentRunRequest,
  GetAgentRunResponse,
  UpdateProjectAssistanceRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";
import { BINDING, PROSE, USER_A, UUID_V7, admitRun, challenged, createEmpty, drainLeftoverWork, freshNamespace, id,
  inspectRun, issueChallenge, prepare, problemCode, runRequest, settleOnce, startServer, waitFor } from "../support/http-harness.ts";

const HOST_FAKE_MODEL_REGISTRATION = "018f0000-0000-7001-8000-00000000fa01";
const ADVISORY = "This passage is inspectable Host-fake advice. It is not Authoritative State.";
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

function assistanceRequest(
  availability: "available" | "unavailable",
  expectedAssistanceRevision: string,
  correlationId: string,
): UpdateProjectAssistanceRequest {
  return {
    command_schema: "storyos.command.update-project-assistance.request.v1",
    update_project_assistance_input: {
      availability,
      expected_assistance_revision: expectedAssistanceRevision,
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

async function prepareAssistance(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  idempotencyKey: string,
  request: UpdateProjectAssistanceRequest,
) {
  let nonce = "";
  const updated = await challenged(baseUrl, fetchImpl, projectId, "PUT", "/api/v1/projects/{project_id}/assistance",
    request.command_schema, await digestUpdateProjectAssistance(request), idempotencyKey, (antiForgery) =>
      updateProjectAssistance({ baseUrl, projectId, fetchImpl, idempotencyKey, antiForgery: (nonce = antiForgery), request }));
  return { nonce, updated };
}

/** Sends one createAgentRun command and returns its Command Challenge nonce with the response. */
async function postRunWithChallenge(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  key: string,
  request: CreateAgentRunRequest,
) {
  let nonce = "";
  const admitted = await challenged(baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/agent-runs",
    request.command_schema, await digestCreateAgentRun(request), key, (antiForgery) =>
      createAgentRun({ baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery: (nonce = antiForgery), request }));
  return { nonce, admitted };
}

test("Worker refuses Tool, MCP, research, embedding, Memory, Skill, Subrun, and Eval without dispatch", async () => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e141"), "Refuse Novel", "e5");
    let conversationId: string | undefined;
    for (const [index, [text, capability]] of REFUSALS.entries()) {
      const created = await admitRun(
        started.baseUrl,
        prepared,
        id(`e5${index}`),
        text,
        conversationId === undefined ? { kind: "new" } : { kind: "existing", conversation_id: conversationId },
      );
      conversationId = created.conversation_id;
      await settleOnce();
      const queried = await inspectRun(started.baseUrl, prepared, created.effect.run_id);
      assert.equal(queried.status, "refused");
      assert.deepEqual(queried.decision, { kind: "execution_refused", capability });
      assert.equal(queried.model_attempt.kind, "absent");
      assert.deepEqual(queried.evidence, []);
      assert.equal(queried.context.destination_io.kind, "none");
    }
    const blocked = await prepare(started.baseUrl, id("e161"), "Blocked Novel", "e7");
    await queryPostgres(`
      UPDATE storyos.authoritative_payloads AS payload
         SET canonical_bytes = convert_to(repeat('a', 32001), 'UTF8')
        FROM storyos.authoritative_heads AS head
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id, revision.revision_id) =
             (head.owner_user_id, head.project_id, head.manuscript_object_id, head.current_revision_id)
       WHERE (payload.owner_user_id, payload.project_id, payload.payload_id) =
             (revision.owner_user_id, revision.project_id, revision.payload_id)
         AND head.project_id = '${blocked.projectId}'::uuid
         AND head.manuscript_object_id = '${blocked.chapterId}'::uuid;
    `);
    const createdBlocked = await admitRun(started.baseUrl, blocked, id("e181"), "Help with this passage.");
    await settleOnce();
    const refused = await inspectRun(started.baseUrl, blocked, createdBlocked.effect.run_id);
    assert.equal(refused.status, "refused");
    assert.deepEqual(refused.decision, { kind: "execution_refused", capability: "blocked_context" });
    assert.equal(refused.model_attempt.kind, "absent");
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.model_attempts WHERE project_id IN ('${prepared.projectId}'::uuid, '${blocked.projectId}'::uuid);`), "0");
  } finally {
    await stopRealServer(started.server);
  }
});

test("Worker distinguishes prose-change, clarification, and CFP holds", async () => {
  const started = await startServer();
  const dispatchHold = join(tmpdir(), `storyos-fake-dispatch-${process.pid}`);
  const streamHold = join(tmpdir(), `storyos-fake-stream-${process.pid}`);
  const decisionHold = join(tmpdir(), `storyos-fake-decision-${process.pid}`);
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("e191"), "Hold Novel", "ea");
    const prose = await admitRun(started.baseUrl, prepared, id("eb11"), "Revise this passage: keep the voice.");
    await settleOnce();
    const proseInspect = await inspectRun(started.baseUrl, prepared, prose.effect.run_id);
    assert.equal(proseInspect.status, "completed");
    assert.equal(proseInspect.decision.kind, "prose_change");
    if (proseInspect.decision.kind !== "prose_change") throw new Error("expected prose");
    assert.equal(proseInspect.decision.selected, true);
    assert.equal(proseInspect.decision.text, PROSE);
    assert.equal(proseInspect.decision.producer_input, PROSE);
    assert.equal(proseInspect.decision.authoritative, false);
    assert.equal(proseInspect.decision.continuation.kind, "present");

    const clarification = await admitRun(started.baseUrl, prepared, id("eb13"), "Which wording should I keep? The first clause.");
    await settleOnce();
    const clarified = await inspectRun(started.baseUrl, prepared, clarification.effect.run_id);
    assert.equal(clarified.status, "completed");
    assert.equal(clarified.decision.kind, "clarification");
    if (clarified.decision.kind !== "clarification") throw new Error("expected clarification");
    assert.equal(clarified.decision.selected, true);
    assert.equal(clarified.decision.question, CLARIFICATION);
    assert.equal(clarified.decision.required_reply, CLARIFICATION);
    assert.equal(clarified.decision.continuation.kind, "absent");

    writeFileSync(dispatchHold, "hold");
    const dispatchRun = await admitRun(started.baseUrl, prepared, id("eb15"), "Help with this passage.");
    const dispatchWorker = settleOnce({ STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: dispatchHold });
    const beforeDecision = await waitFor(
      () => inspectRun(started.baseUrl, prepared, dispatchRun.effect.run_id),
      (current) => current.status === "claimed" && current.model_attempt.kind === "present",
    );
    assert.equal(beforeDecision.decision.kind, "absent");
    assert.equal(beforeDecision.context.destination_io.kind, "host_fake");
    assertEvidence(beforeDecision, "Help with this passage.");
    unlinkSync(dispatchHold);
    await dispatchWorker;
    writeFileSync(streamHold, "hold");
    const streamRun = await admitRun(started.baseUrl, prepared, id("eb16"), "Help with this passage.");
    const streamWorker = settleOnce({ STORYOS_TEST_FAKE_STREAM_HOLD_PATH: streamHold });
    const beforeDecisionCut = await waitFor(
      () => inspectRun(started.baseUrl, prepared, streamRun.effect.run_id),
      (current) => current.items.length > 0 && current.decision.kind === "absent",
    );
    assert.equal(beforeDecisionCut.status, "claimed");
    unlinkSync(streamHold);
    await streamWorker;
    assert.equal((await inspectRun(started.baseUrl, prepared, streamRun.effect.run_id)).status, "completed");
    const unselected = await admitRun(started.baseUrl, prepared, id("eb18"), "SCRIPT:unselected");
    await settleOnce();
    const unselectedInspect = await inspectRun(started.baseUrl, prepared, unselected.effect.run_id);
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
      const scripted = await admitRun(
        started.baseUrl,
        prepared,
        id(`ec1${scriptIndex}`),
        text,
      );
      scriptIndex += 1;
      await settleOnce();
      const inspect = await inspectRun(started.baseUrl, prepared, scripted.effect.run_id);
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
    const decisionRun = await admitRun(started.baseUrl, prepared, id("eb17"), "Help with this passage.");
    const decisionWorker = settleOnce({ STORYOS_TEST_FAKE_DECISION_HOLD_PATH: decisionHold });
    const beforeContinuation = await waitFor(
      () => inspectRun(started.baseUrl, prepared, decisionRun.effect.run_id),
      (current) => current.decision.kind === "advisory" && current.decision.continuation.kind === "absent",
    );
    assert.equal(beforeContinuation.status, "claimed");
    unlinkSync(decisionHold);
    await decisionWorker;
    const afterDecision = await inspectRun(started.baseUrl, prepared, decisionRun.effect.run_id);
    assert.equal(afterDecision.status, "completed");
    assert.equal(afterDecision.decision.kind, "advisory");
    if (afterDecision.decision.kind !== "advisory") throw new Error("expected advisory");
    assert.equal(afterDecision.decision.continuation.kind, "present");
    assert.equal(afterDecision.decision.selected, true);
    assert.equal(afterDecision.decision.text, ADVISORY);
    assert.match(afterDecision.decision.decision_id, UUID_V7);
    assert.equal(afterDecision.context.destination_io.kind, "host_fake");
    assert.equal(afterDecision.context.destination_context_manifest.kind, "present");
    assert.equal(afterDecision.context.outbound_disclosure_manifest.kind, "present");
    assert.equal(afterDecision.context.host_control.distinct_from_destination, true);
    assert.equal(afterDecision.context.host_control.destination_visible, true);
    if (afterDecision.model_attempt.kind !== "present") throw new Error("expected attempt");
    assert.match(afterDecision.model_attempt.model_attempt_id, UUID_V7);
    assert.equal(afterDecision.model_attempt.dispatch_state, "settled");
    assertEvidence(afterDecision, "Help with this passage.");
    assert.equal(afterDecision.redaction_profile, "storyos.author.v1");
    assert.equal(afterDecision.items[0]?.state, "complete");
    assert.equal(afterDecision.items[0]?.phase, "complete");
    assert.equal(afterDecision.usage.kind, "unknown");
    const decisionRunId = decisionRun.effect.run_id;
    assert.equal((await inspectRun(started.baseUrl, prepared, decisionRunId,
      afterDecision.model_attempt.model_attempt_id)).status, "completed");
    if (beforeDecision.model_attempt.kind !== "present") throw new Error("expected dispatch attempt");
    for (const attemptId of [beforeDecision.model_attempt.model_attempt_id, id("dead")]) {
      await assert.rejects(
        () => inspectRun(started.baseUrl, prepared, decisionRunId, attemptId),
        (error) => requireStoryOSProtocolError(error).status === 404,
      );
    }
    await assert.rejects(
      () => inspectRun(started.baseUrl, { projectId: prepared.projectId, fetchImpl: browserFetch(started.baseUrl, "session-b") }, decisionRunId),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.model_attempts WHERE project_id = '${prepared.projectId}'::uuid AND run_id = '${decisionRunId}'::uuid;`), "1");
  } finally {
    if (existsSync(dispatchHold)) unlinkSync(dispatchHold);
    if (existsSync(streamHold)) unlinkSync(streamHold);
    if (existsSync(decisionHold)) unlinkSync(decisionHold);
    await stopRealServer(started.server);
  }
});

test("project assistance prepares the host fake binding without a run", async () => {
  const ns = freshNamespace();
  let started = await startServer();
  try {
    const first = await createEmpty(started.baseUrl, "session-a", id(`${ns}0a21`), "Assistance Novel", id("a10"));
    const responses: Buffer[] = [];
    const captureFetch: typeof fetch = async (input, init) => {
      const response = await first.fetchImpl(input, init);
      if (init?.method === "PUT") responses.push(Buffer.from(await response.clone().arrayBuffer()));
      return response;
    };
    await assert.rejects(
      () => getProjectAssistance({ baseUrl: started.baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl }),
      (error) => requireStoryOSProtocolError(error).status === 404 && problemCode(error) === "assistance_not_bound",
    );
    const absentRequest = assistanceRequest("available", "1", "018f0000-0000-7001-8000-000000000a2f");
    const absentKey = id(`${ns}0a20`);
    const absentDigest = await digestUpdateProjectAssistance(absentRequest);
    const absentChallenge = await issueChallenge(started.baseUrl, captureFetch, first.projectId, "PUT",
      "/api/v1/projects/{project_id}/assistance", absentRequest.command_schema, absentDigest, absentKey);
    const retryAbsent = () => updateProjectAssistance({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: captureFetch,
      idempotencyKey: absentKey,
      antiForgery: absentChallenge.nonce,
      request: absentRequest,
    });
    await assert.rejects(
      retryAbsent,
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 409 && problemCode(error) === "stale_assistance_revision";
      },
    );
    const absentBytes = responses.at(-1);

    const initialized = await prepareAssistance(
      started.baseUrl,
      captureFetch,
      first.projectId,
      id(`${ns}0a22`),
      assistanceRequest("available", "0", "018f0000-0000-7001-8000-000000000a23"),
    );
    const initializedBytes = responses.at(-1);
    assert.equal(initialized.updated.effect.kind, "initialized");
    if (initialized.updated.effect.kind !== "initialized") throw new Error("expected initialized");
    assert.equal(initialized.updated.effect.availability, "available");
    assert.equal(initialized.updated.effect.revision, "1");
    assert.equal(initialized.updated.assistance.model_registration_revision, HOST_FAKE_MODEL_REGISTRATION);
    assert.match(initialized.updated.assistance.processing_destination_identity, UUID_V7);
    assert.equal(initialized.updated.assistance.processing_destination_identity_evidence_revision, "1");
    const firstBinding = initialized.updated.assistance;

    const replay = await updateProjectAssistance({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: id(`${ns}0a22`),
      antiForgery: initialized.nonce,
      request: assistanceRequest("available", "0", "018f0000-0000-7001-8000-000000000a23"),
    });
    assert.deepEqual(replay, initialized.updated);

    const queried = await getProjectAssistance({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.deepEqual(queried.assistance, firstBinding);

    const unchanged = await prepareAssistance(
      started.baseUrl,
      captureFetch,
      first.projectId,
      id(`${ns}0a24`),
      assistanceRequest("available", "1", "018f0000-0000-7001-8000-000000000a25"),
    );
    const unchangedBytes = responses.at(-1);
    assert.equal(unchanged.updated.effect.kind, "no_effect");
    assert.deepEqual(unchanged.updated.assistance, firstBinding);

    const stale = await prepareAssistance(
      started.baseUrl,
      captureFetch,
      first.projectId,
      id(`${ns}0a26`),
      assistanceRequest("unavailable", "0", "018f0000-0000-7001-8000-000000000a27"),
    );
    const staleBytes = responses.at(-1);
    assert.equal(stale.updated.effect.kind, "conflicted");
    assert.deepEqual(stale.updated.assistance, firstBinding);

    const toggled = await prepareAssistance(
      started.baseUrl,
      captureFetch,
      first.projectId,
      id(`${ns}0a28`),
      assistanceRequest("unavailable", "1", "018f0000-0000-7001-8000-000000000a29"),
    );
    assert.equal(toggled.updated.effect.kind, "authoritative_applied");
    if (toggled.updated.effect.kind !== "authoritative_applied") throw new Error("expected applied");
    assert.equal(toggled.updated.effect.availability, "unavailable");
    assert.equal(toggled.updated.effect.revision, "2");
    assert.equal(toggled.updated.assistance.processing_destination_identity, firstBinding.processing_destination_identity);
    assert.equal(toggled.updated.assistance.project_model_use_binding_revision, firstBinding.project_model_use_binding_revision);
    assert.equal(toggled.updated.assistance.external_compatibility_decision, firstBinding.external_compatibility_decision);

    const afterToggle = await getProjectAssistance({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(afterToggle.assistance.availability, "unavailable");
    assert.deepEqual(afterToggle.assistance, toggled.updated.assistance);

    await assert.rejects(retryAbsent, (error) => problemCode(error) === "stale_assistance_revision");
    assert.deepEqual(responses.at(-1), absentBytes);

    for (const [original, originalBytes, nonce, key, request] of [
      [initialized.updated, initializedBytes, initialized.nonce, id(`${ns}0a22`), assistanceRequest("available", "0", "018f0000-0000-7001-8000-000000000a23")],
      [unchanged.updated, unchangedBytes, unchanged.nonce, id(`${ns}0a24`), assistanceRequest("available", "1", "018f0000-0000-7001-8000-000000000a25")],
      [stale.updated, staleBytes, stale.nonce, id(`${ns}0a26`), assistanceRequest("unavailable", "0", "018f0000-0000-7001-8000-000000000a27")],
    ] as const) {
      const replay = await updateProjectAssistance({
        baseUrl: started.baseUrl,
        projectId: first.projectId,
        fetchImpl: captureFetch,
        idempotencyKey: key,
        antiForgery: nonce,
        request,
      });
      assert.deepEqual(replay, original);
      assert.deepEqual(responses.at(-1), originalBytes);
    }

    const bound = await createEmpty(started.baseUrl, "session-a", id(`${ns}0a16`), "Bound Novel", id("a10"));
    const boundInit = await prepareAssistance(
      started.baseUrl,
      bound.fetchImpl,
      bound.projectId,
      id(`${ns}0a15`),
      assistanceRequest("available", "0", "018f0000-0000-7001-8000-000000000a14"),
    );
    assert.equal(boundInit.updated.assistance.availability, "available");

    await stopRealServer(started.server);
    started = await startServer({ modelDestination: null });
    const restartedFetch = browserFetch(started.baseUrl, "session-a");
    const restartedResponse = await updateProjectAssistance({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: async (input, init) => {
        const response = await restartedFetch(input, init);
        if (init?.method === "PUT") responses.push(Buffer.from(await response.clone().arrayBuffer()));
        return response;
      },
      idempotencyKey: id(`${ns}0a22`),
      antiForgery: initialized.nonce,
      request: assistanceRequest("available", "0", "018f0000-0000-7001-8000-000000000a23"),
    });
    assert.deepEqual(restartedResponse, initialized.updated);
    assert.deepEqual(responses.at(-1), initializedBytes);
    assert.deepEqual((await getProjectAssistance({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: restartedFetch,
    })).assistance, toggled.updated.assistance);
    assert.deepEqual((await getProjectAssistance({
      baseUrl: started.baseUrl,
      projectId: bound.projectId,
      fetchImpl: restartedFetch,
    })).assistance, { ...boundInit.updated.assistance, availability: "unavailable" });
    const unconfigured = await createEmpty(started.baseUrl, "session-a", id(`${ns}0a19`), "Unconfigured Novel", id("a10"));
    await assert.rejects(
      () => getProjectAssistance({ baseUrl: started.baseUrl, projectId: unconfigured.projectId, fetchImpl: unconfigured.fetchImpl }),
      (error) => requireStoryOSProtocolError(error).status === 404 && problemCode(error) === "resource_unavailable",
    );
    await assert.rejects(
      () => prepareAssistance(
        started.baseUrl,
        unconfigured.fetchImpl,
        unconfigured.projectId,
        id(`${ns}0a18`),
        assistanceRequest("available", "0", "018f0000-0000-7001-8000-000000000a17"),
      ),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );

    await stopRealServer(started.server);
    started = await startServer();

    const stillOpen = await getProject({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: browserFetch(started.baseUrl, "session-a"),
    });
    assert.equal(stillOpen.project.title, "Assistance Novel");

    const other = await createEmpty(started.baseUrl, "session-a", id(`${ns}0a2a`), "Second Novel", id("a10"));
    const otherInit = await prepareAssistance(
      started.baseUrl,
      other.fetchImpl,
      other.projectId,
      id(`${ns}0a2b`),
      assistanceRequest("available", "0", "018f0000-0000-7001-8000-000000000a2c"),
    );
    assert.equal(otherInit.updated.assistance.model_registration_revision, HOST_FAKE_MODEL_REGISTRATION);
    assert.notEqual(
      otherInit.updated.assistance.processing_destination_identity,
      firstBinding.processing_destination_identity,
    );

    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    await assert.rejects(
      () => getProjectAssistance({ baseUrl: started.baseUrl, projectId: first.projectId, fetchImpl: foreignFetch }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    await assert.rejects(
      () => prepareAssistance(
        started.baseUrl,
        foreignFetch,
        first.projectId,
        id(`${ns}0a2d`),
        assistanceRequest("available", "2", "018f0000-0000-7001-8000-000000000a2e"),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );

    const policyCount = await queryPostgres(`
      SELECT count(*)::text FROM storyos.project_policy_revisions
       WHERE project_id = '${first.projectId}'::uuid;
    `);
    assert.equal(policyCount, "2");
  } finally {
    await stopRealServer(started.server);
  }
});

test("createAgentRun exact retry replays in a deployment that offers no model destination", async () => {
  const ns = freshNamespace();
  let started = await startServer();
  try {
    const first = await prepare(started.baseUrl, id(`${ns}e1a0`), "No Destination Novel", `${ns}e1b`);
    const { chapterId } = first;
    const request = runRequest(chapterId, id(`${ns}e1a2`), "Help with this passage.");
    const created = await postRunWithChallenge(started.baseUrl, first.fetchImpl, first.projectId, id(`${ns}e1a3`), request);

    await stopRealServer(started.server);
    started = await startServer({ modelDestination: null });
    const restartedFetch = browserFetch(started.baseUrl, "session-a");
    const replay = await createAgentRun({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: restartedFetch,
      idempotencyKey: id(`${ns}e1a3`),
      antiForgery: created.nonce,
      request,
    });
    assert.deepEqual(replay, created.admitted);
    await assert.rejects(
      () => postRunWithChallenge(
        started.baseUrl,
        restartedFetch,
        first.projectId,
        id(`${ns}e1a4`),
        runRequest(chapterId, id(`${ns}e1a5`), "Help with this passage."),
      ),
      (error) => requireStoryOSProtocolError(error).status === 422
        && problemCode(error) === "assistance_unavailable",
    );
  } finally {
    await stopRealServer(started.server);
  }
});
