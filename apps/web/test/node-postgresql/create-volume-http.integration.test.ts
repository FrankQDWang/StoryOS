import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  activityStream,
  archiveProject,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  deleteVolume,
  digestArchiveProject,
  digestCreateVolume,
  digestDeleteVolume,
  digestUpdateVolume,
  getManuscriptTree,
  updateVolume,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  CreateVolumeResponse,
  DeleteVolumeRequest,
  UpdateVolumeRequest,
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

function deleteRequest(expectedTreeRevision: string, correlationId: string): DeleteVolumeRequest {
  return {
    command_schema: "storyos.command.delete-volume.request.v1",
    delete_volume_input: {
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
  expectedTreeRevision: string,
  correlationId: string,
): UpdateVolumeRequest {
  return {
    command_schema: "storyos.command.update-volume.request.v1",
    update_volume_input: {
      title,
      order,
      expected_tree_revision: expectedTreeRevision,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: correlationId,
    },
  };
}

async function deleteOwned(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  volumeId: string,
  idempotencyKey: string,
  request: DeleteVolumeRequest,
) {
  const digest = await digestDeleteVolume(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "DELETE",
      route_template: "/api/v1/projects/{project_id}/volumes/{volume_id}",
      command_schema: "storyos.command.delete-volume.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  return deleteVolume({
    baseUrl,
    projectId,
    volumeId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
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
  return {
    challenge,
    updated: await updateVolume({
      baseUrl,
      projectId,
      volumeId,
      fetchImpl,
      idempotencyKey,
      antiForgery: challenge.nonce,
      request,
    }),
  };
}

function appliedVolume(created: CreateVolumeResponse) {
  if (created.effect.kind !== "authoritative_applied") {
    throw new Error("Create Volume must apply");
  }
  return created.effect;
}

test("createVolume reports Canonical Sibling Order through removal, replay, and historical acks", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000861", "Order Novel");
    const volumeA = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000871",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000881"),
    );
    const volumeB = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000872",
      volumeRequest("Volume B", "2", "018f0000-0000-7001-8000-000000000882"),
    );
    const volumeC = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000873",
      volumeRequest("Volume C", "3", "018f0000-0000-7001-8000-000000000883"),
    );
    assert.equal(appliedVolume(volumeA.created).order, "1");
    assert.equal(appliedVolume(volumeB.created).order, "2");
    assert.equal(appliedVolume(volumeC.created).order, "3");
    const treeAfterC = await getManuscriptTree({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    const snapshotAfterC = treeAfterC.snapshot.snapshot_id;
    assert.equal(treeAfterC.snapshot.replay_generation, "1");
    const replayB = await createVolume({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000872",
      antiForgery: volumeB.challenge.nonce,
      request: volumeRequest("Volume B", "2", "018f0000-0000-7001-8000-000000000882"),
    });
    assert.equal(replayB.command_id, volumeB.created.command_id);
    assert.equal(appliedVolume(replayB).order, "2");

    const volumeBId = appliedVolume(volumeB.created).volume_id;
    const removedB = await deleteOwned(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeBId,
      "018f0000-0000-7001-8000-000000000874",
      deleteRequest("4", "018f0000-0000-7001-8000-000000000884"),
    );
    assert.equal(removedB.effect.kind, "authoritative_applied");

    const volumeD = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000875",
      volumeRequest("Volume D", "5", "018f0000-0000-7001-8000-000000000885"),
    );
    const createdD = appliedVolume(volumeD.created);
    assert.equal(createdD.order, "3");
    const replayD = await createVolume({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000875",
      antiForgery: volumeD.challenge.nonce,
      request: volumeRequest("Volume D", "5", "018f0000-0000-7001-8000-000000000885"),
    });
    assert.equal(replayD.command_id, volumeD.created.command_id);
    assert.equal(appliedVolume(replayD).order, "3");

    const tree = await getManuscriptTree({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(tree.tree_revision, "6");
    assert.equal(tree.snapshot.replay_generation, "1");
    assert.deepEqual(
      tree.volumes.map((volume) => ({ title: volume.title, order: volume.order, volume_id: volume.volume_id })),
      [
        { title: "Volume A", order: "1", volume_id: appliedVolume(volumeA.created).volume_id },
        { title: "Volume C", order: "2", volume_id: appliedVolume(volumeC.created).volume_id },
        { title: "Volume D", order: "3", volume_id: createdD.volume_id },
      ],
    );

    const createdDActivity = (await activityStream({
      baseUrl,
      projectId: first.projectId,
      snapshotId: snapshotAfterC,
      protocolRelease: "storyos.public.release.1",
      fetchImpl: first.fetchImpl,
    })).split("\n\n").map((block) => {
      const data = block.split("\n").find((line) => line.startsWith("data:"));
      return data === undefined ? undefined : JSON.parse(data.slice(5).trim()) as {
        event_kind?: string;
        event_schema?: string;
        payload?: { volume_id?: string; order?: string };
      };
    }).find((event) => event?.event_kind === "volume_created"
      && event.payload?.volume_id === createdD.volume_id);
    assert.equal(createdDActivity?.event_schema, "storyos.event.volume-created.v2");
    assert.equal(createdDActivity?.payload?.order, "3");

    const durable = JSON.parse(await queryPostgres(`SELECT json_build_object(
      'receipt_order', receipt.result_payload->>'order',
      'activity_order', payload.payload->>'order'
    )::text
      FROM storyos.domain_receipts AS receipt
      JOIN storyos.project_activity_event_payloads AS payload
        ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
           (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
     WHERE receipt.receipt_id = '${volumeD.created.receipt.receipt_id}'::uuid`));
    assert.equal(durable.receipt_order, "3");
    assert.equal(durable.activity_order, "3");

    await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      createdD.volume_id,
      "018f0000-0000-7001-8000-000000000876",
      updateRequest("Volume D", "1", "6", "018f0000-0000-7001-8000-000000000886"),
    );
    const replayAfterReorder = await createVolume({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000875",
      antiForgery: volumeD.challenge.nonce,
      request: volumeRequest("Volume D", "5", "018f0000-0000-7001-8000-000000000885"),
    });
    assert.equal(appliedVolume(replayAfterReorder).order, "3");

    const removedC = await deleteOwned(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      appliedVolume(volumeC.created).volume_id,
      "018f0000-0000-7001-8000-000000000877",
      deleteRequest("7", "018f0000-0000-7001-8000-000000000887"),
    );
    assert.equal(removedC.effect.kind, "authoritative_applied");
    const replayAfterDelete = await createVolume({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000875",
      antiForgery: volumeD.challenge.nonce,
      request: volumeRequest("Volume D", "5", "018f0000-0000-7001-8000-000000000885"),
    });
    assert.equal(appliedVolume(replayAfterDelete).order, "3");

    await queryPostgres(`UPDATE storyos.domain_receipts
        SET result_payload = '{}'::jsonb
      WHERE receipt_id = '${volumeB.created.receipt.receipt_id}'::uuid`);
    const historicalB = await createVolume({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000872",
      antiForgery: volumeB.challenge.nonce,
      request: volumeRequest("Volume B", "2", "018f0000-0000-7001-8000-000000000882"),
    });
    assert.equal(appliedVolume(historicalB).order, "1");
    assert.equal(historicalB.receipt.receipt_id, volumeB.created.receipt.receipt_id);
  } finally {
    await stopRealServer(server);
  }
});
