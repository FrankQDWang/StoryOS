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
  getChapter,
  getEditorSession,
  getProject,
  setCurrentChapter,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateChapterRequest,
  CreateEditorSessionRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  SetCurrentChapterRequest,
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
const WRONG_HEAD = "018f0000-0000-7001-8000-00000000fffe";
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const CLIENT = RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision;
const SECURITY = "storyos.web-security-policy.release-1.v1";

function createChallengeRequest(idempotencyKey: string, title: string, correlationId: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
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
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
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
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
      correlation_id: correlationId,
    },
  };
}

function currentRequest(options: {
  chapterId: string;
  expectedCurrentChapterId: string;
  expectedTargetRevisionId: string;
  editorSessionId: string;
  correlationId: string;
}): SetCurrentChapterRequest {
  return {
    command_schema: "storyos.command.set-current-chapter.request.v1",
    set_current_chapter_input: {
      chapter_id: options.chapterId,
      expected_current_chapter_id: options.expectedCurrentChapterId,
      expected_target_revision_id: options.expectedTargetRevisionId,
      editor_session_id: options.editorSessionId,
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
      correlation_id: options.correlationId,
    },
  };
}

function archiveRequest(expectedProjectRevision: string, correlationId: string): ArchiveProjectRequest {
  return {
    command_schema: "storyos.command.archive-project.request.v1",
    archive_project_input: {
      expected_project_revision: expectedProjectRevision,
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
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

async function putCurrent(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  idempotencyKey: string,
  request: SetCurrentChapterRequest,
) {
  const digest = await digestSetCurrentChapter(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "PUT",
      route_template: "/api/v1/projects/{project_id}/current-chapter",
      command_schema: "storyos.command.set-current-chapter.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const switched = await setCurrentChapter({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, switched };
}

test("setCurrentChapter switches the current Chapter, replays, and fails closed", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000000f10",
      "Current Chapter Novel",
      "018f0000-0000-7001-8000-000000000f11",
    );
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000f12",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000f13"),
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
      "018f0000-0000-7001-8000-000000000f14",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-000000000f15"),
    );
    const chapterB = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-000000000f16",
      chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-000000000f17"),
    );
    assert.equal(chapterA.effect.kind, "authoritative_applied");
    assert.equal(chapterB.effect.kind, "authoritative_applied");
    if (chapterA.effect.kind !== "authoritative_applied"
      || chapterB.effect.kind !== "authoritative_applied") {
      throw new Error("both Chapters must apply");
    }
    const chapterAId = chapterA.effect.chapter_id;
    const chapterBId = chapterB.effect.chapter_id;
    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
      correlation_id: "018f0000-0000-7001-8000-000000000f18",
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
        idempotency_key: "018f0000-0000-7001-8000-000000000f19",
      },
    }));
    const session = await createEditorSession({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f19",
      antiForgery: sessionChallenge.nonce,
      request: sessionRequest,
    });
    const editorSessionId = session.editor_session.editor_session_id;
    const openedB = await getChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId: chapterBId,
      fetchImpl: first.fetchImpl,
    });
    const revisionB = openedB.chapter.current_revision.revision_id;
    const switchRequest = currentRequest({
      chapterId: chapterBId,
      expectedCurrentChapterId: chapterAId,
      expectedTargetRevisionId: revisionB,
      editorSessionId,
      correlationId: "018f0000-0000-7001-8000-000000000f1a",
    });
    const applied = await putCurrent(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000f1b",
      switchRequest,
    );
    assert.equal(applied.switched.schema_id, "storyos.command.set-current-chapter.response.v1");
    assert.equal(applied.switched.receipt.command_kind, "setCurrentChapter");
    assert.equal(applied.switched.receipt.result, "authoritative_applied");
    assert.equal(applied.switched.effect.kind, "authoritative_applied");
    if (applied.switched.effect.kind !== "authoritative_applied") {
      throw new Error("Set Current Chapter must apply");
    }
    assert.equal(applied.switched.effect.current_chapter_id, chapterBId);
    assert.match(applied.switched.effect.base_snapshot_id, UUID_V7);
    assert.equal(applied.switched.project.open.kind, "current_chapter");
    if (applied.switched.project.open.kind !== "current_chapter") {
      throw new Error("the Project must name the new current Chapter");
    }
    assert.equal(applied.switched.project.open.current_chapter_id, chapterBId);
    const replay = await setCurrentChapter({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f1b",
      antiForgery: applied.challenge.nonce,
      request: switchRequest,
    });
    assert.deepEqual(replay, applied.switched);
    const project = await getProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(project.project.open.kind, "current_chapter");
    if (project.project.open.kind !== "current_chapter") {
      throw new Error("getProject must name the new current Chapter");
    }
    assert.equal(project.project.open.current_chapter_id, chapterBId);
    const editor = await getEditorSession({
      baseUrl,
      projectId: first.projectId,
      editorSessionId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(editor.base_snapshot.chapter_id, chapterBId);
    assert.equal(editor.base_snapshot.authoritative_head_revision_id, revisionB);

    const stale = await putCurrent(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000f1c",
      currentRequest({
        chapterId: chapterAId,
        expectedCurrentChapterId: chapterAId,
        expectedTargetRevisionId: revisionB,
        editorSessionId,
        correlationId: "018f0000-0000-7001-8000-000000000f1d",
      }),
    );
    assert.equal(stale.switched.effect.kind, "conflicted");
    if (stale.switched.effect.kind !== "conflicted") {
      throw new Error("stale current Chapter must conflict");
    }
    assert.equal(stale.switched.effect.reason, "stale_current_chapter");

    const wrong = await putCurrent(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000f1e",
      currentRequest({
        chapterId: chapterAId,
        expectedCurrentChapterId: chapterBId,
        expectedTargetRevisionId: WRONG_HEAD,
        editorSessionId,
        correlationId: "018f0000-0000-7001-8000-000000000f1f",
      }),
    );
    assert.equal(wrong.switched.effect.kind, "conflicted");
    if (wrong.switched.effect.kind !== "conflicted") {
      throw new Error("wrong target Head must conflict");
    }
    assert.equal(wrong.switched.effect.reason, "wrong_target_head");

    const already = await putCurrent(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000f20",
      currentRequest({
        chapterId: chapterBId,
        expectedCurrentChapterId: chapterBId,
        expectedTargetRevisionId: revisionB,
        editorSessionId,
        correlationId: "018f0000-0000-7001-8000-000000000f21",
      }),
    );
    assert.equal(already.switched.effect.kind, "no_effect");
    if (already.switched.effect.kind !== "no_effect") {
      throw new Error("already-current must have no effect");
    }
    assert.equal(already.switched.effect.reason, "already_current");

    const invalidJoin = await putCurrent(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000f22",
      currentRequest({
        chapterId: MISSING_CHAPTER,
        expectedCurrentChapterId: chapterBId,
        expectedTargetRevisionId: revisionB,
        editorSessionId,
        correlationId: "018f0000-0000-7001-8000-000000000f23",
      }),
    );
    assert.equal(invalidJoin.switched.effect.kind, "refused");
    if (invalidJoin.switched.effect.kind !== "refused") {
      throw new Error("missing Chapter must refuse");
    }
    assert.equal(invalidJoin.switched.effect.reason, "invalid_chapter_join");

    const archiveDigest = await digestArchiveProject(
      archiveRequest("1", "018f0000-0000-7001-8000-000000000f24"),
    );
    const archiveChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: archiveDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000f25",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f25",
      antiForgery: archiveChallenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-000000000f24"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");
    const refused = await putCurrent(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000f26",
      currentRequest({
        chapterId: chapterAId,
        expectedCurrentChapterId: chapterBId,
        expectedTargetRevisionId: revisionB,
        editorSessionId,
        correlationId: "018f0000-0000-7001-8000-000000000f27",
      }),
    );
    assert.equal(refused.switched.effect.kind, "refused");
    if (refused.switched.effect.kind !== "refused") {
      throw new Error("archived Project must refuse");
    }
    assert.equal(refused.switched.effect.reason, "archived_project");

    const foreign = await createEmpty(
      baseUrl,
      "session-b",
      "018f0000-0000-7001-8000-000000000f28",
      "Other Novel",
      "018f0000-0000-7001-8000-000000000f29",
    );
    await assert.rejects(
      putCurrent(
        baseUrl,
        foreign.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000f2a",
        currentRequest({
          chapterId: chapterBId,
          expectedCurrentChapterId: chapterAId,
          expectedTargetRevisionId: revisionB,
          editorSessionId,
          correlationId: "018f0000-0000-7001-8000-000000000f2b",
        }),
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
