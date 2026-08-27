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
  digestUpdateVolume,
  getManuscriptTree,
  updateVolume,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  UpdateVolumeRequest,
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
const MISSING_VOLUME = "018f0000-0000-7001-8000-00000000ffff";
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function createChallengeRequest(idempotencyKey: string, title: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000c30",
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

function updateRequest(
  title: string,
  order: string,
  expectedVolumeRevision: string,
  correlationId: string,
): UpdateVolumeRequest {
  return {
    command_schema: "storyos.command.update-volume.request.v1",
    update_volume_input: {
      title,
      order,
      expected_volume_revision: expectedVolumeRevision,
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

async function patchVolume(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  volumeId: string,
  idempotencyKey: string,
  request: UpdateVolumeRequest,
) {
  const digest = await digestUpdateVolume(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "PATCH",
      route_template: "/api/v1/projects/{project_id}/volumes/{volume_id}",
      command_schema: "storyos.command.update-volume.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const updated = await updateVolume({
    baseUrl,
    projectId,
    volumeId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, updated };
}

test("updateVolume renames and reorders one Volume, replays, and fails closed", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000c31", "Empty Novel");
    const volumeA = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000c51",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000c41"),
    );
    const volumeB = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000c52",
      volumeRequest("Volume B", "2", "018f0000-0000-7001-8000-000000000c42"),
    );
    assert.equal(volumeA.created.effect.kind, "authoritative_applied");
    assert.equal(volumeB.created.effect.kind, "authoritative_applied");
    if (volumeA.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume A must apply");
    }
    const volumeId = volumeA.created.effect.volume_id;
    const request = updateRequest("Volume B", "2", "3", "018f0000-0000-7001-8000-000000000c43");
    const applied = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000c53",
      request,
    );
    assert.equal(applied.updated.schema_id, "storyos.command.update-volume.response.v1");
    assert.equal(applied.updated.receipt.command_kind, "updateVolume");
    assert.equal(applied.updated.receipt.result, "authoritative_applied");
    assert.equal(applied.updated.effect.kind, "authoritative_applied");
    if (applied.updated.effect.kind !== "authoritative_applied") {
      throw new Error("Update Volume must apply");
    }
    assert.equal(applied.updated.effect.volume_id, volumeId);
    assert.equal(applied.updated.effect.title, "Volume B");
    assert.equal(applied.updated.effect.order, "2");
    assert.equal(applied.updated.effect.tree_revision, "4");
    assert.match(applied.updated.effect.project_activity_position, /^[1-9][0-9]*$/);
    assert.match(volumeId, UUID_V7);

    const replay = await updateVolume({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000c53",
      antiForgery: applied.challenge.nonce,
      request,
    });
    assert.equal(replay.command_id, applied.updated.command_id);
    assert.equal(replay.receipt.receipt_id, applied.updated.receipt.receipt_id);

    const tree = await getManuscriptTree({ baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl });
    assert.equal(tree.tree_revision, "4");
    assert.equal(tree.volumes.length, 2);
    assert.equal(tree.volumes[1]?.volume_id, volumeId);
    assert.equal(tree.volumes[1]?.title, "Volume B");
    assert.equal(tree.volumes[1]?.order, "2");

    const stale = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000c54",
      updateRequest("Volume B", "2", "3", "018f0000-0000-7001-8000-000000000c44"),
    );
    assert.equal(stale.updated.receipt.result, "conflicted");
    assert.equal(stale.updated.effect.kind, "conflicted");
    if (stale.updated.effect.kind !== "conflicted") {
      throw new Error("stale Update Volume must conflict");
    }
    assert.equal(stale.updated.effect.reason, "stale_volume_revision");
    const afterStale = await getManuscriptTree({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(afterStale.tree_revision, "4");
    assert.equal(afterStale.volumes.length, 2);

    const unchanged = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000c55",
      updateRequest("Volume B", "2", "4", "018f0000-0000-7001-8000-000000000c45"),
    );
    assert.equal(unchanged.updated.receipt.result, "no_effect");
    assert.equal(unchanged.updated.effect.kind, "no_effect");
    if (unchanged.updated.effect.kind !== "no_effect") {
      throw new Error("unchanged Update Volume must have no effect");
    }
    assert.equal(unchanged.updated.effect.reason, "unchanged");

    const invalidJoin = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      MISSING_VOLUME,
      "018f0000-0000-7001-8000-000000000c56",
      updateRequest("Volume B", "1", "4", "018f0000-0000-7001-8000-000000000c46"),
    );
    assert.equal(invalidJoin.updated.receipt.result, "refused");
    assert.equal(invalidJoin.updated.effect.kind, "refused");
    if (invalidJoin.updated.effect.kind !== "refused") {
      throw new Error("missing Volume must refuse");
    }
    assert.equal(invalidJoin.updated.effect.reason, "invalid_volume_join");

    const invalidOrder = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000c57",
      updateRequest("Volume B", "3", "4", "018f0000-0000-7001-8000-000000000c47"),
    );
    assert.equal(invalidOrder.updated.receipt.result, "refused");
    if (invalidOrder.updated.effect.kind !== "refused") {
      throw new Error("order beyond the Volume count must refuse");
    }
    assert.equal(invalidOrder.updated.effect.reason, "invalid_order");

    await assert.rejects(
      updateVolume({
        baseUrl,
        projectId: first.projectId,
        volumeId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000c58",
        antiForgery: applied.challenge.nonce,
        request: updateRequest("Changed Retry", "2", "4", "018f0000-0000-7001-8000-000000000c48"),
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );

    await assert.rejects(
      patchVolume(
        baseUrl,
        first.fetchImpl,
        first.projectId,
        volumeId,
        "018f0000-0000-7001-8000-000000000c59",
        updateRequest("", "2", "4", "018f0000-0000-7001-8000-000000000c49"),
      ),
      (error) => requireStoryOSProtocolError(error).status === 400,
    );

    const archiveDigest = await digestArchiveProject(archiveRequest("1", "018f0000-0000-7001-8000-000000000c4a"));
    const archiveChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: archiveDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000c5a",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000c5a",
      antiForgery: archiveChallenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-000000000c4a"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");

    const refused = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000c5b",
      updateRequest("Volume B", "2", "4", "018f0000-0000-7001-8000-000000000c4b"),
    );
    assert.equal(refused.updated.receipt.result, "refused");
    assert.equal(refused.updated.effect.kind, "refused");
    if (refused.updated.effect.kind !== "refused") {
      throw new Error("Update Volume on an archived Project must refuse");
    }
    assert.equal(refused.updated.effect.reason, "archived_project");

    const foreign = await createEmpty(baseUrl, "session-b", "018f0000-0000-7001-8000-000000000c32", "Other Novel");
    await assert.rejects(
      patchVolume(
        baseUrl,
        foreign.fetchImpl,
        first.projectId,
        volumeId,
        "018f0000-0000-7001-8000-000000000c5c",
        updateRequest("Stolen Volume", "1", "4", "018f0000-0000-7001-8000-000000000c4c"),
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
