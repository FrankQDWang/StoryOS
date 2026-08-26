import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  archiveProject,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  digestArchiveProject,
  digestCreateVolume,
  getManuscriptTree,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
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
const serverBinary = join(repositoryRoot, "target", "debug", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
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
      correlation_id: "018f0000-0000-7001-8000-000000000830",
    },
    idempotency_key: idempotencyKey,
  };
}

function volumeRequest(title: string, expectedTreeRevision: string, correlationId: string): CreateVolumeRequest {
  return {
    command_schema: "storyos.command.create-volume.request.v1",
    create_volume_input: {
      title,
      expected_tree_revision: expectedTreeRevision,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: correlationId,
    },
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
  await createProject({
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
  return { fetchImpl, projectId: challenge.prospective_project_id };
}

async function postVolume(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  idempotencyKey: string,
  request: CreateVolumeRequest,
) {
  const digest = await digestCreateVolume(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/volumes",
      command_schema: "storyos.command.create-volume.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const created = await createVolume({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, created };
}

test("createVolume creates one named Volume, replays, and fails closed", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000831", "Empty Novel");
    const request = volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000841");
    const applied = await postVolume(baseUrl, first.fetchImpl, first.projectId, "018f0000-0000-7001-8000-000000000851", request);
    assert.equal(applied.created.schema_id, "storyos.command.create-volume.response.v1");
    assert.equal(applied.created.receipt.command_kind, "createVolume");
    assert.equal(applied.created.receipt.result, "authoritative_applied");
    assert.equal(applied.created.effect.kind, "authoritative_applied");
    if (applied.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    assert.equal(applied.created.effect.title, "Volume A");
    assert.equal(applied.created.effect.tree_revision, "2");
    assert.equal(applied.created.effect.order, "1");
    assert.match(applied.created.effect.volume_id, UUID_V7);
    assert.match(applied.created.effect.project_activity_position, /^[1-9][0-9]*$/);

    const replay = await createVolume({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000851",
      antiForgery: applied.challenge.nonce,
      request,
    });
    assert.equal(replay.command_id, applied.created.command_id);
    assert.equal(replay.receipt.receipt_id, applied.created.receipt.receipt_id);

    const tree = await getManuscriptTree({ baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl });
    assert.equal(tree.tree_revision, "2");
    assert.equal(tree.volumes.length, 1);
    assert.equal(tree.volumes[0]?.title, "Volume A");
    assert.equal(tree.volumes[0]?.chapters.length, 0);

    const stale = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000852",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000842"),
    );
    assert.equal(stale.created.receipt.result, "conflicted");
    assert.equal(stale.created.effect.kind, "conflicted");
    if (stale.created.effect.kind !== "conflicted") {
      throw new Error("stale Create Volume must conflict");
    }
    assert.equal(stale.created.effect.reason, "stale_tree_revision");
    const afterStale = await getManuscriptTree({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(afterStale.tree_revision, "2");
    assert.equal(afterStale.volumes.length, 1);

    await assert.rejects(
      createVolume({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000854",
        antiForgery: applied.challenge.nonce,
        request: volumeRequest("Changed Retry", "2", "018f0000-0000-7001-8000-000000000844"),
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );

    await assert.rejects(
      postVolume(
        baseUrl,
        first.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000855",
        volumeRequest("", "2", "018f0000-0000-7001-8000-000000000845"),
      ),
      (error) => requireStoryOSProtocolError(error).status === 400,
    );

    const archiveDigest = await digestArchiveProject(archiveRequest("1", "018f0000-0000-7001-8000-000000000846"));
    const archiveChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: archiveDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000856",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000856",
      antiForgery: archiveChallenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-000000000846"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");

    const refused = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000857",
      volumeRequest("Volume A", "2", "018f0000-0000-7001-8000-000000000847"),
    );
    assert.equal(refused.created.receipt.result, "refused");
    assert.equal(refused.created.effect.kind, "refused");
    if (refused.created.effect.kind !== "refused") {
      throw new Error("Create Volume on an archived Project must refuse");
    }
    assert.equal(refused.created.effect.reason, "archived_project");

    const foreign = await createEmpty(baseUrl, "session-b", "018f0000-0000-7001-8000-000000000832", "Other Novel");
    await assert.rejects(
      postVolume(
        baseUrl,
        foreign.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000853",
        volumeRequest("Stolen Volume", "2", "018f0000-0000-7001-8000-000000000843"),
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
