import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createChapter,
  createEditorSession,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  deleteChapter,
  digestCreateChapter,
  digestCreateEditorSession,
  digestCreateVolume,
  digestDeleteChapter,
  digestUndoLatestAuthorAction,
  digestUpdateProject,
  getProject,
  undoLatestAuthorAction,
  updateProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateChapterRequest,
  CreateEditorSessionRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  DeleteChapterRequest,
  UndoLatestAuthorActionRequest,
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

function deleteRequest(expectedTreeRevision: string, correlationId: string): DeleteChapterRequest {
  return {
    command_schema: "storyos.command.delete-chapter.request.v1",
    delete_chapter_input: {
      expected_tree_revision: expectedTreeRevision,
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
      correlation_id: correlationId,
    },
  };
}

function undoRequest(options: {
  expectedFrontier: string;
  expectedRevisionId: string;
  editorSessionId: string;
  correlationId: string;
}): UndoLatestAuthorActionRequest {
  return {
    command_schema: "storyos.command.undo-latest-author-action.request.v1",
    undo_latest_author_action_input: {
      expected_author_undo_frontier_sequence: options.expectedFrontier,
      expected_authoritative_revision_id: options.expectedRevisionId,
      editor_session_id: options.editorSessionId,
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
      correlation_id: options.correlationId,
    },
  };
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

async function startRealServer() {
  return startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A },
  });
}

function capturingUndo(inner: typeof fetch): { fetchImpl: typeof fetch; lastUndoBody: () => string } {
  let lastUndoBody = "";
  return {
    fetchImpl: async (input, init) => {
      const response = await inner(input, init);
      const url = String(input instanceof Request ? input.url : input);
      if (response.ok && (init?.method ?? "GET") === "POST" && url.includes("/author-actions/undo")) {
        lastUndoBody = await response.clone().text();
      }
      return response;
    },
    lastUndoBody: () => lastUndoBody,
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

async function postUndo(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  idempotencyKey: string,
  request: UndoLatestAuthorActionRequest,
) {
  const digest = await digestUndoLatestAuthorAction(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/author-actions/undo",
      command_schema: "storyos.command.undo-latest-author-action.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const undone = await undoLatestAuthorAction({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, undone };
}

// One Project admits at most 10 Command Challenges in one 60-second window.
test("undoLatestAuthorAction freezes compensation and conflict acknowledgements after later title and Current Chapter changes", async () => {
  let { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-00000000ea00",
      "Undo Freeze Novel",
      "018f0000-0000-7001-8000-00000000ea01",
    );
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000ea02",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-00000000ea03"),
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
      "018f0000-0000-7001-8000-00000000ea04",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-00000000ea05"),
    );
    assert.equal(chapterA.effect.kind, "authoritative_applied");
    if (chapterA.effect.kind !== "authoritative_applied") {
      throw new Error("Create Chapter must apply");
    }
    const chapterAId = chapterA.effect.chapter_id;
    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
      correlation_id: "018f0000-0000-7001-8000-00000000ea06",
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
        idempotency_key: "018f0000-0000-7001-8000-00000000ea07",
      },
    }));
    const session = await createEditorSession({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000ea07",
      antiForgery: sessionChallenge.nonce,
      request: sessionRequest,
    });
    const removed = await deleteOwned(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      chapterAId,
      "018f0000-0000-7001-8000-00000000ea08",
      deleteRequest("3", "018f0000-0000-7001-8000-00000000ea09"),
    );
    assert.equal(removed.effect.kind, "authoritative_applied");
    assert.equal(removed.project.open.kind, "empty");
    if (removed.receipt.author_action_sequence === null) {
      throw new Error("Delete Chapter must allocate an Author Action");
    }
    const compensatedRequest = undoRequest({
      expectedFrontier: removed.receipt.author_action_sequence,
      expectedRevisionId: session.base_snapshot.authoritative_head_revision_id,
      editorSessionId: session.editor_session.editor_session_id,
      correlationId: "018f0000-0000-7001-8000-00000000ea0b",
    });
    const compensatedCapture = capturingUndo(first.fetchImpl);
    const compensated = await postUndo(
      baseUrl,
      compensatedCapture.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000ea0a",
      compensatedRequest,
    );
    const compensatedBody = compensatedCapture.lastUndoBody();
    assert.equal(compensated.undone.effect.kind, "compensated");
    assert.equal(compensated.undone.project.title, "Undo Freeze Novel");
    assert.equal(compensated.undone.project.open.kind, "current_chapter");
    if (compensated.undone.project.open.kind !== "current_chapter") {
      throw new Error("Undo must restore Chapter A");
    }
    assert.equal(compensated.undone.project.open.current_chapter_id, chapterAId);
    const staleRequest = undoRequest({
      expectedFrontier: removed.receipt.author_action_sequence,
      expectedRevisionId: session.base_snapshot.authoritative_head_revision_id,
      editorSessionId: session.editor_session.editor_session_id,
      correlationId: "018f0000-0000-7001-8000-00000000ea0d",
    });
    const staleCapture = capturingUndo(first.fetchImpl);
    const stale = await postUndo(
      baseUrl,
      staleCapture.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000ea0c",
      staleRequest,
    );
    const staleBody = staleCapture.lastUndoBody();
    assert.equal(stale.undone.effect.kind, "conflicted");
    assert.equal(stale.undone.project.title, "Undo Freeze Novel");
    const laterDigest = await digestUpdateProject(
      renameRequest("Later Undo Title", "1", "018f0000-0000-7001-8000-00000000ea0f"),
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
        idempotency_key: "018f0000-0000-7001-8000-00000000ea0e",
      },
    }));
    const later = await updateProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000ea0e",
      antiForgery: laterChallenge.nonce,
      request: renameRequest("Later Undo Title", "1", "018f0000-0000-7001-8000-00000000ea0f"),
    });
    assert.equal(later.project.title, "Later Undo Title");
    const laterChapter = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-00000000ea10",
      chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-00000000ea11"),
    );
    assert.equal(laterChapter.effect.kind, "authoritative_applied");
    if (laterChapter.effect.kind !== "authoritative_applied") {
      throw new Error("later Create Chapter must apply");
    }
    const frozenCapture = capturingUndo(first.fetchImpl);
    const frozen = await undoLatestAuthorAction({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: frozenCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000ea0a",
      antiForgery: compensated.challenge.nonce,
      request: compensatedRequest,
    });
    assert.deepEqual(frozen, compensated.undone);
    assert.equal(frozenCapture.lastUndoBody(), compensatedBody);
    assert.equal(frozen.project.title, "Undo Freeze Novel");
    if (frozen.project.open.kind !== "current_chapter") {
      throw new Error("retry must restore Chapter A");
    }
    assert.equal(frozen.project.open.current_chapter_id, chapterAId);
    const frozenStaleCapture = capturingUndo(first.fetchImpl);
    const frozenStale = await undoLatestAuthorAction({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: frozenStaleCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000ea0c",
      antiForgery: stale.challenge.nonce,
      request: staleRequest,
    });
    assert.deepEqual(frozenStale, stale.undone);
    assert.equal(frozenStaleCapture.lastUndoBody(), staleBody);
    const opened = await getProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(opened.project.title, "Later Undo Title");
    assert.equal(opened.project.open.kind, "current_chapter");
    if (opened.project.open.kind !== "current_chapter") {
      throw new Error("GET must report Chapter B");
    }
    assert.equal(opened.project.open.current_chapter_id, laterChapter.effect.chapter_id);
    const receiptCount = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'undoLatestAuthorAction';
    `);
    assert.equal(receiptCount, "2");
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.author_action_entries
         WHERE project_id = '${first.projectId}'::uuid
           AND receipt_id = '${compensated.undone.receipt.receipt_id}'::uuid
           AND disposition = 'compensation';
      `),
      "1",
    );
    await stopRealServer(server);
    ({ baseUrl, server } = await startRealServer());
    const restartedFetch = browserFetch(baseUrl, "session-a");
    const afterRestartCapture = capturingUndo(restartedFetch);
    const afterRestart = await undoLatestAuthorAction({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: afterRestartCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000ea0a",
      antiForgery: compensated.challenge.nonce,
      request: compensatedRequest,
    });
    assert.deepEqual(afterRestart, compensated.undone);
    assert.equal(afterRestartCapture.lastUndoBody(), compensatedBody);
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'undoLatestAuthorAction';
      `),
      receiptCount,
    );
  } finally {
    await stopRealServer(server);
  }
});

test("undoLatestAuthorAction distinguishes historical absence from damaged new-format evidence", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-00000000ea20",
      "Undo History Novel",
      "018f0000-0000-7001-8000-00000000ea21",
    );
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000ea22",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-00000000ea23"),
    );
    if (volume.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const chapter = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volume.effect.volume_id,
      "018f0000-0000-7001-8000-00000000ea24",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-00000000ea25"),
    );
    if (chapter.effect.kind !== "authoritative_applied") {
      throw new Error("Create Chapter must apply");
    }
    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      client_contract_revision: CLIENT,
      security_policy_revision: SECURITY,
      correlation_id: "018f0000-0000-7001-8000-00000000ea26",
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
        idempotency_key: "018f0000-0000-7001-8000-00000000ea27",
      },
    }));
    const session = await createEditorSession({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000ea27",
      antiForgery: sessionChallenge.nonce,
      request: sessionRequest,
    });
    const removed = await deleteOwned(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      chapter.effect.chapter_id,
      "018f0000-0000-7001-8000-00000000ea28",
      deleteRequest("3", "018f0000-0000-7001-8000-00000000ea29"),
    );
    if (removed.receipt.author_action_sequence === null) {
      throw new Error("Delete Chapter must allocate an Author Action");
    }
    const request = undoRequest({
      expectedFrontier: removed.receipt.author_action_sequence,
      expectedRevisionId: session.base_snapshot.authoritative_head_revision_id,
      editorSessionId: session.editor_session.editor_session_id,
      correlationId: "018f0000-0000-7001-8000-00000000ea2b",
    });
    const applied = await postUndo(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000ea2a",
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
    const compensationsBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.author_action_entries
       WHERE project_id = '${first.projectId}'::uuid AND disposition = 'compensation';
    `);
    await queryPostgres(`
      UPDATE storyos.command_idempotency
         SET acknowledgement_format = NULL, response_project = NULL
       WHERE project_id = '${first.projectId}'::uuid
         AND idempotency_key = '018f0000-0000-7001-8000-00000000ea2a'::uuid;
    `);
    await assert.rejects(
      undoLatestAuthorAction({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-00000000ea2a",
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
         AND idempotency_key = '018f0000-0000-7001-8000-00000000ea2a'::uuid;
    `);
    await assert.rejects(
      undoLatestAuthorAction({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-00000000ea2a",
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
        SELECT count(*) FROM storyos.author_action_entries
         WHERE project_id = '${first.projectId}'::uuid AND disposition = 'compensation';
      `),
      compensationsBefore,
    );
  } finally {
    await stopRealServer(server);
  }
});
