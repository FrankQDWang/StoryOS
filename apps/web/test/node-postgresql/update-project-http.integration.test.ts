import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  digestUpdateProject,
  getProject,
  listProjects,
  updateProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateProjectChallengeRequest,
  UpdateProjectRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
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

async function startRealServer() {
  return startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A, "session-b": USER_B },
  });
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
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000803", "Empty Novel");
    const firstRequest = renameRequest("Renamed Novel", "1", "018f0000-0000-7001-8000-000000000815");
    const firstRename = await rename(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000826",
      firstRequest,
    );
    assert.equal(firstRename.updated.effect.kind, "authoritative_applied");

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

    const frozen = await updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000826",
      antiForgery: firstRename.challenge.nonce,
      request: firstRequest,
    });
    assert.equal(frozen.command_id, firstRename.updated.command_id);
    assert.equal(frozen.receipt.receipt_id, firstRename.updated.receipt.receipt_id);
    assert.equal(frozen.effect.kind, "authoritative_applied");
    if (frozen.effect.kind === "authoritative_applied") {
      assert.equal(frozen.effect.title, "Renamed Novel");
      assert.equal(frozen.effect.revision, "2");
    }
    const opened = await getProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(opened.project.title, "Later Novel");
  } finally {
    await stopRealServer(server);
  }
});
