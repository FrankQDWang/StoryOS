// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/update-project-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  digestUpdateProjectAssistance,
  getProject,
  getProjectAssistance,
  updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateProjectChallengeRequest,
  UpdateProjectAssistanceRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeRetry,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const HOST_FAKE_MODEL_REGISTRATION = "018f0000-0000-7001-8000-00000000fa01";

function createChallengeRequest(idempotencyKey: string, title: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000a10",
    },
    idempotency_key: idempotencyKey,
  };
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
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: correlationId,
    },
  };
}

async function startRealServer() {
  return startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A, "session-b": USER_B },
  });
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

async function createEmpty(baseUrl: string, session: string, idempotencyKey: string, title: string) {
  const fetchImpl = browserFetch(baseUrl, session);
  const request = createChallengeRequest(idempotencyKey, title);
  const challenge = await createProjectChallenge({ baseUrl, request, fetchImpl });
  const created = await createProject({
    baseUrl,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request: {
      command_schema: request.command_schema,
      prospective_project_id: challenge.prospective_project_id,
      create_project_input: request.create_project_input,
    },
  });
  return { created, fetchImpl, projectId: challenge.prospective_project_id };
}

async function prepare(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  idempotencyKey: string,
  request: UpdateProjectAssistanceRequest,
) {
  const digest = await digestUpdateProjectAssistance(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "PUT",
      route_template: "/api/v1/projects/{project_id}/assistance",
      command_schema: "storyos.command.update-project-assistance.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const updated = await updateProjectAssistance({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, updated };
}

test("project assistance prepares the host fake binding without a run", async () => {
  let started = await startRealServer();
  try {
    const first = await createEmpty(started.baseUrl, "session-a", "018f0000-0000-7001-8000-000000000a21", "Assistance Novel");
    const responses: Buffer[] = [];
    const captureFetch: typeof fetch = async (input, init) => {
      const response = await first.fetchImpl(input, init);
      if (init?.method === "PUT") responses.push(Buffer.from(await response.clone().arrayBuffer()));
      return response;
    };
    await assert.rejects(
      () => getProjectAssistance({ baseUrl: started.baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl }),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );
    const absentRequest = assistanceRequest("available", "1", "018f0000-0000-7001-8000-000000000a2f");
    const absentKey = "018f0000-0000-7001-8000-000000000a20";
    const absentDigest = await digestUpdateProjectAssistance(absentRequest);
    const absentChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: captureFetch,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/assistance",
        command_schema: absentRequest.command_schema,
        canonical_command_digest: absentDigest,
        idempotency_key: absentKey,
      },
    }));
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

    const initialized = await prepare(
      started.baseUrl,
      captureFetch,
      first.projectId,
      "018f0000-0000-7001-8000-000000000a22",
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
      idempotencyKey: "018f0000-0000-7001-8000-000000000a22",
      antiForgery: initialized.challenge.nonce,
      request: assistanceRequest("available", "0", "018f0000-0000-7001-8000-000000000a23"),
    });
    assert.deepEqual(replay, initialized.updated);

    const queried = await getProjectAssistance({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.deepEqual(queried.assistance, firstBinding);

    const unchanged = await prepare(
      started.baseUrl,
      captureFetch,
      first.projectId,
      "018f0000-0000-7001-8000-000000000a24",
      assistanceRequest("available", "1", "018f0000-0000-7001-8000-000000000a25"),
    );
    const unchangedBytes = responses.at(-1);
    assert.equal(unchanged.updated.effect.kind, "no_effect");
    assert.deepEqual(unchanged.updated.assistance, firstBinding);

    const stale = await prepare(
      started.baseUrl,
      captureFetch,
      first.projectId,
      "018f0000-0000-7001-8000-000000000a26",
      assistanceRequest("unavailable", "0", "018f0000-0000-7001-8000-000000000a27"),
    );
    const staleBytes = responses.at(-1);
    assert.equal(stale.updated.effect.kind, "conflicted");
    assert.deepEqual(stale.updated.assistance, firstBinding);

    const toggled = await prepare(
      started.baseUrl,
      captureFetch,
      first.projectId,
      "018f0000-0000-7001-8000-000000000a28",
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

    const counts = () => queryPostgres(`
      SELECT json_build_object(
        'commands', (SELECT count(*) FROM storyos.command_idempotency WHERE project_id = '${first.projectId}'::uuid),
        'receipts', (SELECT count(*) FROM storyos.domain_receipts WHERE project_id = '${first.projectId}'::uuid),
        'activities', (SELECT count(*) FROM storyos.project_activity_events WHERE project_id = '${first.projectId}'::uuid),
        'policies', (SELECT count(*) FROM storyos.project_policy_revisions WHERE project_id = '${first.projectId}'::uuid)
      )::text;
    `);
    const beforeRetries = await counts();
    for (const [original, originalBytes, challenge, key, request] of [
      [initialized.updated, initializedBytes, initialized.challenge, "018f0000-0000-7001-8000-000000000a22", assistanceRequest("available", "0", "018f0000-0000-7001-8000-000000000a23")],
      [unchanged.updated, unchangedBytes, unchanged.challenge, "018f0000-0000-7001-8000-000000000a24", assistanceRequest("available", "1", "018f0000-0000-7001-8000-000000000a25")],
      [stale.updated, staleBytes, stale.challenge, "018f0000-0000-7001-8000-000000000a26", assistanceRequest("unavailable", "0", "018f0000-0000-7001-8000-000000000a27")],
    ] as const) {
      const replay = await updateProjectAssistance({
        baseUrl: started.baseUrl,
        projectId: first.projectId,
        fetchImpl: captureFetch,
        idempotencyKey: key,
        antiForgery: challenge.nonce,
        request,
      });
      assert.deepEqual(replay, original);
      assert.deepEqual(responses.at(-1), originalBytes);
    }
    assert.equal(await counts(), beforeRetries);

    await stopRealServer(started.server);
    started = await startRealServer();
    const restartedFetch = browserFetch(started.baseUrl, "session-a");
    const restartedResponse = await updateProjectAssistance({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: async (input, init) => {
        const response = await restartedFetch(input, init);
        if (init?.method === "PUT") responses.push(Buffer.from(await response.clone().arrayBuffer()));
        return response;
      },
      idempotencyKey: "018f0000-0000-7001-8000-000000000a22",
      antiForgery: initialized.challenge.nonce,
      request: assistanceRequest("available", "0", "018f0000-0000-7001-8000-000000000a23"),
    });
    assert.deepEqual(restartedResponse, initialized.updated);
    assert.deepEqual(responses.at(-1), initializedBytes);
    assert.equal(await counts(), beforeRetries);
    assert.deepEqual((await getProjectAssistance({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: restartedFetch,
    })).assistance, toggled.updated.assistance);

    const stillOpen = await getProject({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: restartedFetch,
    });
    assert.equal(stillOpen.project.title, "Assistance Novel");

    const other = await createEmpty(started.baseUrl, "session-a", "018f0000-0000-7001-8000-000000000a2a", "Second Novel");
    const otherInit = await prepare(
      started.baseUrl,
      other.fetchImpl,
      other.projectId,
      "018f0000-0000-7001-8000-000000000a2b",
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
      () => prepare(
        started.baseUrl,
        foreignFetch,
        first.projectId,
        "018f0000-0000-7001-8000-000000000a2d",
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
