import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  archiveProject,
  createChapter,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  deleteVolume,
  digestArchiveProject,
  digestCreateChapter,
  digestCreateVolume,
  digestDeleteVolume,
  getManuscriptTree,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateChapterRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  DeleteVolumeRequest,
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

function createChallengeRequest(idempotencyKey: string, title: string, correlationId: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: correlationId,
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

function chapterRequest(title: string, expectedTreeRevision: string, correlationId: string): CreateChapterRequest {
  return {
    command_schema: "storyos.command.create-chapter.request.v1",
    create_chapter_input: {
      title,
      expected_tree_revision: expectedTreeRevision,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: correlationId,
    },
  };
}

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

async function createEmpty(baseUrl: string, session: string, idempotencyKey: string, title: string, correlationId: string) {
  const fetchImpl = browserFetch(baseUrl, session);
  const request = createChallengeRequest(idempotencyKey, title, correlationId);
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
  return createVolume({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
}

async function postChapter(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  volumeId: string,
  idempotencyKey: string,
  request: CreateChapterRequest,
) {
  const digest = await digestCreateChapter(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
      command_schema: "storyos.command.create-chapter.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  return createChapter({
    baseUrl,
    projectId,
    volumeId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
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

function appliedVolumeId(created: { effect: { kind: string; volume_id?: string } }): string {
  if (created.effect.kind !== "authoritative_applied" || created.effect.volume_id === undefined) {
    throw new Error("Volume must apply");
  }
  return created.effect.volume_id;
}

test("deleteVolume removes an empty Volume, refuses a nonempty Volume, and honors deletion", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const { fetchImpl, projectId } = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-00000000d201",
      "Delete Volume Novel",
      "018f0000-0000-7001-8000-00000000d202",
    );
    const volumeA = await postVolume(
      baseUrl,
      fetchImpl,
      projectId,
      "018f0000-0000-7001-8000-00000000d203",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-00000000d204"),
    );
    const volumeB = await postVolume(
      baseUrl,
      fetchImpl,
      projectId,
      "018f0000-0000-7001-8000-00000000d205",
      volumeRequest("Volume B", "2", "018f0000-0000-7001-8000-00000000d206"),
    );
    const volumeAId = appliedVolumeId(volumeA);
    const volumeBId = appliedVolumeId(volumeB);
    const chapter = await postChapter(
      baseUrl,
      fetchImpl,
      projectId,
      volumeBId,
      "018f0000-0000-7001-8000-00000000d207",
      chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-00000000d208"),
    );
    assert.equal(chapter.effect.kind, "authoritative_applied");
    const treeBefore = await getManuscriptTree({ baseUrl, projectId, fetchImpl });
    assert.deepEqual(treeBefore.volumes.map((volume) => volume.title), ["Volume A", "Volume B"]);

    const refused = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      volumeBId,
      "018f0000-0000-7001-8000-00000000d209",
      deleteRequest("4", "018f0000-0000-7001-8000-00000000d210"),
    );
    assert.equal(refused.effect.kind, "refused");
    if (refused.effect.kind === "refused") {
      assert.equal(refused.effect.reason, "nonempty_volume");
    }
    const treeAfterRefuse = await getManuscriptTree({ baseUrl, projectId, fetchImpl });
    assert.equal(treeAfterRefuse.tree_revision, treeBefore.tree_revision);
    assert.deepEqual(
      treeAfterRefuse.volumes.map((volume) => ({ title: volume.title, order: volume.order })),
      [
        { title: "Volume A", order: "1" },
        { title: "Volume B", order: "2" },
      ],
    );

    const removed = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      volumeAId,
      "018f0000-0000-7001-8000-00000000d211",
      deleteRequest("4", "018f0000-0000-7001-8000-00000000d212"),
    );
    assert.equal(removed.effect.kind, "authoritative_applied");
    assert.equal(removed.receipt.command_kind, "deleteVolume");
    assert.match(removed.command_id, UUID_V7);
    const treeAfterA = await getManuscriptTree({ baseUrl, projectId, fetchImpl });
    assert.deepEqual(
      treeAfterA.volumes.map((volume) => ({ title: volume.title, order: volume.order })),
      [{ title: "Volume B", order: "1" }],
    );

    const retried = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      volumeAId,
      "018f0000-0000-7001-8000-00000000d211",
      deleteRequest("4", "018f0000-0000-7001-8000-00000000d212"),
    );
    assert.equal(retried.effect.kind, "authoritative_applied");
    assert.equal(retried.command_id, removed.command_id);

    const alreadyRemoved = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      volumeAId,
      "018f0000-0000-7001-8000-00000000d213",
      deleteRequest("5", "018f0000-0000-7001-8000-00000000d214"),
    );
    assert.equal(alreadyRemoved.effect.kind, "no_effect");
    if (alreadyRemoved.effect.kind === "no_effect") {
      assert.equal(alreadyRemoved.effect.reason, "already_removed");
    }

    const revived = await postChapter(
      baseUrl,
      fetchImpl,
      projectId,
      volumeAId,
      "018f0000-0000-7001-8000-00000000d215",
      chapterRequest("Revived", "5", "018f0000-0000-7001-8000-00000000d216"),
    );
    assert.equal(revived.effect.kind, "refused");
    if (revived.effect.kind === "refused") {
      assert.equal(revived.effect.reason, "invalid_volume_join");
    }
    const treeAfterRevive = await getManuscriptTree({ baseUrl, projectId, fetchImpl });
    assert.equal(treeAfterRevive.tree_revision, treeAfterA.tree_revision);
    assert.deepEqual(treeAfterRevive.volumes.map((volume) => volume.title), ["Volume B"]);

    const foreign = browserFetch(baseUrl, "session-b");
    await assert.rejects(
      () => deleteOwned(
        baseUrl,
        foreign,
        projectId,
        volumeBId,
        "018f0000-0000-7001-8000-00000000d217",
        deleteRequest("5", "018f0000-0000-7001-8000-00000000d218"),
      ),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );
  } finally {
    await stopRealServer(server);
  }
});

test("deleteVolume refuses an archived Project", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const { fetchImpl, projectId } = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-00000000d301",
      "Archive Then Delete Volume",
      "018f0000-0000-7001-8000-00000000d302",
    );
    const volume = await postVolume(
      baseUrl,
      fetchImpl,
      projectId,
      "018f0000-0000-7001-8000-00000000d303",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-00000000d304"),
    );
    const volumeId = appliedVolumeId(volume);
    const digest = await digestArchiveProject(archiveRequest("1", "018f0000-0000-7001-8000-00000000d305"));
    const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId,
      fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: digest,
        idempotency_key: "018f0000-0000-7001-8000-00000000d306",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d306",
      antiForgery: challenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-00000000d305"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");
    const refused = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      volumeId,
      "018f0000-0000-7001-8000-00000000d307",
      deleteRequest("2", "018f0000-0000-7001-8000-00000000d308"),
    );
    assert.equal(refused.effect.kind, "refused");
    if (refused.effect.kind === "refused") {
      assert.equal(refused.effect.reason, "archived_project");
    }
  } finally {
    await stopRealServer(server);
  }
});
