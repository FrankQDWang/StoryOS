import assert from "node:assert/strict";
import { unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createChapter,
  createEditorSession,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  digestCreateChapter,
  digestCreateEditorSession,
  digestCreateVolume,
  digestSetCurrentChapter,
  digestUpdateProject,
  getChapter,
  getProject,
  listProjects,
  setCurrentChapter,
  updateProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateChapterRequest,
  CreateEditorSessionRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  DigestValue,
  SetCurrentChapterRequest,
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
const CLIENT = RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision;
const SECURITY = "storyos.web-security-policy.release-1.v1";

function createChallengeRequest(idempotencyKey: string, title: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000810",
    },
    idempotency_key: idempotencyKey,
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

async function startRealServer(extraEnv?: Readonly<Record<string, string>>) {
  return extraEnv === undefined
    ? startStoryOSServer({
        repositoryRoot,
        serverBinary,
        sessions: { "session-a": USER_A, "session-b": USER_B },
      })
    : startStoryOSServer({
        repositoryRoot,
        serverBinary,
        sessions: { "session-a": USER_A, "session-b": USER_B },
        extraEnv,
      });
}

function capturingPatch(inner: typeof fetch): { fetchImpl: typeof fetch; lastPatchBody: () => string } {
  let lastPatchBody = "";
  return {
    fetchImpl: async (input, init) => {
      const response = await inner(input, init);
      if (response.ok && (init?.method ?? "GET") === "PATCH") {
        lastPatchBody = await response.clone().text();
      }
      return response;
    },
    lastPatchBody: () => lastPatchBody,
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

async function waitForSettledKey(projectId: string, idempotencyKey: string) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    const outcome = await queryPostgres(`
      SELECT outcome_kind FROM storyos.command_idempotency
       WHERE project_id = '${projectId}'::uuid
         AND command_kind = 'updateProject'
         AND idempotency_key = '${idempotencyKey}'::uuid;
    `);
    if (outcome === "settled") return;
    await new Promise((resolve) => {
      setTimeout(resolve, 10);
    });
  }
  throw new Error(`Update Project ${idempotencyKey} did not settle`);
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

async function rename(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  idempotencyKey: string,
  request: UpdateProjectRequest,
) {
  const digest = await digestUpdateProject(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "PATCH",
      route_template: "/api/v1/projects/{project_id}",
      command_schema: "storyos.command.update-project.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const updated = await updateProject({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, updated };
}

async function challenged<T>(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  idempotencyKey: string,
  method: string,
  routeTemplate: string,
  commandSchema: string,
  digest: DigestValue,
  submit: (nonce: string) => Promise<T>,
): Promise<T> {
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method,
      route_template: routeTemplate,
      command_schema: commandSchema,
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  return submit(challenge.nonce);
}

test("updateProject renames one exact Project, replays, and fails closed", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000801", "Empty Novel");
    const request = renameRequest("Renamed Novel", "1", "018f0000-0000-7001-8000-000000000811");
    const applied = await rename(baseUrl, first.fetchImpl, first.projectId, "018f0000-0000-7001-8000-000000000821", request);
    assert.equal(applied.updated.schema_id, "storyos.command.update-project.response.v1");
    assert.equal(applied.updated.receipt.command_kind, "updateProject");
    assert.equal(applied.updated.receipt.result, "authoritative_applied");
    assert.equal(applied.updated.effect.kind, "authoritative_applied");
    assert.equal(applied.updated.project.title, "Renamed Novel");
    assert.match(applied.updated.command_id, UUID_V7);

    const replay = await updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000821",
      antiForgery: applied.challenge.nonce,
      request,
    });
    assert.equal(replay.command_id, applied.updated.command_id);
    assert.equal(replay.receipt.receipt_id, applied.updated.receipt.receipt_id);

    const opened = await getProject({ baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl });
    assert.equal(opened.project.title, "Renamed Novel");
    const listed = await listProjects({ baseUrl, fetchImpl: first.fetchImpl });
    const listedItem = listed.projects.find((item) => item.project_scope.project_id === first.projectId);
    assert.equal(listedItem?.title, "Renamed Novel");
    assert.equal(listedItem?.revision, "2");

    const unchanged = await rename(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000822",
      renameRequest("Renamed Novel", "2", "018f0000-0000-7001-8000-000000000812"),
    );
    assert.equal(unchanged.updated.receipt.result, "no_effect");
    assert.equal(unchanged.updated.effect.kind, "no_effect");

    const stale = await rename(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000823",
      renameRequest("Stale Title", "1", "018f0000-0000-7001-8000-000000000813"),
    );
    assert.equal(stale.updated.receipt.result, "conflicted");
    assert.equal(stale.updated.effect.kind, "conflicted");
    const afterStale = await getProject({ baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl });
    assert.equal(afterStale.project.title, "Renamed Novel");

    await assert.rejects(
      updateProject({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000821",
        antiForgery: applied.challenge.nonce,
        request: renameRequest("Changed Retry", "1", "018f0000-0000-7001-8000-000000000811"),
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );

    const foreign = await createEmpty(baseUrl, "session-b", "018f0000-0000-7001-8000-000000000802", "Other Novel");
    await assert.rejects(
      rename(
        baseUrl,
        foreign.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000824",
        renameRequest("Stolen Title", "2", "018f0000-0000-7001-8000-000000000814"),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
  } finally {
    await stopRealServer(server);
  }
});

test("updateProject refuses an invalid title and replays the settled Receipt", async () => {
  let { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000803", "Empty Novel");
    const firstRequest = renameRequest("Renamed Novel", "1", "018f0000-0000-7001-8000-000000000815");
    const firstCapture = capturingPatch(first.fetchImpl);
    const firstRename = await rename(
      baseUrl,
      firstCapture.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000826",
      firstRequest,
    );
    const firstBody = firstCapture.lastPatchBody();
    assert.equal(firstRename.updated.effect.kind, "authoritative_applied");
    assert.equal(JSON.parse(firstBody).project.title, "Renamed Novel");

    await assert.rejects(
      rename(
        baseUrl,
        first.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000827",
        renameRequest("", "2", "018f0000-0000-7001-8000-000000000816"),
      ),
      (error) => requireStoryOSProtocolError(error).status === 400,
    );
    const afterInvalid = await getProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(afterInvalid.project.title, "Renamed Novel");

    const lostRequest = renameRequest("Ack Lost Novel", "2", "018f0000-0000-7001-8000-000000000817");
    const lostKey = "018f0000-0000-7001-8000-000000000828";
    const lostDigest = await digestUpdateProject(lostRequest);
    const lostChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PATCH",
        route_template: "/api/v1/projects/{project_id}",
        command_schema: "storyos.command.update-project.request.v1",
        canonical_command_digest: lostDigest,
        idempotency_key: lostKey,
      },
    }));
    await assert.rejects(
      updateProject({
        baseUrl,
        projectId: first.projectId,
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
    const recovered = await updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: lostKey,
      antiForgery: lostChallenge.nonce,
      request: lostRequest,
    });
    assert.notEqual(recovered.command_id, firstRename.updated.command_id);
    assert.equal(recovered.receipt.result, "authoritative_applied");
    assert.equal(recovered.effect.kind, "authoritative_applied");
    if (recovered.effect.kind !== "authoritative_applied") {
      throw new Error("the recovered rename is not applied");
    }
    assert.equal(recovered.effect.title, "Ack Lost Novel");
    assert.equal(recovered.project.title, "Ack Lost Novel");

    const later = await rename(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000829",
      renameRequest("Later Novel", "3", "018f0000-0000-7001-8000-000000000818"),
    );
    assert.equal(later.updated.project.title, "Later Novel");
    const afterLater = await getProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(afterLater.project.title, "Later Novel");

    const unchangedRequest = renameRequest("Later Novel", "4", "018f0000-0000-7001-8000-000000000819");
    const unchangedCapture = capturingPatch(first.fetchImpl);
    const unchanged = await rename(
      baseUrl,
      unchangedCapture.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000830",
      unchangedRequest,
    );
    const unchangedBody = unchangedCapture.lastPatchBody();
    assert.equal(unchanged.updated.receipt.result, "no_effect");
    assert.equal(unchanged.updated.project.title, "Later Novel");

    const staleRequest = renameRequest("Stale After Later", "1", "018f0000-0000-7001-8000-00000000081a");
    const staleCapture = capturingPatch(first.fetchImpl);
    const stale = await rename(
      baseUrl,
      staleCapture.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000831",
      staleRequest,
    );
    const staleBody = staleCapture.lastPatchBody();
    assert.equal(stale.updated.receipt.result, "conflicted");
    assert.equal(stale.updated.project.title, "Later Novel");

    const third = await rename(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000832",
      renameRequest("Third Novel", "4", "018f0000-0000-7001-8000-00000000081b"),
    );
    assert.equal(third.updated.project.title, "Third Novel");

    const frozenCapture = capturingPatch(first.fetchImpl);
    const frozen = await updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: frozenCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000826",
      antiForgery: firstRename.challenge.nonce,
      request: firstRequest,
    });
    assert.deepEqual(frozen, firstRename.updated);
    assert.equal(frozenCapture.lastPatchBody(), firstBody);
    assert.equal(frozen.project.title, "Renamed Novel");

    const frozenUnchangedCapture = capturingPatch(first.fetchImpl);
    const frozenUnchanged = await updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: frozenUnchangedCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000830",
      antiForgery: unchanged.challenge.nonce,
      request: unchangedRequest,
    });
    assert.deepEqual(frozenUnchanged, unchanged.updated);
    assert.equal(frozenUnchangedCapture.lastPatchBody(), unchangedBody);

    const frozenStaleCapture = capturingPatch(first.fetchImpl);
    const frozenStale = await updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: frozenStaleCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000831",
      antiForgery: stale.challenge.nonce,
      request: staleRequest,
    });
    assert.deepEqual(frozenStale, stale.updated);
    assert.equal(frozenStaleCapture.lastPatchBody(), staleBody);

    const opened = await getProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(opened.project.title, "Third Novel");
    const receiptCount = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'updateProject';
    `);
    await stopRealServer(server);
    ({ baseUrl, server } = await startRealServer());
    const restartedFetch = browserFetch(baseUrl, "session-a");
    const afterRestartCapture = capturingPatch(restartedFetch);
    const afterRestart = await updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: afterRestartCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000826",
      antiForgery: firstRename.challenge.nonce,
      request: firstRequest,
    });
    assert.deepEqual(afterRestart, firstRename.updated);
    assert.equal(afterRestartCapture.lastPatchBody(), firstBody);
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'updateProject';
      `),
      receiptCount,
    );
  } finally {
    await stopRealServer(server);
  }
});

test("updateProject first acknowledgement excludes a later committed rename", async () => {
  const heldKey = "018f0000-0000-7001-8000-000000000840";
  const holdPath = join(tmpdir(), `storyos-ack-hold-${heldKey}.flag`);
  writeFileSync(holdPath, "hold");
  const { baseUrl, server } = await startRealServer({
    STORYOS_TEST_ACK_HOLD_PATH: holdPath,
    STORYOS_TEST_ACK_HOLD_IDEMPOTENCY_KEY: heldKey,
  });
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000804", "Empty Novel");
    const requestA = renameRequest("Held Novel", "1", "018f0000-0000-7001-8000-000000000841");
    const digestA = await digestUpdateProject(requestA);
    const challengeA = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PATCH",
        route_template: "/api/v1/projects/{project_id}",
        command_schema: "storyos.command.update-project.request.v1",
        canonical_command_digest: digestA,
        idempotency_key: heldKey,
      },
    }));
    const firstAck = updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: heldKey,
      antiForgery: challengeA.nonce,
      request: requestA,
    });
    await waitForSettledKey(first.projectId, heldKey);
    const later = await rename(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000842",
      renameRequest("Later Held", "2", "018f0000-0000-7001-8000-000000000843"),
    );
    assert.equal(later.updated.project.title, "Later Held");
    unlinkSync(holdPath);
    const held = await firstAck;
    assert.equal(held.project.title, "Held Novel");
    assert.equal(held.receipt.result, "authoritative_applied");
    const opened = await getProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(opened.project.title, "Later Held");
    const retried = await updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: heldKey,
      antiForgery: challengeA.nonce,
      request: requestA,
    });
    assert.deepEqual(retried, held);
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'updateProject';
      `),
      "2",
    );
  } finally {
    try {
      unlinkSync(holdPath);
    } catch {
      // The race test deletes the hold file on the success path.
    }
    await stopRealServer(server);
  }
});

test("updateProject distinguishes historical absence from damaged new-format evidence", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000805", "Empty Novel");
    const firstRequest = renameRequest("Renamed Novel", "1", "018f0000-0000-7001-8000-000000000850");
    const firstRename = await rename(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000851",
      firstRequest,
    );
    assert.equal(firstRename.updated.project.title, "Renamed Novel");
    const novelsBefore = await queryPostgres(`
      SELECT title || ' ' || count(*)::text FROM storyos.projects
       WHERE project_id = '${first.projectId}'::uuid GROUP BY title;
    `);
    const receiptsBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${first.projectId}'::uuid;
    `);
    const keysBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.command_idempotency
       WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'updateProject';
    `);
    await queryPostgres(`
      UPDATE storyos.command_idempotency
         SET acknowledgement_format = NULL, response_project = NULL
       WHERE project_id = '${first.projectId}'::uuid
         AND idempotency_key = '018f0000-0000-7001-8000-000000000851'::uuid;
    `);
    await assert.rejects(
      updateProject({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000851",
        antiForgery: firstRename.challenge.nonce,
        request: firstRequest,
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
         AND idempotency_key = '018f0000-0000-7001-8000-000000000851'::uuid;
    `);
    await assert.rejects(
      updateProject({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000851",
        antiForgery: firstRename.challenge.nonce,
        request: firstRequest,
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 503
          && problemCode(error) !== "historical_acknowledgement_unavailable";
      },
    );
    assert.equal(
      await queryPostgres(`
        SELECT title || ' ' || count(*)::text FROM storyos.projects
         WHERE project_id = '${first.projectId}'::uuid GROUP BY title;
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
         WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'updateProject';
      `),
      keysBefore,
    );
  } finally {
    await stopRealServer(server);
  }
});

test("updateProject acknowledgement freezes Current Chapter", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000860", "Empty Novel");
    const volumeRequest: CreateVolumeRequest = {
      command_schema: "storyos.command.create-volume.request.v1",
      create_volume_input: {
        title: "Volume A",
        expected_tree_revision: "1",
        client_contract_revision: CLIENT,
        security_policy_revision: SECURITY,
        correlation_id: "018f0000-0000-7001-8000-000000000861",
      },
    };
    const volume = await challenged(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000862",
      "POST",
      "/api/v1/projects/{project_id}/volumes",
      volumeRequest.command_schema,
      await digestCreateVolume(volumeRequest),
      (nonce) => createVolume({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000862",
        antiForgery: nonce,
        request: volumeRequest,
      }),
    );
    assert.equal(volume.effect.kind, "authoritative_applied");
    if (volume.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const volumeId = volume.effect.volume_id;
    const chapterRequest = (title: string, expectedTreeRevision: string, correlationId: string): CreateChapterRequest => ({
      command_schema: "storyos.command.create-chapter.request.v1",
      create_chapter_input: {
        title,
        expected_tree_revision: expectedTreeRevision,
        client_contract_revision: CLIENT,
        security_policy_revision: SECURITY,
        correlation_id: correlationId,
      },
    });
    const postChapter = async (
      request: CreateChapterRequest,
      idempotencyKey: string,
    ) => challenged(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      idempotencyKey,
      "POST",
      "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
      request.command_schema,
      await digestCreateChapter(request),
      (nonce) => createChapter({
        baseUrl,
        projectId: first.projectId,
        volumeId,
        fetchImpl: first.fetchImpl,
        idempotencyKey,
        antiForgery: nonce,
        request,
      }),
    );
    const chapterA = await postChapter(
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-000000000863"),
      "018f0000-0000-7001-8000-000000000864",
    );
    const chapterB = await postChapter(
      chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-000000000865"),
      "018f0000-0000-7001-8000-000000000866",
    );
    assert.equal(chapterA.effect.kind, "authoritative_applied");
    assert.equal(chapterB.effect.kind, "authoritative_applied");
    if (chapterA.effect.kind !== "authoritative_applied"
      || chapterB.effect.kind !== "authoritative_applied") {
      throw new Error("both Chapters must apply");
    }
    const renameKey = "018f0000-0000-7001-8000-000000000867";
    const requestA = renameRequest("Open Novel", "1", "018f0000-0000-7001-8000-000000000868");
    const firstCapture = capturingPatch(first.fetchImpl);
    const firstRename = await rename(baseUrl, firstCapture.fetchImpl, first.projectId, renameKey, requestA);
    const firstBody = firstCapture.lastPatchBody();
    assert.equal(firstRename.updated.project.open.kind, "current_chapter");
    if (firstRename.updated.project.open.kind !== "current_chapter") {
      throw new Error("the captured Project must name Chapter A");
    }
    assert.equal(firstRename.updated.project.open.current_chapter_id, chapterA.effect.chapter_id);
    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
      correlation_id: "018f0000-0000-7001-8000-000000000869",
    };
    const session = await challenged(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000086a",
      "POST",
      "/api/v1/projects/{project_id}/editor-sessions",
      sessionRequest.command_schema,
      await digestCreateEditorSession(sessionRequest),
      (nonce) => createEditorSession({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-00000000086a",
        antiForgery: nonce,
        request: sessionRequest,
      }),
    );
    const openedB = await getChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId: chapterB.effect.chapter_id,
      fetchImpl: first.fetchImpl,
    });
    const switchRequest: SetCurrentChapterRequest = {
      command_schema: "storyos.command.set-current-chapter.request.v1",
      set_current_chapter_input: {
        chapter_id: chapterB.effect.chapter_id,
        expected_current_chapter_id: chapterA.effect.chapter_id,
        expected_target_revision_id: openedB.chapter.current_revision.revision_id,
        editor_session_id: session.editor_session.editor_session_id,
        client_contract_revision: CLIENT,
        security_policy_revision: SECURITY,
        correlation_id: "018f0000-0000-7001-8000-00000000086b",
      },
    };
    await challenged(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000086c",
      "PUT",
      "/api/v1/projects/{project_id}/current-chapter",
      switchRequest.command_schema,
      await digestSetCurrentChapter(switchRequest),
      (nonce) => setCurrentChapter({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-00000000086c",
        antiForgery: nonce,
        request: switchRequest,
      }),
    );
    const opened = await getProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(opened.project.open.kind, "current_chapter");
    if (opened.project.open.kind !== "current_chapter") {
      throw new Error("GET must report Chapter B");
    }
    assert.equal(opened.project.open.current_chapter_id, chapterB.effect.chapter_id);
    const frozenCapture = capturingPatch(first.fetchImpl);
    const frozen = await updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: frozenCapture.fetchImpl,
      idempotencyKey: renameKey,
      antiForgery: firstRename.challenge.nonce,
      request: requestA,
    });
    assert.deepEqual(frozen, firstRename.updated);
    assert.equal(frozenCapture.lastPatchBody(), firstBody);
    assert.equal(frozen.project.open.kind, "current_chapter");
    if (frozen.project.open.kind !== "current_chapter") {
      throw new Error("retry must keep Chapter A");
    }
    assert.equal(frozen.project.open.current_chapter_id, chapterA.effect.chapter_id);
  } finally {
    await stopRealServer(server);
  }
});
