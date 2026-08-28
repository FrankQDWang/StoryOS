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
  digestArchiveProject,
  digestCreateChapter,
  digestCreateVolume,
  digestUpdateChapter,
  getChapter,
  getManuscriptTree,
  updateChapter,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateChapterRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  UpdateChapterRequest,
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
const MISSING_CHAPTER = "018f0000-0000-7001-8000-00000000ffff";
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

function updateRequest(
  title: string,
  order: string,
  expectedChapterRevision: string,
  correlationId: string,
): UpdateChapterRequest {
  return {
    command_schema: "storyos.command.update-chapter.request.v1",
    update_chapter_input: {
      title,
      order,
      expected_chapter_revision: expectedChapterRevision,
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
  const created = await createChapter({
    baseUrl,
    projectId,
    volumeId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, created };
}

async function patchChapter(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  idempotencyKey: string,
  request: UpdateChapterRequest,
) {
  const digest = await digestUpdateChapter(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "PATCH",
      route_template: "/api/v1/projects/{project_id}/chapters/{chapter_id}",
      command_schema: "storyos.command.update-chapter.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const updated = await updateChapter({
    baseUrl,
    projectId,
    chapterId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, updated };
}

test("updateChapter renames and reorders one Chapter, replays, and fails closed", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000000e31",
      "Empty Novel",
      "018f0000-0000-7001-8000-000000000e30",
    );
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000e51",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000e41"),
    );
    assert.equal(volume.created.effect.kind, "authoritative_applied");
    if (volume.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const volumeId = volume.created.effect.volume_id;
    const chapterA = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000e52",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-000000000e42"),
    );
    const chapterB = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000e53",
      chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-000000000e43"),
    );
    assert.equal(chapterA.created.effect.kind, "authoritative_applied");
    assert.equal(chapterB.created.effect.kind, "authoritative_applied");
    if (chapterA.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Chapter A must apply");
    }
    const chapterId = chapterA.created.effect.chapter_id;
    const request = updateRequest("Chapter B", "2", "4", "018f0000-0000-7001-8000-000000000e44");
    const applied = await patchChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      chapterId,
      "018f0000-0000-7001-8000-000000000e54",
      request,
    );
    assert.equal(applied.updated.schema_id, "storyos.command.update-chapter.response.v1");
    assert.equal(applied.updated.receipt.command_kind, "updateChapter");
    assert.equal(applied.updated.receipt.result, "authoritative_applied");
    assert.equal(applied.updated.effect.kind, "authoritative_applied");
    if (applied.updated.effect.kind !== "authoritative_applied") {
      throw new Error("Update Chapter must apply");
    }
    assert.equal(applied.updated.effect.chapter_id, chapterId);
    assert.equal(applied.updated.effect.title, "Chapter B");
    assert.equal(applied.updated.effect.order, "2");
    assert.equal(applied.updated.effect.tree_revision, "5");
    assert.match(applied.updated.effect.project_activity_position, /^[1-9][0-9]*$/);
    assert.match(chapterId, UUID_V7);
    assert.equal(applied.updated.project.open.kind, "current_chapter");
    if (applied.updated.project.open.kind !== "current_chapter") {
      throw new Error("Update Chapter must preserve current-Chapter identity");
    }
    assert.equal(applied.updated.project.open.current_chapter_id, chapterId);

    const replay = await updateChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000e54",
      antiForgery: applied.challenge.nonce,
      request,
    });
    assert.equal(replay.command_id, applied.updated.command_id);
    assert.equal(replay.receipt.receipt_id, applied.updated.receipt.receipt_id);

    const opened = await getChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(opened.chapter.title, "Chapter B");
    assert.equal(opened.chapter.current_revision.body, "");

    const tree = await getManuscriptTree({ baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl });
    assert.equal(tree.tree_revision, "5");
    assert.equal(tree.volumes.length, 1);
    assert.equal(tree.volumes[0]?.chapters.length, 2);
    assert.equal(tree.volumes[0]?.chapters[0]?.title, "Chapter B");
    assert.equal(tree.volumes[0]?.chapters[0]?.order, "1");
    assert.equal(tree.volumes[0]?.chapters[1]?.chapter_id, chapterId);
    assert.equal(tree.volumes[0]?.chapters[1]?.title, "Chapter B");
    assert.equal(tree.volumes[0]?.chapters[1]?.order, "2");

    const stale = await patchChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      chapterId,
      "018f0000-0000-7001-8000-000000000e55",
      updateRequest("Chapter B", "2", "4", "018f0000-0000-7001-8000-000000000e45"),
    );
    assert.equal(stale.updated.receipt.result, "conflicted");
    assert.equal(stale.updated.effect.kind, "conflicted");
    if (stale.updated.effect.kind !== "conflicted") {
      throw new Error("stale Update Chapter must conflict");
    }
    assert.equal(stale.updated.effect.reason, "stale_chapter_revision");
    const afterStale = await getManuscriptTree({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(afterStale.tree_revision, "5");
    assert.equal(afterStale.volumes[0]?.chapters.length, 2);

    const unchanged = await patchChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      chapterId,
      "018f0000-0000-7001-8000-000000000e56",
      updateRequest("Chapter B", "2", "5", "018f0000-0000-7001-8000-000000000e46"),
    );
    assert.equal(unchanged.updated.receipt.result, "no_effect");
    assert.equal(unchanged.updated.effect.kind, "no_effect");
    if (unchanged.updated.effect.kind !== "no_effect") {
      throw new Error("unchanged Update Chapter must have no effect");
    }
    assert.equal(unchanged.updated.effect.reason, "unchanged");

    const invalidJoin = await patchChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      MISSING_CHAPTER,
      "018f0000-0000-7001-8000-000000000e57",
      updateRequest("Chapter B", "1", "5", "018f0000-0000-7001-8000-000000000e47"),
    );
    assert.equal(invalidJoin.updated.receipt.result, "refused");
    assert.equal(invalidJoin.updated.effect.kind, "refused");
    if (invalidJoin.updated.effect.kind !== "refused") {
      throw new Error("missing Chapter must refuse");
    }
    assert.equal(invalidJoin.updated.effect.reason, "invalid_chapter_join");

    const invalidOrder = await patchChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      chapterId,
      "018f0000-0000-7001-8000-000000000e58",
      updateRequest("Chapter B", "3", "5", "018f0000-0000-7001-8000-000000000e48"),
    );
    assert.equal(invalidOrder.updated.receipt.result, "refused");
    if (invalidOrder.updated.effect.kind !== "refused") {
      throw new Error("order beyond the sibling count must refuse");
    }
    assert.equal(invalidOrder.updated.effect.reason, "invalid_order");

    await assert.rejects(
      updateChapter({
        baseUrl,
        projectId: first.projectId,
        chapterId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000e59",
        antiForgery: applied.challenge.nonce,
        request: updateRequest("Changed Retry", "2", "5", "018f0000-0000-7001-8000-000000000e49"),
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );

    await assert.rejects(
      patchChapter(
        baseUrl,
        first.fetchImpl,
        first.projectId,
        chapterId,
        "018f0000-0000-7001-8000-000000000e5a",
        updateRequest("", "2", "5", "018f0000-0000-7001-8000-000000000e4a"),
      ),
      (error) => requireStoryOSProtocolError(error).status === 400,
    );

    const archiveDigest = await digestArchiveProject(archiveRequest("1", "018f0000-0000-7001-8000-000000000e4b"));
    const archiveChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: archiveDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000e5b",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000e5b",
      antiForgery: archiveChallenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-000000000e4b"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");

    const refused = await patchChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      chapterId,
      "018f0000-0000-7001-8000-000000000e5c",
      updateRequest("Chapter B", "2", "5", "018f0000-0000-7001-8000-000000000e4c"),
    );
    assert.equal(refused.updated.receipt.result, "refused");
    assert.equal(refused.updated.effect.kind, "refused");
    if (refused.updated.effect.kind !== "refused") {
      throw new Error("Update Chapter on an archived Project must refuse");
    }
    assert.equal(refused.updated.effect.reason, "archived_project");

    const foreign = await createEmpty(
      baseUrl,
      "session-b",
      "018f0000-0000-7001-8000-000000000e32",
      "Other Novel",
      "018f0000-0000-7001-8000-000000000e33",
    );
    await assert.rejects(
      patchChapter(
        baseUrl,
        foreign.fetchImpl,
        first.projectId,
        chapterId,
        "018f0000-0000-7001-8000-000000000e5d",
        updateRequest("Stolen Chapter", "1", "5", "018f0000-0000-7001-8000-000000000e4d"),
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
