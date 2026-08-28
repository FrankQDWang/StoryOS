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
  getChapter,
  getManuscriptTree,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateChapterRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
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
const MISSING_VOLUME = "018f0000-0000-7001-8000-00000000ffff";
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

test("createChapter creates three named Chapters, keeps the first current, and fails closed", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000000931",
      "Empty Novel",
      "018f0000-0000-7001-8000-000000000930",
    );
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000951",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000941"),
    );
    assert.equal(volume.created.effect.kind, "authoritative_applied");
    if (volume.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const volumeId = volume.created.effect.volume_id;

    const chapterARequest = chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-000000000942");
    const applied = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000952",
      chapterARequest,
    );
    assert.equal(applied.created.schema_id, "storyos.command.create-chapter.response.v1");
    assert.equal(applied.created.receipt.command_kind, "createChapter");
    assert.equal(applied.created.receipt.result, "authoritative_applied");
    assert.equal(applied.created.effect.kind, "authoritative_applied");
    if (applied.created.effect.kind !== "authoritative_applied") {
      throw new Error("the first Chapter must apply");
    }
    assert.equal(applied.created.effect.title, "Chapter A");
    assert.equal(applied.created.effect.tree_revision, "3");
    assert.equal(applied.created.effect.order, "1");
    assert.equal(applied.created.effect.volume_id, volumeId);
    assert.match(applied.created.effect.chapter_id, UUID_V7);
    assert.equal(applied.created.effect.current_chapter_id, applied.created.effect.chapter_id);
    assert.equal(applied.created.project.open.kind, "current_chapter");
    if (applied.created.project.open.kind !== "current_chapter") {
      throw new Error("the first Chapter must select current");
    }
    assert.equal(applied.created.project.open.current_chapter_id, applied.created.effect.chapter_id);
    const opened = await getChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId: applied.created.effect.chapter_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(opened.chapter.title, "Chapter A");
    assert.equal(opened.chapter.current_revision.body, "");

    const replay = await createChapter({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000952",
      antiForgery: applied.challenge.nonce,
      request: chapterARequest,
    });
    assert.equal(replay.command_id, applied.created.command_id);
    assert.equal(replay.receipt.receipt_id, applied.created.receipt.receipt_id);

    const chapterB = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000953",
      chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-000000000943"),
    );
    assert.equal(chapterB.created.effect.kind, "authoritative_applied");
    if (chapterB.created.effect.kind !== "authoritative_applied") {
      throw new Error("the second Chapter must apply");
    }
    assert.equal(chapterB.created.effect.order, "2");
    assert.equal(chapterB.created.effect.tree_revision, "4");
    assert.equal(chapterB.created.effect.current_chapter_id, applied.created.effect.chapter_id);
    if (chapterB.created.project.open.kind !== "current_chapter") {
      throw new Error("later Chapters must preserve current");
    }
    assert.equal(chapterB.created.project.open.current_chapter_id, applied.created.effect.chapter_id);

    const chapterC = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000954",
      chapterRequest("Chapter C", "4", "018f0000-0000-7001-8000-000000000944"),
    );
    assert.equal(chapterC.created.effect.kind, "authoritative_applied");
    if (chapterC.created.effect.kind !== "authoritative_applied") {
      throw new Error("the third Chapter must apply");
    }
    assert.equal(chapterC.created.effect.order, "3");
    assert.equal(chapterC.created.effect.current_chapter_id, applied.created.effect.chapter_id);

    const tree = await getManuscriptTree({ baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl });
    assert.equal(tree.tree_revision, "5");
    assert.equal(tree.volumes.length, 1);
    assert.equal(tree.volumes[0]?.chapters.length, 3);
    assert.equal(tree.volumes[0]?.chapters[0]?.title, "Chapter A");
    assert.equal(tree.volumes[0]?.chapters[1]?.title, "Chapter B");
    assert.equal(tree.volumes[0]?.chapters[2]?.title, "Chapter C");

    const chapterBId = chapterB.created.effect.kind === "authoritative_applied"
      ? chapterB.created.effect.chapter_id
      : "";
    const chapterCId = chapterC.created.effect.kind === "authoritative_applied"
      ? chapterC.created.effect.chapter_id
      : "";
    const openedB = await getChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId: chapterBId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(openedB.chapter.chapter_id, chapterBId);
    assert.equal(openedB.chapter.title, "Chapter B");
    assert.equal(openedB.chapter.current_revision.body, "");
    const openedC = await getChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId: chapterCId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(openedC.chapter.chapter_id, chapterCId);
    assert.equal(openedC.chapter.title, "Chapter C");
    await assert.rejects(
      getChapter({
        baseUrl,
        projectId: first.projectId,
        chapterId: chapterBId,
        fetchImpl: browserFetch(baseUrl, "session-b"),
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    await queryPostgres(`
      UPDATE storyos.project_snapshots
         SET expires_at = clock_timestamp() - interval '1 second'
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${first.projectId}'::uuid`);
    await assert.rejects(
      getChapter({
        baseUrl,
        projectId: first.projectId,
        chapterId: chapterBId,
        fetchImpl: first.fetchImpl,
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 409
          && JSON.parse(protocol.responseBody ?? "{}").code === "snapshot_expired";
      },
    );
    await queryPostgres(`
      UPDATE storyos.project_snapshots
         SET expires_at = NULL
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${first.projectId}'::uuid`);

    const stale = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000955",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-000000000945"),
    );
    assert.equal(stale.created.receipt.result, "conflicted");
    assert.equal(stale.created.effect.kind, "conflicted");
    if (stale.created.effect.kind !== "conflicted") {
      throw new Error("stale Create Chapter must conflict");
    }
    assert.equal(stale.created.effect.reason, "stale_tree_revision");

    const invalidJoin = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      MISSING_VOLUME,
      "018f0000-0000-7001-8000-000000000956",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-000000000946"),
    );
    assert.equal(invalidJoin.created.receipt.result, "refused");
    assert.equal(invalidJoin.created.effect.kind, "refused");
    if (invalidJoin.created.effect.kind !== "refused") {
      throw new Error("wrong Volume join must refuse");
    }
    assert.equal(invalidJoin.created.effect.reason, "invalid_volume_join");

    await assert.rejects(
      createChapter({
        baseUrl,
        projectId: first.projectId,
        volumeId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000957",
        antiForgery: applied.challenge.nonce,
        request: chapterRequest("Changed Retry", "5", "018f0000-0000-7001-8000-000000000947"),
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );

    await assert.rejects(
      postChapter(
        baseUrl,
        first.fetchImpl,
        first.projectId,
        volumeId,
        "018f0000-0000-7001-8000-000000000958",
        chapterRequest("", "5", "018f0000-0000-7001-8000-000000000948"),
      ),
      (error) => requireStoryOSProtocolError(error).status === 400,
    );

    const archiveDigest = await digestArchiveProject(archiveRequest("1", "018f0000-0000-7001-8000-000000000949"));
    const archiveChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: archiveDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000959",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000959",
      antiForgery: archiveChallenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-000000000949"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");

    const refused = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000960",
      chapterRequest("Chapter D", "5", "018f0000-0000-7001-8000-000000000950"),
    );
    assert.equal(refused.created.receipt.result, "refused");
    assert.equal(refused.created.effect.kind, "refused");
    if (refused.created.effect.kind !== "refused") {
      throw new Error("Create Chapter on an archived Project must refuse");
    }
    assert.equal(refused.created.effect.reason, "archived_project");

    const foreign = await createEmpty(
      baseUrl,
      "session-b",
      "018f0000-0000-7001-8000-000000000932",
      "Other Novel",
      "018f0000-0000-7001-8000-000000000933",
    );
    await assert.rejects(
      postChapter(
        baseUrl,
        foreign.fetchImpl,
        first.projectId,
        volumeId,
        "018f0000-0000-7001-8000-000000000961",
        chapterRequest("Stolen Chapter", "5", "018f0000-0000-7001-8000-000000000934"),
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
