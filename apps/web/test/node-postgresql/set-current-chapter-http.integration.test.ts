import assert from "node:assert/strict";
import { unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
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
  digestUpdateProject,
  getChapter,
  getEditorSession,
  getProject,
  setCurrentChapter,
  updateProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateChapterRequest,
  CreateEditorSessionRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  SetCurrentChapterRequest,
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

async function startRealServer(extraEnv?: Readonly<Record<string, string>>) {
  return extraEnv === undefined
    ? startStoryOSServer({
        repositoryRoot,
        serverBinary,
        sessions: { "session-a": USER_A, "session-b": USER_B },
      })
    : startStoryOSServer({
        repositoryRoot,
        serverBinary,
        sessions: { "session-a": USER_A, "session-b": USER_B },
        extraEnv,
      });
}

function capturingCurrent(inner: typeof fetch): { fetchImpl: typeof fetch; lastPutBody: () => string } {
  let lastPutBody = "";
  return {
    fetchImpl: async (input, init) => {
      const response = await inner(input, init);
      const url = String(input instanceof Request ? input.url : input);
      if (response.ok && (init?.method ?? "GET") === "PUT" && url.includes("/current-chapter")) {
        lastPutBody = await response.clone().text();
      }
      return response;
    },
    lastPutBody: () => lastPutBody,
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

async function waitForSettledKey(projectId: string, idempotencyKey: string) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    const outcome = await queryPostgres(`
      SELECT outcome_kind FROM storyos.command_idempotency
       WHERE project_id = '${projectId}'::uuid
         AND command_kind = 'setCurrentChapter'
         AND idempotency_key = '${idempotencyKey}'::uuid;
    `);
    if (outcome === "settled") return;
    await new Promise((resolve) => {
      setTimeout(resolve, 10);
    });
  }
  throw new Error(`Set Current Chapter ${idempotencyKey} did not settle`);
}

function renameRequest(title: string, expectedProjectRevision: string, correlationId: string): UpdateProjectRequest {
  return {
    command_schema: "storyos.command.update-project.request.v1",
    update_project_input: {
      title,
      expected_project_revision: expectedProjectRevision,
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
      correlation_id: correlationId,
    },
  };
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

// One Project admits at most PROJECT_COMMAND_CHALLENGE_RATE_CAPACITY (10) Command
// Challenges in one 60-second window. The eleventh waits for the next window, so each
// test in this file stays at or under 10 Command Challenges on one Project.
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
    assert.equal(applied.switched.receipt.author_action_sequence, "4");
    assert.deepEqual(applied.switched.receipt.authoritative_commit_ids, []);
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
    assert.equal(stale.switched.receipt.author_action_sequence, null);

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
    assert.equal(wrong.switched.receipt.author_action_sequence, null);

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
    assert.equal(already.switched.receipt.author_action_sequence, null);

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

test("setCurrentChapter refuses a missing Chapter join and an archived Project", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const owned = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000000f30",
      "Archive Then Switch",
      "018f0000-0000-7001-8000-000000000f31",
    );
    const volume = await postVolume(
      baseUrl,
      owned.fetchImpl,
      owned.projectId,
      "018f0000-0000-7001-8000-000000000f32",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000f33"),
    );
    if (volume.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const chapterA = await postChapter(
      baseUrl,
      owned.fetchImpl,
      owned.projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-000000000f34",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-000000000f35"),
    );
    const chapterB = await postChapter(
      baseUrl,
      owned.fetchImpl,
      owned.projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-000000000f36",
      chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-000000000f37"),
    );
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
      correlation_id: "018f0000-0000-7001-8000-000000000f38",
    };
    const sessionDigest = await digestCreateEditorSession(sessionRequest);
    const sessionChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: owned.projectId,
      fetchImpl: owned.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/editor-sessions",
        command_schema: sessionRequest.command_schema,
        canonical_command_digest: sessionDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000f39",
      },
    }));
    const session = await createEditorSession({
      baseUrl,
      projectId: owned.projectId,
      fetchImpl: owned.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f39",
      antiForgery: sessionChallenge.nonce,
      request: sessionRequest,
    });
    const editorSessionId = session.editor_session.editor_session_id;
    const openedB = await getChapter({
      baseUrl,
      projectId: owned.projectId,
      chapterId: chapterBId,
      fetchImpl: owned.fetchImpl,
    });
    const revisionB = openedB.chapter.current_revision.revision_id;

    const invalidJoin = await putCurrent(
      baseUrl,
      owned.fetchImpl,
      owned.projectId,
      "018f0000-0000-7001-8000-000000000f3a",
      currentRequest({
        chapterId: MISSING_CHAPTER,
        expectedCurrentChapterId: chapterAId,
        expectedTargetRevisionId: revisionB,
        editorSessionId,
        correlationId: "018f0000-0000-7001-8000-000000000f3b",
      }),
    );
    assert.equal(invalidJoin.switched.effect.kind, "refused");
    if (invalidJoin.switched.effect.kind !== "refused") {
      throw new Error("missing Chapter must refuse");
    }
    assert.equal(invalidJoin.switched.effect.reason, "invalid_chapter_join");

    const archiveDigest = await digestArchiveProject(
      archiveRequest("1", "018f0000-0000-7001-8000-000000000f3c"),
    );
    const archiveChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: owned.projectId,
      fetchImpl: owned.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: archiveDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000f3d",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId: owned.projectId,
      fetchImpl: owned.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f3d",
      antiForgery: archiveChallenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-000000000f3c"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");
    const refused = await putCurrent(
      baseUrl,
      owned.fetchImpl,
      owned.projectId,
      "018f0000-0000-7001-8000-000000000f3e",
      currentRequest({
        chapterId: chapterBId,
        expectedCurrentChapterId: chapterAId,
        expectedTargetRevisionId: revisionB,
        editorSessionId,
        correlationId: "018f0000-0000-7001-8000-000000000f3f",
      }),
    );
    assert.equal(refused.switched.effect.kind, "refused");
    if (refused.switched.effect.kind !== "refused") {
      throw new Error("archived Project must refuse");
    }
    assert.equal(refused.switched.effect.reason, "archived_project");
  } finally {
    await stopRealServer(server);
  }
});

async function prepareTwoChapterProject(
  baseUrl: string,
  keys: {
    create: string;
    createCorrelation: string;
    volume: string;
    volumeCorrelation: string;
    chapterA: string;
    chapterACorrelation: string;
    chapterB: string;
    chapterBCorrelation: string;
    session: string;
    sessionCorrelation: string;
  },
) {
  const first = await createEmpty(baseUrl, "session-a", keys.create, "Current Freeze Novel", keys.createCorrelation);
  const volume = await postVolume(
    baseUrl,
    first.fetchImpl,
    first.projectId,
    keys.volume,
    volumeRequest("Volume A", "1", keys.volumeCorrelation),
  );
  if (volume.effect.kind !== "authoritative_applied") {
    throw new Error("Create Volume must apply");
  }
  const chapterA = await postChapter(
    baseUrl,
    first.fetchImpl,
    first.projectId,
    volume.effect.volume_id,
    keys.chapterA,
    chapterRequest("Chapter A", "2", keys.chapterACorrelation),
  );
  const chapterB = await postChapter(
    baseUrl,
    first.fetchImpl,
    first.projectId,
    volume.effect.volume_id,
    keys.chapterB,
    chapterRequest("Chapter B", "3", keys.chapterBCorrelation),
  );
  if (chapterA.effect.kind !== "authoritative_applied"
    || chapterB.effect.kind !== "authoritative_applied") {
    throw new Error("both Chapters must apply");
  }
  const sessionRequest: CreateEditorSessionRequest = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    client_contract_revision: CLIENT,
    security_policy_revision: SECURITY,
    correlation_id: keys.sessionCorrelation,
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
      idempotency_key: keys.session,
    },
  }));
  const session = await createEditorSession({
    baseUrl,
    projectId: first.projectId,
    fetchImpl: first.fetchImpl,
    idempotencyKey: keys.session,
    antiForgery: sessionChallenge.nonce,
    request: sessionRequest,
  });
  const openedB = await getChapter({
    baseUrl,
    projectId: first.projectId,
    chapterId: chapterB.effect.chapter_id,
    fetchImpl: first.fetchImpl,
  });
  return {
    fetchImpl: first.fetchImpl,
    projectId: first.projectId,
    chapterAId: chapterA.effect.chapter_id,
    chapterBId: chapterB.effect.chapter_id,
    editorSessionId: session.editor_session.editor_session_id,
    revisionB: openedB.chapter.current_revision.revision_id,
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
      command_schema: "storyos.command.update-project.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  return updateProject({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
}

test("setCurrentChapter freezes the selected chapter and title", async () => {
  let { baseUrl, server } = await startRealServer();
  try {
    const prepared = await prepareTwoChapterProject(baseUrl, {
      create: "018f0000-0000-7001-8000-000000000f70",
      createCorrelation: "018f0000-0000-7001-8000-000000000f71",
      volume: "018f0000-0000-7001-8000-000000000f72",
      volumeCorrelation: "018f0000-0000-7001-8000-000000000f73",
      chapterA: "018f0000-0000-7001-8000-000000000f74",
      chapterACorrelation: "018f0000-0000-7001-8000-000000000f75",
      chapterB: "018f0000-0000-7001-8000-000000000f76",
      chapterBCorrelation: "018f0000-0000-7001-8000-000000000f77",
      session: "018f0000-0000-7001-8000-000000000f78",
      sessionCorrelation: "018f0000-0000-7001-8000-000000000f79",
    });
    const requestA = currentRequest({
      chapterId: prepared.chapterBId,
      expectedCurrentChapterId: prepared.chapterAId,
      expectedTargetRevisionId: prepared.revisionB,
      editorSessionId: prepared.editorSessionId,
      correlationId: "018f0000-0000-7001-8000-000000000f7a",
    });
    const firstCapture = capturingCurrent(prepared.fetchImpl);
    const first = await putCurrent(
      baseUrl,
      firstCapture.fetchImpl,
      prepared.projectId,
      "018f0000-0000-7001-8000-000000000f7b",
      requestA,
    );
    const firstBody = firstCapture.lastPutBody();
    assert.equal(first.switched.effect.kind, "authoritative_applied");
    assert.equal(first.switched.project.open.kind, "current_chapter");
    if (first.switched.project.open.kind !== "current_chapter") {
      throw new Error("first acknowledgement must name Chapter B");
    }
    assert.equal(first.switched.project.open.current_chapter_id, prepared.chapterBId);
    assert.equal(first.switched.project.title, "Current Freeze Novel");
    const openedA = await getChapter({
      baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterAId,
      fetchImpl: prepared.fetchImpl,
    });
    const laterSwitch = await putCurrent(
      baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "018f0000-0000-7001-8000-000000000f7c",
      currentRequest({
        chapterId: prepared.chapterAId,
        expectedCurrentChapterId: prepared.chapterBId,
        expectedTargetRevisionId: openedA.chapter.current_revision.revision_id,
        editorSessionId: prepared.editorSessionId,
        correlationId: "018f0000-0000-7001-8000-000000000f7d",
      }),
    );
    assert.equal(laterSwitch.switched.effect.kind, "authoritative_applied");
    const laterRename = await renameProject(
      baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "018f0000-0000-7001-8000-000000000f7e",
      renameRequest("Later Current Title", "1", "018f0000-0000-7001-8000-000000000f7f"),
    );
    assert.equal(laterRename.project.title, "Later Current Title");
    const opened = await getProject({
      baseUrl,
      projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(opened.project.title, "Later Current Title");
    assert.equal(opened.project.open.kind, "current_chapter");
    if (opened.project.open.kind !== "current_chapter") {
      throw new Error("GET must report Chapter A");
    }
    assert.equal(opened.project.open.current_chapter_id, prepared.chapterAId);
    const frozenCapture = capturingCurrent(prepared.fetchImpl);
    const frozen = await setCurrentChapter({
      baseUrl,
      projectId: prepared.projectId,
      fetchImpl: frozenCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f7b",
      antiForgery: first.challenge.nonce,
      request: requestA,
    });
    assert.deepEqual(frozen, first.switched);
    assert.equal(frozenCapture.lastPutBody(), firstBody);
    assert.equal(frozen.project.title, "Current Freeze Novel");
    assert.equal(frozen.project.open.kind, "current_chapter");
    if (frozen.project.open.kind !== "current_chapter") {
      throw new Error("retry must keep Chapter B");
    }
    assert.equal(frozen.project.open.current_chapter_id, prepared.chapterBId);
    const receiptCount = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${prepared.projectId}'::uuid AND command_kind = 'setCurrentChapter';
    `);
    assert.equal(receiptCount, "2");
    await stopRealServer(server);
    ({ baseUrl, server } = await startRealServer());
    const restartedFetch = browserFetch(baseUrl, "session-a");
    const afterRestartCapture = capturingCurrent(restartedFetch);
    const afterRestart = await setCurrentChapter({
      baseUrl,
      projectId: prepared.projectId,
      fetchImpl: afterRestartCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000f7b",
      antiForgery: first.challenge.nonce,
      request: requestA,
    });
    assert.deepEqual(afterRestart, first.switched);
    assert.equal(afterRestartCapture.lastPutBody(), firstBody);
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${prepared.projectId}'::uuid AND command_kind = 'setCurrentChapter';
      `),
      receiptCount,
    );
  } finally {
    await stopRealServer(server);
  }
});

test("setCurrentChapter first acknowledgement excludes a later selection", async () => {
  const heldKey = "018f0000-0000-7001-8000-000000000f8b";
  const holdPath = join(tmpdir(), `storyos-ack-hold-${heldKey}.flag`);
  writeFileSync(holdPath, "hold");
  const { baseUrl, server } = await startRealServer({
    STORYOS_TEST_ACK_HOLD_PATH: holdPath,
    STORYOS_TEST_ACK_HOLD_IDEMPOTENCY_KEY: heldKey,
  });
  try {
    const prepared = await prepareTwoChapterProject(baseUrl, {
      create: "018f0000-0000-7001-8000-000000000f80",
      createCorrelation: "018f0000-0000-7001-8000-000000000f81",
      volume: "018f0000-0000-7001-8000-000000000f82",
      volumeCorrelation: "018f0000-0000-7001-8000-000000000f83",
      chapterA: "018f0000-0000-7001-8000-000000000f84",
      chapterACorrelation: "018f0000-0000-7001-8000-000000000f85",
      chapterB: "018f0000-0000-7001-8000-000000000f86",
      chapterBCorrelation: "018f0000-0000-7001-8000-000000000f87",
      session: "018f0000-0000-7001-8000-000000000f88",
      sessionCorrelation: "018f0000-0000-7001-8000-000000000f89",
    });
    const requestA = currentRequest({
      chapterId: prepared.chapterBId,
      expectedCurrentChapterId: prepared.chapterAId,
      expectedTargetRevisionId: prepared.revisionB,
      editorSessionId: prepared.editorSessionId,
      correlationId: "018f0000-0000-7001-8000-000000000f8a",
    });
    const digestA = await digestSetCurrentChapter(requestA);
    const challengeA = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/current-chapter",
        command_schema: "storyos.command.set-current-chapter.request.v1",
        canonical_command_digest: digestA,
        idempotency_key: heldKey,
      },
    }));
    const firstAck = setCurrentChapter({
      baseUrl,
      projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: heldKey,
      antiForgery: challengeA.nonce,
      request: requestA,
    });
    await waitForSettledKey(prepared.projectId, heldKey);
    const openedA = await getChapter({
      baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterAId,
      fetchImpl: prepared.fetchImpl,
    });
    const later = await putCurrent(
      baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "018f0000-0000-7001-8000-000000000f8c",
      currentRequest({
        chapterId: prepared.chapterAId,
        expectedCurrentChapterId: prepared.chapterBId,
        expectedTargetRevisionId: openedA.chapter.current_revision.revision_id,
        editorSessionId: prepared.editorSessionId,
        correlationId: "018f0000-0000-7001-8000-000000000f8d",
      }),
    );
    assert.equal(later.switched.effect.kind, "authoritative_applied");
    unlinkSync(holdPath);
    const held = await firstAck;
    assert.equal(held.project.open.kind, "current_chapter");
    if (held.project.open.kind !== "current_chapter") {
      throw new Error("held acknowledgement must name Chapter B");
    }
    assert.equal(held.project.open.current_chapter_id, prepared.chapterBId);
    const opened = await getProject({
      baseUrl,
      projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(opened.project.open.kind, "current_chapter");
    if (opened.project.open.kind !== "current_chapter") {
      throw new Error("GET must report Chapter A");
    }
    assert.equal(opened.project.open.current_chapter_id, prepared.chapterAId);
    const retried = await setCurrentChapter({
      baseUrl,
      projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: heldKey,
      antiForgery: challengeA.nonce,
      request: requestA,
    });
    assert.deepEqual(retried, held);
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${prepared.projectId}'::uuid AND command_kind = 'setCurrentChapter';
      `),
      "2",
    );
  } finally {
    try {
      unlinkSync(holdPath);
    } catch {
      // The race test deletes the hold file on the success path.
    }
    await stopRealServer(server);
  }
});

test("setCurrentChapter distinguishes historical absence from damaged new-format evidence", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const prepared = await prepareTwoChapterProject(baseUrl, {
      create: "018f0000-0000-7001-8000-000000000f90",
      createCorrelation: "018f0000-0000-7001-8000-000000000f91",
      volume: "018f0000-0000-7001-8000-000000000f92",
      volumeCorrelation: "018f0000-0000-7001-8000-000000000f93",
      chapterA: "018f0000-0000-7001-8000-000000000f94",
      chapterACorrelation: "018f0000-0000-7001-8000-000000000f95",
      chapterB: "018f0000-0000-7001-8000-000000000f96",
      chapterBCorrelation: "018f0000-0000-7001-8000-000000000f97",
      session: "018f0000-0000-7001-8000-000000000f98",
      sessionCorrelation: "018f0000-0000-7001-8000-000000000f99",
    });
    const requestA = currentRequest({
      chapterId: prepared.chapterBId,
      expectedCurrentChapterId: prepared.chapterAId,
      expectedTargetRevisionId: prepared.revisionB,
      editorSessionId: prepared.editorSessionId,
      correlationId: "018f0000-0000-7001-8000-000000000f9a",
    });
    const first = await putCurrent(
      baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "018f0000-0000-7001-8000-000000000f9b",
      requestA,
    );
    assert.equal(first.switched.project.open.kind, "current_chapter");
    const novelsBefore = await queryPostgres(`
      SELECT title || ' ' || count(*)::text FROM storyos.projects
       WHERE project_id = '${prepared.projectId}'::uuid GROUP BY title;
    `);
    const receiptsBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${prepared.projectId}'::uuid;
    `);
    const keysBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.command_idempotency
       WHERE project_id = '${prepared.projectId}'::uuid AND command_kind = 'setCurrentChapter';
    `);
    await queryPostgres(`
      UPDATE storyos.command_idempotency
         SET acknowledgement_format = NULL, response_project = NULL
       WHERE project_id = '${prepared.projectId}'::uuid
         AND idempotency_key = '018f0000-0000-7001-8000-000000000f9b'::uuid;
    `);
    await assert.rejects(
      setCurrentChapter({
        baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000f9b",
        antiForgery: first.challenge.nonce,
        request: requestA,
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
       WHERE project_id = '${prepared.projectId}'::uuid
         AND idempotency_key = '018f0000-0000-7001-8000-000000000f9b'::uuid;
    `);
    await assert.rejects(
      setCurrentChapter({
        baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000f9b",
        antiForgery: first.challenge.nonce,
        request: requestA,
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
         WHERE project_id = '${prepared.projectId}'::uuid GROUP BY title;
      `),
      novelsBefore,
    );
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${prepared.projectId}'::uuid;
      `),
      receiptsBefore,
    );
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.command_idempotency
         WHERE project_id = '${prepared.projectId}'::uuid AND command_kind = 'setCurrentChapter';
      `),
      keysBefore,
    );
  } finally {
    await stopRealServer(server);
  }
});
