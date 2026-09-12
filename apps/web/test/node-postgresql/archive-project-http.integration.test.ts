import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  archiveProject,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  digestArchiveProject,
  digestUpdateProject,
  listProjects,
  updateProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateProjectChallengeRequest,
  UpdateProjectRequest,
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

function createChallengeRequest(idempotencyKey: string, title: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000910",
    },
    idempotency_key: idempotencyKey,
  };
}

function archiveRequest(expectedProjectRevision: string, correlationId: string): ArchiveProjectRequest {
  return {
    command_schema: "storyos.command.archive-project.request.v1",
    archive_project_input: {
      expected_project_revision: expectedProjectRevision,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: correlationId,
    },
  };
}

function renameRequest(title: string, expectedProjectRevision: string, correlationId: string): UpdateProjectRequest {
  return {
    command_schema: "storyos.command.update-project.request.v1",
    update_project_input: {
      title,
      expected_project_revision: expectedProjectRevision,
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

function capturingPut(inner: typeof fetch): { fetchImpl: typeof fetch; lastPutBody: () => string } {
  let lastPutBody = "";
  return {
    fetchImpl: async (input, init) => {
      const response = await inner(input, init);
      const url = String(input instanceof Request ? input.url : input);
      if (response.ok && (init?.method ?? "GET") === "PUT" && url.includes("/archival")) {
        lastPutBody = await response.clone().text();
      }
      return response;
    },
    lastPutBody: () => lastPutBody,
  };
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

async function archive(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  idempotencyKey: string,
  request: ArchiveProjectRequest,
) {
  const digest = await digestArchiveProject(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "PUT",
      route_template: "/api/v1/projects/{project_id}/archival",
      command_schema: "storyos.command.archive-project.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const archived = await archiveProject({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, archived };
}

test("archiveProject settles lifecycle, replays, lists archived, and fails closed", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000901", "Empty Novel");
    const request = archiveRequest("1", "018f0000-0000-7001-8000-000000000911");
    const applied = await archive(baseUrl, first.fetchImpl, first.projectId, "018f0000-0000-7001-8000-000000000921", request);
    assert.equal(applied.archived.schema_id, "storyos.command.archive-project.response.v1");
    assert.equal(applied.archived.receipt.command_kind, "archiveProject");
    assert.equal(applied.archived.receipt.result, "authoritative_applied");
    assert.equal(applied.archived.effect.kind, "authoritative_applied");
    assert.match(applied.archived.command_id, UUID_V7);
    if (applied.archived.effect.kind === "authoritative_applied") {
      assert.equal(applied.archived.effect.revision, "2");
    }

    const replay = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000921",
      antiForgery: applied.challenge.nonce,
      request,
    });
    assert.equal(replay.command_id, applied.archived.command_id);
    assert.equal(replay.receipt.receipt_id, applied.archived.receipt.receipt_id);

    const listed = await listProjects({ baseUrl, fetchImpl: first.fetchImpl });
    const listedItem = listed.projects.find((item) => item.project_scope.project_id === first.projectId);
    assert.equal(listedItem?.lifecycle.kind, "archived");
    assert.equal(listedItem?.revision, "2");

    const already = await archive(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000922",
      archiveRequest("2", "018f0000-0000-7001-8000-000000000912"),
    );
    assert.equal(already.archived.receipt.result, "no_effect");
    assert.equal(already.archived.effect.kind, "no_effect");

    const stale = await archive(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000923",
      archiveRequest("1", "018f0000-0000-7001-8000-000000000913"),
    );
    assert.equal(stale.archived.receipt.result, "conflicted");
    assert.equal(stale.archived.effect.kind, "conflicted");

    await assert.rejects(
      archiveProject({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000921",
        antiForgery: applied.challenge.nonce,
        request: archiveRequest("2", "018f0000-0000-7001-8000-000000000911"),
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );

    const blockedRename = renameRequest("Stolen Title", "2", "018f0000-0000-7001-8000-000000000914");
    const blockedDigest = await digestUpdateProject(blockedRename);
    const blockedChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PATCH",
        route_template: "/api/v1/projects/{project_id}",
        command_schema: "storyos.command.update-project.request.v1",
        canonical_command_digest: blockedDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000924",
      },
    }));
    await assert.rejects(
      updateProject({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000924",
        antiForgery: blockedChallenge.nonce,
        request: blockedRename,
      }),
      (error) => requireStoryOSProtocolError(error).status === 409,
    );

    const foreign = await createEmpty(baseUrl, "session-b", "018f0000-0000-7001-8000-000000000902", "Other Novel");
    await assert.rejects(
      archive(
        baseUrl,
        foreign.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000925",
        archiveRequest("2", "018f0000-0000-7001-8000-000000000915"),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );

    const second = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000903", "Ack Novel");
    const lostRequest = archiveRequest("1", "018f0000-0000-7001-8000-000000000916");
    const lostKey = "018f0000-0000-7001-8000-000000000926";
    const lostDigest = await digestArchiveProject(lostRequest);
    const lostChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: second.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: lostDigest,
        idempotency_key: lostKey,
      },
    }));
    await assert.rejects(
      archiveProject({
        baseUrl,
        projectId: second.projectId,
        fetchImpl: async (input, init) => {
          const response = await first.fetchImpl(input, init);
          await response.arrayBuffer();
          throw new Error("simulated acknowledgement delivery loss");
        },
        idempotencyKey: lostKey,
        antiForgery: lostChallenge.nonce,
        request: lostRequest,
      }),
      /simulated acknowledgement delivery loss/,
    );
    const recovered = await archiveProject({
      baseUrl,
      projectId: second.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: lostKey,
      antiForgery: lostChallenge.nonce,
      request: lostRequest,
    });
    assert.equal(recovered.receipt.result, "authoritative_applied");
    assert.equal(recovered.effect.kind, "authoritative_applied");
  } finally {
    await stopRealServer(server);
  }
});

test("archiveProject freezes the acknowledgement and does not archive twice", async () => {
  let { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000930", "Archive Novel");
    const request = archiveRequest("1", "018f0000-0000-7001-8000-000000000931");
    const firstCapture = capturingPut(first.fetchImpl);
    const applied = await archive(
      baseUrl,
      firstCapture.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000932",
      request,
    );
    const firstBody = firstCapture.lastPutBody();
    assert.equal(applied.archived.receipt.result, "authoritative_applied");
    assert.equal(JSON.parse(firstBody).project.title, "Archive Novel");
    assert.equal(JSON.parse(firstBody).project.open.kind, "empty");

    const alreadyRequest = archiveRequest("2", "018f0000-0000-7001-8000-000000000933");
    const alreadyCapture = capturingPut(first.fetchImpl);
    const already = await archive(
      baseUrl,
      alreadyCapture.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000934",
      alreadyRequest,
    );
    const alreadyBody = alreadyCapture.lastPutBody();
    assert.equal(already.archived.receipt.result, "no_effect");

    const staleRequest = archiveRequest("1", "018f0000-0000-7001-8000-000000000935");
    const staleCapture = capturingPut(first.fetchImpl);
    const stale = await archive(
      baseUrl,
      staleCapture.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000936",
      staleRequest,
    );
    const staleBody = staleCapture.lastPutBody();
    assert.equal(stale.archived.receipt.result, "conflicted");

    const frozenCapture = capturingPut(first.fetchImpl);
    const frozen = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: frozenCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000932",
      antiForgery: applied.challenge.nonce,
      request,
    });
    assert.deepEqual(frozen, applied.archived);
    assert.equal(frozenCapture.lastPutBody(), firstBody);

    const frozenAlreadyCapture = capturingPut(first.fetchImpl);
    const frozenAlready = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: frozenAlreadyCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000934",
      antiForgery: already.challenge.nonce,
      request: alreadyRequest,
    });
    assert.deepEqual(frozenAlready, already.archived);
    assert.equal(frozenAlreadyCapture.lastPutBody(), alreadyBody);

    const frozenStaleCapture = capturingPut(first.fetchImpl);
    const frozenStale = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: frozenStaleCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000936",
      antiForgery: stale.challenge.nonce,
      request: staleRequest,
    });
    assert.deepEqual(frozenStale, stale.archived);
    assert.equal(frozenStaleCapture.lastPutBody(), staleBody);

    assert.equal(
      await queryPostgres(`
        SELECT lifecycle_state || ' ' || count(*)::text FROM storyos.projects
         WHERE project_id = '${first.projectId}'::uuid GROUP BY lifecycle_state;
      `),
      "archived 1",
    );
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.project_archival_decisions
         WHERE project_id = '${first.projectId}'::uuid;
      `),
      "1",
    );
    const receiptCount = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'archiveProject';
    `);
    assert.equal(receiptCount, "3");
    await stopRealServer(server);
    ({ baseUrl, server } = await startRealServer());
    const restartedFetch = browserFetch(baseUrl, "session-a");
    const afterRestartCapture = capturingPut(restartedFetch);
    const afterRestart = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: afterRestartCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000932",
      antiForgery: applied.challenge.nonce,
      request,
    });
    assert.deepEqual(afterRestart, applied.archived);
    assert.equal(afterRestartCapture.lastPutBody(), firstBody);
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'archiveProject';
      `),
      receiptCount,
    );
  } finally {
    await stopRealServer(server);
  }
});

test("archiveProject distinguishes historical absence from damaged new-format evidence", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000937", "Archive Novel");
    const request = archiveRequest("1", "018f0000-0000-7001-8000-000000000938");
    const applied = await archive(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000939",
      request,
    );
    assert.equal(applied.archived.project.title, "Archive Novel");
    const novelsBefore = await queryPostgres(`
      SELECT title || ' ' || lifecycle_state || ' ' || count(*)::text FROM storyos.projects
       WHERE project_id = '${first.projectId}'::uuid GROUP BY title, lifecycle_state;
    `);
    const receiptsBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${first.projectId}'::uuid;
    `);
    const keysBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.command_idempotency
       WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'archiveProject';
    `);
    const decisionsBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.project_archival_decisions
       WHERE project_id = '${first.projectId}'::uuid;
    `);
    await queryPostgres(`
      UPDATE storyos.command_idempotency
         SET acknowledgement_format = NULL, response_project = NULL
       WHERE project_id = '${first.projectId}'::uuid
         AND idempotency_key = '018f0000-0000-7001-8000-000000000939'::uuid;
    `);
    await assert.rejects(
      archiveProject({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000939",
        antiForgery: applied.challenge.nonce,
        request,
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 409
          && problemCode(error) === "historical_acknowledgement_unavailable";
      },
    );
    await queryPostgres(`
      UPDATE storyos.command_idempotency
         SET acknowledgement_format = 'command_response_project.v1',
             response_project = '{"broken":true}'::jsonb
       WHERE project_id = '${first.projectId}'::uuid
         AND idempotency_key = '018f0000-0000-7001-8000-000000000939'::uuid;
    `);
    await assert.rejects(
      archiveProject({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000939",
        antiForgery: applied.challenge.nonce,
        request,
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 503
          && problemCode(error) !== "historical_acknowledgement_unavailable";
      },
    );
    assert.equal(
      await queryPostgres(`
        SELECT title || ' ' || lifecycle_state || ' ' || count(*)::text FROM storyos.projects
         WHERE project_id = '${first.projectId}'::uuid GROUP BY title, lifecycle_state;
      `),
      novelsBefore,
    );
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${first.projectId}'::uuid;
      `),
      receiptsBefore,
    );
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.command_idempotency
         WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'archiveProject';
      `),
      keysBefore,
    );
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.project_archival_decisions
         WHERE project_id = '${first.projectId}'::uuid;
      `),
      decisionsBefore,
    );
  } finally {
    await stopRealServer(server);
  }
});
