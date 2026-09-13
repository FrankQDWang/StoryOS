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
  deleteChapter,
  digestArchiveProject,
  digestCreateChapter,
  digestCreateEditorSession,
  digestCreateVolume,
  digestDeleteChapter,
  digestUpdateProject,
  getChapter,
  getManuscriptTree,
  getProject,
  updateProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateChapterRequest,
  CreateEditorSessionRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  DeleteChapterRequest,
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

function deleteRequest(expectedTreeRevision: string, correlationId: string): DeleteChapterRequest {
  return {
    command_schema: "storyos.command.delete-chapter.request.v1",
    delete_chapter_input: {
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
  chapterId: string,
  idempotencyKey: string,
  request: DeleteChapterRequest,
) {
  const digest = await digestDeleteChapter(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "DELETE",
      route_template: "/api/v1/projects/{project_id}/chapters/{chapter_id}",
      command_schema: "storyos.command.delete-chapter.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  return deleteChapter({
    baseUrl,
    projectId,
    chapterId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
}

function appliedId(created: { effect: { kind: string; chapter_id?: string } }): string {
  if (created.effect.kind !== "authoritative_applied" || created.effect.chapter_id === undefined) {
    throw new Error("Chapter must apply");
  }
  return created.effect.chapter_id;
}

// One Project admits at most PROJECT_COMMAND_CHALLENGE_RATE_CAPACITY (10) Command
// Challenges in one 60-second window. The eleventh waits for the next window, so each
// test in this file stays at or under 10 Command Challenges on one Project.
test("deleteChapter removes a Chapter, honors deletion, and selects next then previous then empty", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const { fetchImpl, projectId } = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-00000000c201",
      "Delete Novel",
      "018f0000-0000-7001-8000-00000000c202",
    );
    const volume = await postVolume(
      baseUrl,
      fetchImpl,
      projectId,
      "018f0000-0000-7001-8000-00000000c203",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-00000000c204"),
    );
    assert.equal(volume.effect.kind, "authoritative_applied");
    if (volume.effect.kind !== "authoritative_applied") throw new Error("Volume must apply");
    const chapterA = await postChapter(
      baseUrl,
      fetchImpl,
      projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-00000000c205",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-00000000c206"),
    );
    const chapterB = await postChapter(
      baseUrl,
      fetchImpl,
      projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-00000000c207",
      chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-00000000c208"),
    );
    const chapterC = await postChapter(
      baseUrl,
      fetchImpl,
      projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-00000000c209",
      chapterRequest("Chapter C", "4", "018f0000-0000-7001-8000-00000000c210"),
    );
    const chapterAId = appliedId(chapterA);
    const chapterBId = appliedId(chapterB);
    const chapterCId = appliedId(chapterC);
    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      client_contract_revision:
        RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-00000000c230",
    };
    const sessionDigest = await digestCreateEditorSession(sessionRequest);
    const sessionChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId,
      fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/editor-sessions",
        command_schema: sessionRequest.command_schema,
        canonical_command_digest: sessionDigest,
        idempotency_key: "018f0000-0000-7001-8000-00000000c231",
      },
    }));
    const session = await createEditorSession({
      baseUrl,
      projectId,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000c231",
      antiForgery: sessionChallenge.nonce,
      request: sessionRequest,
    });
    assert.equal(session.writer.kind, "current_writer");
    assert.equal(session.base_snapshot.chapter_id, chapterAId);

    const removedB = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      chapterBId,
      "018f0000-0000-7001-8000-00000000c211",
      deleteRequest("5", "018f0000-0000-7001-8000-00000000c212"),
    );
    assert.equal(removedB.effect.kind, "authoritative_applied");
    assert.equal(removedB.receipt.command_kind, "deleteChapter");
    assert.match(removedB.command_id, UUID_V7);
    assert.equal(removedB.project.open.kind, "current_chapter");
    if (removedB.project.open.kind === "current_chapter") {
      assert.equal(removedB.project.open.current_chapter_id, chapterAId);
    }
    const treeAfterB = await getManuscriptTree({ baseUrl, projectId, fetchImpl });
    assert.deepEqual(
      treeAfterB.volumes[0]?.chapters.map((chapter) => chapter.title),
      ["Chapter A", "Chapter C"],
    );
    await assert.rejects(
      getChapter({ baseUrl, projectId, chapterId: chapterBId, fetchImpl }),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );

    const removedA = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      chapterAId,
      "018f0000-0000-7001-8000-00000000c213",
      deleteRequest("6", "018f0000-0000-7001-8000-00000000c214"),
    );
    assert.equal(removedA.effect.kind, "authoritative_applied");
    assert.equal(removedA.project.open.kind, "current_chapter");
    if (removedA.project.open.kind === "current_chapter") {
      assert.equal(removedA.project.open.current_chapter_id, chapterCId);
    }

    const removedC = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      chapterCId,
      "018f0000-0000-7001-8000-00000000c215",
      deleteRequest("7", "018f0000-0000-7001-8000-00000000c216"),
    );
    assert.equal(removedC.effect.kind, "authoritative_applied");
    assert.equal(removedC.project.open.kind, "empty");
    const treeEmpty = await getManuscriptTree({ baseUrl, projectId, fetchImpl });
    assert.deepEqual(treeEmpty.volumes[0]?.chapters, []);
    const project = await getProject({ baseUrl, projectId, fetchImpl });
    assert.equal(project.project.open.kind, "empty");

    const already = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      chapterCId,
      "018f0000-0000-7001-8000-00000000c217",
      deleteRequest("8", "018f0000-0000-7001-8000-00000000c218"),
    );
    assert.equal(already.effect.kind, "no_effect");
    if (already.effect.kind === "no_effect") {
      assert.equal(already.effect.reason, "already_removed");
    }

    const stale = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      chapterAId,
      "018f0000-0000-7001-8000-00000000c219",
      deleteRequest("7", "018f0000-0000-7001-8000-00000000c220"),
    );
    assert.equal(stale.effect.kind, "conflicted");

    const foreign = browserFetch(baseUrl, "session-b");
    await assert.rejects(
      () => deleteOwned(
        baseUrl,
        foreign,
        projectId,
        chapterAId,
        "018f0000-0000-7001-8000-00000000c223",
        deleteRequest("8", "018f0000-0000-7001-8000-00000000c224"),
      ),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );
  } finally {
    await stopRealServer(server);
  }
});

test("deleteChapter refuses a missing Chapter join and an archived Project", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const { fetchImpl, projectId } = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-00000000c301",
      "Archive Then Delete",
      "018f0000-0000-7001-8000-00000000c302",
    );
    const volume = await postVolume(
      baseUrl,
      fetchImpl,
      projectId,
      "018f0000-0000-7001-8000-00000000c303",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-00000000c304"),
    );
    if (volume.effect.kind !== "authoritative_applied") throw new Error("Volume must apply");
    const chapter = await postChapter(
      baseUrl,
      fetchImpl,
      projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-00000000c305",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-00000000c306"),
    );
    const chapterId = appliedId(chapter);

    const invalidJoin = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      MISSING_CHAPTER,
      "018f0000-0000-7001-8000-00000000c311",
      deleteRequest("3", "018f0000-0000-7001-8000-00000000c312"),
    );
    assert.equal(invalidJoin.effect.kind, "refused");
    if (invalidJoin.effect.kind === "refused") {
      assert.equal(invalidJoin.effect.reason, "invalid_chapter_join");
    }

    const digest = await digestArchiveProject(archiveRequest("1", "018f0000-0000-7001-8000-00000000c307"));
    const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId,
      fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: digest,
        idempotency_key: "018f0000-0000-7001-8000-00000000c308",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000c308",
      antiForgery: challenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-00000000c307"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");
    const refused = await deleteOwned(
      baseUrl,
      fetchImpl,
      projectId,
      chapterId,
      "018f0000-0000-7001-8000-00000000c309",
      deleteRequest("3", "018f0000-0000-7001-8000-00000000c310"),
    );
    assert.equal(refused.effect.kind, "refused");
    if (refused.effect.kind === "refused") {
      assert.equal(refused.effect.reason, "archived_project");
    }
  } finally {
    await stopRealServer(server);
  }
});

function capturingDelete(
  inner: typeof fetch,
): { fetchImpl: typeof fetch; lastDeleteBody: () => string } {
  let lastDeleteBody = "";
  return {
    fetchImpl: async (input, init) => {
      const response = await inner(input, init);
      const url = String(input instanceof Request ? input.url : input);
      if (response.ok && (init?.method ?? "GET") === "DELETE" && url.includes("/chapters/")) {
        lastDeleteBody = await response.clone().text();
      }
      return response;
    },
    lastDeleteBody: () => lastDeleteBody,
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

async function deleteOwnedWithChallenge(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  idempotencyKey: string,
  request: DeleteChapterRequest,
) {
  const digest = await digestDeleteChapter(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "DELETE",
      route_template: "/api/v1/projects/{project_id}/chapters/{chapter_id}",
      command_schema: "storyos.command.delete-chapter.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const deleted = await deleteChapter({
    baseUrl,
    projectId,
    chapterId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, deleted };
}

// One Project admits at most 10 Command Challenges in one 60-second window.
test("deleteChapter freezes applied empty Current Chapter, no-effect, and conflict acknowledgements after later title and Current Chapter changes", async () => {
  let { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-00000000e900",
      "Delete Chapter Freeze Novel",
      "018f0000-0000-7001-8000-00000000e901",
    );
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000e902",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-00000000e903"),
    );
    assert.equal(volume.effect.kind, "authoritative_applied");
    if (volume.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const chapterA = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-00000000e904",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-00000000e905"),
    );
    const chapterAId = appliedId(chapterA);
    const appliedRequest = deleteRequest("3", "018f0000-0000-7001-8000-00000000e907");
    const appliedCapture = capturingDelete(first.fetchImpl);
    const applied = await deleteOwnedWithChallenge(
      baseUrl,
      appliedCapture.fetchImpl,
      first.projectId,
      chapterAId,
      "018f0000-0000-7001-8000-00000000e906",
      appliedRequest,
    );
    const appliedBody = appliedCapture.lastDeleteBody();
    assert.equal(applied.deleted.effect.kind, "authoritative_applied");
    assert.equal(applied.deleted.project.title, "Delete Chapter Freeze Novel");
    assert.equal(applied.deleted.project.open.kind, "empty");
    const unchangedRequest = deleteRequest("4", "018f0000-0000-7001-8000-00000000e909");
    const unchangedCapture = capturingDelete(first.fetchImpl);
    const unchanged = await deleteOwnedWithChallenge(
      baseUrl,
      unchangedCapture.fetchImpl,
      first.projectId,
      chapterAId,
      "018f0000-0000-7001-8000-00000000e908",
      unchangedRequest,
    );
    const unchangedBody = unchangedCapture.lastDeleteBody();
    assert.equal(unchanged.deleted.receipt.result, "no_effect");
    assert.equal(unchanged.deleted.project.open.kind, "empty");
    const staleRequest = deleteRequest("3", "018f0000-0000-7001-8000-00000000e90b");
    const staleCapture = capturingDelete(first.fetchImpl);
    const stale = await deleteOwnedWithChallenge(
      baseUrl,
      staleCapture.fetchImpl,
      first.projectId,
      chapterAId,
      "018f0000-0000-7001-8000-00000000e90a",
      staleRequest,
    );
    const staleBody = staleCapture.lastDeleteBody();
    assert.equal(stale.deleted.receipt.result, "conflicted");
    const laterDigest = await digestUpdateProject(
      renameRequest("Later Delete Chapter Title", "1", "018f0000-0000-7001-8000-00000000e90d"),
    );
    const laterChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PATCH",
        route_template: "/api/v1/projects/{project_id}",
        command_schema: "storyos.command.update-project.request.v1",
        canonical_command_digest: laterDigest,
        idempotency_key: "018f0000-0000-7001-8000-00000000e90c",
      },
    }));
    const later = await updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000e90c",
      antiForgery: laterChallenge.nonce,
      request: renameRequest("Later Delete Chapter Title", "1", "018f0000-0000-7001-8000-00000000e90d"),
    });
    assert.equal(later.project.title, "Later Delete Chapter Title");
    const laterChapter = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-00000000e90e",
      chapterRequest("Chapter C", "4", "018f0000-0000-7001-8000-00000000e90f"),
    );
    assert.equal(laterChapter.effect.kind, "authoritative_applied");
    if (laterChapter.effect.kind !== "authoritative_applied") {
      throw new Error("later Create Chapter must apply");
    }
    const frozenCapture = capturingDelete(first.fetchImpl);
    const frozen = await deleteChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId: chapterAId,
      fetchImpl: frozenCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000e906",
      antiForgery: applied.challenge.nonce,
      request: appliedRequest,
    });
    assert.deepEqual(frozen, applied.deleted);
    assert.equal(frozenCapture.lastDeleteBody(), appliedBody);
    assert.equal(frozen.project.title, "Delete Chapter Freeze Novel");
    assert.equal(frozen.project.open.kind, "empty");
    const frozenUnchangedCapture = capturingDelete(first.fetchImpl);
    const frozenUnchanged = await deleteChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId: chapterAId,
      fetchImpl: frozenUnchangedCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000e908",
      antiForgery: unchanged.challenge.nonce,
      request: unchangedRequest,
    });
    assert.deepEqual(frozenUnchanged, unchanged.deleted);
    assert.equal(frozenUnchangedCapture.lastDeleteBody(), unchangedBody);
    const frozenStaleCapture = capturingDelete(first.fetchImpl);
    const frozenStale = await deleteChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId: chapterAId,
      fetchImpl: frozenStaleCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000e90a",
      antiForgery: stale.challenge.nonce,
      request: staleRequest,
    });
    assert.deepEqual(frozenStale, stale.deleted);
    assert.equal(frozenStaleCapture.lastDeleteBody(), staleBody);
    const opened = await getProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(opened.project.title, "Later Delete Chapter Title");
    assert.equal(opened.project.open.kind, "current_chapter");
    if (opened.project.open.kind !== "current_chapter") {
      throw new Error("GET must report Chapter C");
    }
    assert.equal(opened.project.open.current_chapter_id, laterChapter.effect.chapter_id);
    const receiptCount = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'deleteChapter';
    `);
    assert.equal(receiptCount, "3");
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.authoritative_commits
         WHERE project_id = '${first.projectId}'::uuid
           AND receipt_id = '${applied.deleted.receipt.receipt_id}'::uuid;
      `),
      "1",
    );
    await stopRealServer(server);
    ({ baseUrl, server } = await startRealServer());
    const restartedFetch = browserFetch(baseUrl, "session-a");
    const afterRestartCapture = capturingDelete(restartedFetch);
    const afterRestart = await deleteChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId: chapterAId,
      fetchImpl: afterRestartCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000e906",
      antiForgery: applied.challenge.nonce,
      request: appliedRequest,
    });
    assert.deepEqual(afterRestart, applied.deleted);
    assert.equal(afterRestartCapture.lastDeleteBody(), appliedBody);
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'deleteChapter';
      `),
      receiptCount,
    );
  } finally {
    await stopRealServer(server);
  }
});

test("deleteChapter distinguishes historical absence from damaged new-format evidence", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-00000000e920",
      "Delete Chapter History Novel",
      "018f0000-0000-7001-8000-00000000e921",
    );
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000e922",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-00000000e923"),
    );
    if (volume.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const chapter = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-00000000e924",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-00000000e925"),
    );
    const chapterId = appliedId(chapter);
    const request = deleteRequest("3", "018f0000-0000-7001-8000-00000000e927");
    const applied = await deleteOwnedWithChallenge(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      chapterId,
      "018f0000-0000-7001-8000-00000000e926",
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
    const removalsBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.chapter_removal_decisions
       WHERE project_id = '${first.projectId}'::uuid;
    `);
    await queryPostgres(`
      UPDATE storyos.command_idempotency
         SET acknowledgement_format = NULL, response_project = NULL
       WHERE project_id = '${first.projectId}'::uuid
         AND idempotency_key = '018f0000-0000-7001-8000-00000000e926'::uuid;
    `);
    await assert.rejects(
      deleteChapter({
        baseUrl,
        projectId: first.projectId,
        chapterId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-00000000e926",
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
         AND idempotency_key = '018f0000-0000-7001-8000-00000000e926'::uuid;
    `);
    await assert.rejects(
      deleteChapter({
        baseUrl,
        projectId: first.projectId,
        chapterId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-00000000e926",
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
        SELECT count(*) FROM storyos.chapter_removal_decisions
         WHERE project_id = '${first.projectId}'::uuid;
      `),
      removalsBefore,
    );
  } finally {
    await stopRealServer(server);
  }
});
