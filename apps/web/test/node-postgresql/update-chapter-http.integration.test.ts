import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  archiveProject,
  createChapter,
  createEditorSession,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  digestArchiveProject,
  digestCreateChapter,
  digestCreateEditorSession,
  digestCreateVolume,
  digestSetCurrentChapter,
  digestUpdateChapter,
  digestUpdateProject,
  getChapter,
  getManuscriptTree,
  getProject,
  setCurrentChapter,
  updateChapter,
  updateProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateChapterRequest,
  CreateEditorSessionRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  SetCurrentChapterRequest,
  UpdateChapterRequest,
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
  expectedTreeRevision: string,
  correlationId: string,
): UpdateChapterRequest {
  return {
    command_schema: "storyos.command.update-chapter.request.v1",
    update_chapter_input: {
      title,
      order,
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

// One Project admits at most PROJECT_COMMAND_CHALLENGE_RATE_CAPACITY (10) Command
// Challenges in one 60-second window. The eleventh waits for the next window, so each
// test in this file stays at or under 10 Command Challenges on one Project.
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
    assert.equal(applied.updated.receipt.authoritative_commit_ids.length, 1);
    assert.match(applied.updated.receipt.authoritative_commit_ids[0] ?? "", UUID_V7);
    assert.equal(applied.updated.receipt.author_action_sequence, "4");
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
    assert.deepEqual(
      replay.receipt.authoritative_commit_ids,
      applied.updated.receipt.authoritative_commit_ids,
    );
    assert.equal(replay.receipt.author_action_sequence, "4");

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
    assert.equal(tree.snapshot.project_activity_position, applied.updated.effect.project_activity_position);
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
    assert.equal(stale.updated.effect.reason, "stale_tree_revision");
    assert.deepEqual(stale.updated.receipt.authoritative_commit_ids, []);
    assert.equal(stale.updated.receipt.author_action_sequence, null);
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
    assert.deepEqual(unchanged.updated.receipt.authoritative_commit_ids, []);
    assert.equal(unchanged.updated.receipt.author_action_sequence, null);

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

test("updateChapter refuses a missing Chapter join and an archived Project", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const owned = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000000e60",
      "Archive Then Update",
      "018f0000-0000-7001-8000-000000000e61",
    );
    const volume = await postVolume(
      baseUrl,
      owned.fetchImpl,
      owned.projectId,
      "018f0000-0000-7001-8000-000000000e62",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000e63"),
    );
    if (volume.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const chapter = await postChapter(
      baseUrl,
      owned.fetchImpl,
      owned.projectId,
      volume.created.effect.volume_id,
      "018f0000-0000-7001-8000-000000000e64",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-000000000e65"),
    );
    if (chapter.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Chapter must apply");
    }
    const chapterId = chapter.created.effect.chapter_id;

    const invalidJoin = await patchChapter(
      baseUrl,
      owned.fetchImpl,
      owned.projectId,
      MISSING_CHAPTER,
      "018f0000-0000-7001-8000-000000000e66",
      updateRequest("Chapter B", "1", "3", "018f0000-0000-7001-8000-000000000e67"),
    );
    assert.equal(invalidJoin.updated.receipt.result, "refused");
    assert.equal(invalidJoin.updated.effect.kind, "refused");
    if (invalidJoin.updated.effect.kind !== "refused") {
      throw new Error("missing Chapter must refuse");
    }
    assert.equal(invalidJoin.updated.effect.reason, "invalid_chapter_join");
    assert.deepEqual(invalidJoin.updated.receipt.authoritative_commit_ids, []);
    assert.equal(invalidJoin.updated.receipt.author_action_sequence, null);

    const archiveDigest = await digestArchiveProject(archiveRequest("1", "018f0000-0000-7001-8000-000000000e68"));
    const archiveChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: owned.projectId,
      fetchImpl: owned.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: archiveDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000e69",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId: owned.projectId,
      fetchImpl: owned.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000e69",
      antiForgery: archiveChallenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-000000000e68"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");

    const refused = await patchChapter(
      baseUrl,
      owned.fetchImpl,
      owned.projectId,
      chapterId,
      "018f0000-0000-7001-8000-000000000e6a",
      updateRequest("Chapter A", "1", "3", "018f0000-0000-7001-8000-000000000e6b"),
    );
    assert.equal(refused.updated.receipt.result, "refused");
    assert.equal(refused.updated.effect.kind, "refused");
    if (refused.updated.effect.kind !== "refused") {
      throw new Error("Update Chapter on an archived Project must refuse");
    }
    assert.equal(refused.updated.effect.reason, "archived_project");
  } finally {
    await stopRealServer(server);
  }
});

function capturingChapterPatch(inner: typeof fetch): { fetchImpl: typeof fetch; lastPatchBody: () => string } {
  let lastPatchBody = "";
  return {
    fetchImpl: async (input, init) => {
      const response = await inner(input, init);
      const url = String(input instanceof Request ? input.url : input);
      if (response.ok && (init?.method ?? "GET") === "PATCH" && url.includes("/chapters/")) {
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

async function renameProject(
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
      command_schema: request.command_schema,
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const renamed = await updateProject({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, renamed };
}

// One Project admits at most 10 Command Challenges in one 60-second window.
test("updateChapter freezes applied, no-effect, and conflict acknowledgements after later title and Current Chapter changes", async () => {
  let { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000000f00",
      "Chapter Update Freeze Novel",
      "018f0000-0000-7001-8000-000000000f01",
    );
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000f02",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000f03"),
    );
    if (volume.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const volumeId = volume.created.effect.volume_id;
    const chapterA = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000f04",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-000000000f05"),
    );
    const chapterB = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000f06",
      chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-000000000f07"),
    );
    if (chapterA.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Chapter A must apply");
    }
    if (chapterB.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Chapter B must apply");
    }
    const chapterId = chapterA.created.effect.chapter_id;
    const chapterBId = chapterB.created.effect.chapter_id;
    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000f0b",
    };
    const sessionDigest = await digestCreateEditorSession(sessionRequest);
    const sessionChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/editor-sessions",
        command_schema: sessionRequest.command_schema,
        canonical_command_digest: sessionDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000f0a",
      },
    }));
    const session = await createEditorSession({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f0a",
      antiForgery: sessionChallenge.nonce,
      request: sessionRequest,
    });
    const appliedRequest = updateRequest("Chapter A Renamed", "1", "4", "018f0000-0000-7001-8000-000000000f09");
    const firstCapture = capturingChapterPatch(first.fetchImpl);
    const applied = await patchChapter(
      baseUrl,
      firstCapture.fetchImpl,
      first.projectId,
      chapterId,
      "018f0000-0000-7001-8000-000000000f08",
      appliedRequest,
    );
    const appliedBody = firstCapture.lastPatchBody();
    assert.equal(applied.updated.effect.kind, "authoritative_applied");
    assert.equal(applied.updated.project.title, "Chapter Update Freeze Novel");
    assert.equal(applied.updated.project.open.kind, "current_chapter");
    if (applied.updated.project.open.kind !== "current_chapter") {
      throw new Error("Update Chapter must keep Chapter A");
    }
    assert.equal(applied.updated.project.open.current_chapter_id, chapterId);
    const unchangedRequest = updateRequest("Chapter A Renamed", "1", "5", "018f0000-0000-7001-8000-000000000f11");
    const unchangedCapture = capturingChapterPatch(first.fetchImpl);
    const unchanged = await patchChapter(
      baseUrl,
      unchangedCapture.fetchImpl,
      first.projectId,
      chapterId,
      "018f0000-0000-7001-8000-000000000f10",
      unchangedRequest,
    );
    const unchangedBody = unchangedCapture.lastPatchBody();
    assert.equal(unchanged.updated.receipt.result, "no_effect");
    assert.equal(unchanged.updated.project.title, "Chapter Update Freeze Novel");
    assert.equal(unchanged.updated.project.open.kind, "current_chapter");
    if (unchanged.updated.project.open.kind !== "current_chapter") {
      throw new Error("no-effect Update Chapter must keep Chapter A");
    }
    assert.equal(unchanged.updated.project.open.current_chapter_id, chapterId);
    const staleRequest = updateRequest("Stale Chapter", "1", "3", "018f0000-0000-7001-8000-000000000f13");
    const staleCapture = capturingChapterPatch(first.fetchImpl);
    const stale = await patchChapter(
      baseUrl,
      staleCapture.fetchImpl,
      first.projectId,
      chapterId,
      "018f0000-0000-7001-8000-000000000f12",
      staleRequest,
    );
    const staleBody = staleCapture.lastPatchBody();
    assert.equal(stale.updated.receipt.result, "conflicted");
    assert.equal(stale.updated.project.title, "Chapter Update Freeze Novel");
    const openedB = await getChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId: chapterBId,
      fetchImpl: first.fetchImpl,
    });
    const switchRequest: SetCurrentChapterRequest = {
      command_schema: "storyos.command.set-current-chapter.request.v1",
      set_current_chapter_input: {
        chapter_id: chapterBId,
        expected_current_chapter_id: chapterId,
        expected_target_revision_id: openedB.chapter.current_revision.revision_id,
        editor_session_id: session.editor_session.editor_session_id,
        client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: "storyos.web-security-policy.release-1.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000f0d",
      },
    };
    const switchDigest = await digestSetCurrentChapter(switchRequest);
    const switchChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/current-chapter",
        command_schema: switchRequest.command_schema,
        canonical_command_digest: switchDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000f0c",
      },
    }));
    const switched = await setCurrentChapter({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f0c",
      antiForgery: switchChallenge.nonce,
      request: switchRequest,
    });
    assert.equal(switched.effect.kind, "authoritative_applied");
    const later = await renameProject(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000f0e",
      renameRequest("Later Chapter Title", "1", "018f0000-0000-7001-8000-000000000f0f"),
    );
    assert.equal(later.renamed.project.title, "Later Chapter Title");
    const frozenCapture = capturingChapterPatch(first.fetchImpl);
    const frozen = await updateChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId,
      fetchImpl: frozenCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f08",
      antiForgery: applied.challenge.nonce,
      request: appliedRequest,
    });
    assert.deepEqual(frozen, applied.updated);
    assert.equal(frozenCapture.lastPatchBody(), appliedBody);
    assert.equal(frozen.project.title, "Chapter Update Freeze Novel");
    if (frozen.project.open.kind !== "current_chapter") {
      throw new Error("retry must keep Chapter A");
    }
    assert.equal(frozen.project.open.current_chapter_id, chapterId);
    const frozenUnchangedCapture = capturingChapterPatch(first.fetchImpl);
    const frozenUnchanged = await updateChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId,
      fetchImpl: frozenUnchangedCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f10",
      antiForgery: unchanged.challenge.nonce,
      request: unchangedRequest,
    });
    assert.deepEqual(frozenUnchanged, unchanged.updated);
    assert.equal(frozenUnchangedCapture.lastPatchBody(), unchangedBody);
    const frozenStaleCapture = capturingChapterPatch(first.fetchImpl);
    const frozenStale = await updateChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId,
      fetchImpl: frozenStaleCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f12",
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
    assert.equal(opened.project.title, "Later Chapter Title");
    assert.equal(opened.project.open.kind, "current_chapter");
    if (opened.project.open.kind !== "current_chapter") {
      throw new Error("GET must report Chapter B");
    }
    assert.equal(opened.project.open.current_chapter_id, chapterBId);
    const receiptCount = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'updateChapter';
    `);
    assert.equal(receiptCount, "3");
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.authoritative_commits
         WHERE project_id = '${first.projectId}'::uuid
           AND receipt_id = '${applied.updated.receipt.receipt_id}'::uuid;
      `),
      "1",
    );
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.author_action_entries
         WHERE project_id = '${first.projectId}'::uuid
           AND receipt_id = '${applied.updated.receipt.receipt_id}'::uuid;
      `),
      "1",
    );
    await stopRealServer(server);
    ({ baseUrl, server } = await startRealServer());
    const restartedFetch = browserFetch(baseUrl, "session-a");
    const afterRestartCapture = capturingChapterPatch(restartedFetch);
    const afterRestart = await updateChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId,
      fetchImpl: afterRestartCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f08",
      antiForgery: applied.challenge.nonce,
      request: appliedRequest,
    });
    assert.deepEqual(afterRestart, applied.updated);
    assert.equal(afterRestartCapture.lastPatchBody(), appliedBody);
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'updateChapter';
      `),
      receiptCount,
    );
  } finally {
    await stopRealServer(server);
  }
});

test("updateChapter distinguishes historical absence from damaged new-format evidence", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000000f16",
      "Chapter History Novel",
      "018f0000-0000-7001-8000-000000000f17",
    );
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000f18",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000f19"),
    );
    if (volume.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const chapter = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volume.created.effect.volume_id,
      "018f0000-0000-7001-8000-000000000f1a",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-000000000f1b"),
    );
    if (chapter.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Chapter must apply");
    }
    const request = updateRequest("Chapter A Renamed", "1", "3", "018f0000-0000-7001-8000-000000000f1d");
    const applied = await patchChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      chapter.created.effect.chapter_id,
      "018f0000-0000-7001-8000-000000000f1c",
      request,
    );
    const novelsBefore = await queryPostgres(`
      SELECT title || ' ' || count(*)::text FROM storyos.projects
       WHERE project_id = '${first.projectId}'::uuid GROUP BY title;
    `);
    const receiptsBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${first.projectId}'::uuid;
    `);
    const chaptersBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.manuscript_objects
       WHERE project_id = '${first.projectId}'::uuid AND object_kind = 'chapter';
    `);
    await queryPostgres(`
      UPDATE storyos.command_idempotency
         SET acknowledgement_format = NULL, response_project = NULL
       WHERE project_id = '${first.projectId}'::uuid
         AND idempotency_key = '018f0000-0000-7001-8000-000000000f1c'::uuid;
    `);
    await assert.rejects(
      updateChapter({
        baseUrl,
        projectId: first.projectId,
        chapterId: chapter.created.effect.chapter_id,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000f1c",
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
         AND idempotency_key = '018f0000-0000-7001-8000-000000000f1c'::uuid;
    `);
    await assert.rejects(
      updateChapter({
        baseUrl,
        projectId: first.projectId,
        chapterId: chapter.created.effect.chapter_id,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000f1c",
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
        SELECT count(*) FROM storyos.manuscript_objects
         WHERE project_id = '${first.projectId}'::uuid AND object_kind = 'chapter';
      `),
      chaptersBefore,
    );
  } finally {
    await stopRealServer(server);
  }
});
