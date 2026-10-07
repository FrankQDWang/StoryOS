// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/delete-volume-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  acceptProposal,
  createChapter,
  createEditorSession,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  deleteChapter,
  digestAcceptProposal,
  digestCreateChapter,
  digestCreateEditorSession,
  digestCreateVolume,
  digestDeleteChapter,
  digestRejectProposalOperations,
  digestReopenRejectedOperations,
  digestReopenWithdrawnProposal,
  digestReplanProposal,
  digestSetCurrentChapter,
  digestUndoLatestAuthorAction,
  digestUpdateProject,
  digestWithdrawProposal,
  getChapter,
  getProject,
  getProposal,
  rejectProposalOperations,
  reopenRejectedOperations,
  reopenWithdrawnProposal,
  replanProposal,
  setCurrentChapter,
  undoLatestAuthorAction,
  updateProject,
  withdrawProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  AcceptProposalRequest,
  CreateChapterRequest,
  CreateEditorSessionRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  DeleteChapterRequest,
  GetProposalResponse,
  RejectProposalOperationsRequest,
  ReopenRejectedOperationsRequest,
  ReopenWithdrawnProposalRequest,
  ReplanProposalRequest,
  SetCurrentChapterRequest,
  UndoLatestAuthorActionRequest,
  UpdateProjectRequest,
  WithdrawProposalRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeBudget,
} from "../support/node-integration.ts";
import {
  BINDING,
  admitProse,
  challenged,
  drainLeftoverWork,
  id,
  prepare,
  reviseCandidate,
  startRealServer as startProposalServer,
} from "../support/acceptance.ts";

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
  const challenge = await withChallengeBudget(projectId, () => createProjectCommandChallenge({
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
  const challenge = await withChallengeBudget(projectId, () => createProjectCommandChallenge({
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
  const challenge = await withChallengeBudget(projectId, () => createProjectCommandChallenge({
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
  const challenge = await withChallengeBudget(projectId, () => createProjectCommandChallenge({
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
    const sessionChallenge = await withChallengeBudget(first.projectId, () => createProjectCommandChallenge({
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
    const laterChallenge = await withChallengeBudget(first.projectId, () => createProjectCommandChallenge({
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
    const laterChapterId = laterChapter.effect.chapter_id;
    const openedLater = await getChapter({
      baseUrl,
      projectId: first.projectId,
      chapterId: laterChapterId,
      fetchImpl: first.fetchImpl,
    });
    const laterCurrentDigest = await digestSetCurrentChapter(currentRequest({
      chapterId: laterChapterId,
      expectedCurrentChapterId: chapterAId,
      expectedTargetRevisionId: openedLater.chapter.current_revision.revision_id,
      editorSessionId: session.editor_session.editor_session_id,
      correlationId: "018f0000-0000-7001-8000-00000000ea13",
    }));
    const laterCurrentChallenge = await withChallengeBudget(first.projectId, () => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/current-chapter",
        command_schema: "storyos.command.set-current-chapter.request.v1",
        canonical_command_digest: laterCurrentDigest,
        idempotency_key: "018f0000-0000-7001-8000-00000000ea12",
      },
    }));
    const laterCurrent = await setCurrentChapter({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000ea12",
      antiForgery: laterCurrentChallenge.nonce,
      request: currentRequest({
        chapterId: laterChapterId,
        expectedCurrentChapterId: chapterAId,
        expectedTargetRevisionId: openedLater.chapter.current_revision.revision_id,
        editorSessionId: session.editor_session.editor_session_id,
        correlationId: "018f0000-0000-7001-8000-00000000ea13",
      }),
    });
    assert.equal(laterCurrent.effect.kind, "authoritative_applied");
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
    assert.equal(opened.project.open.current_chapter_id, laterChapterId);
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
    const sessionChallenge = await withChallengeBudget(first.projectId, () => createProjectCommandChallenge({
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

// The shared class admits at most 20 new Challenges in two adjacent windows,
// so 21 admitted Undo Challenges prove the author_edit class without a controlled clock.
test("undoLatestAuthorAction Challenges use the author_edit Challenge Rate Class, not the shared budget", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000888000",
      "Undo Admission Novel",
      "018f0000-0000-7001-8000-000000888001",
    );
    const undoChallenge = async (idempotencyKey: string) => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/author-actions/undo",
        command_schema: "storyos.command.undo-latest-author-action.request.v1",
        canonical_command_digest: await digestUndoLatestAuthorAction(undoRequest({
          expectedFrontier: "1",
          expectedRevisionId: "018f0000-0000-7001-8000-000000888002",
          editorSessionId: "018f0000-0000-7001-8000-000000888003",
          correlationId: idempotencyKey,
        })),
        idempotency_key: idempotencyKey,
      },
    });
    const undoKeys = Array.from({ length: 21 }, (_, index) =>
      `018f0000-0000-7001-8000-0000008881${index.toString(16).padStart(2, "0")}`);
    const undoNonces: string[] = [];
    for (const key of undoKeys) undoNonces.push((await undoChallenge(key)).nonce);
    const exactRetry = await undoChallenge(undoKeys[0]!);
    const rename = renameRequest("Undo Admission Novel 2", "1", "018f0000-0000-7001-8000-000000888004");
    const shared = await createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PATCH",
        route_template: "/api/v1/projects/{project_id}",
        command_schema: rename.command_schema,
        canonical_command_digest: await digestUpdateProject(rename),
        idempotency_key: "018f0000-0000-7001-8000-000000888005",
      },
    });

    assert.deepEqual(
      {
        distinctUndoNonces: new Set(undoNonces).size,
        exactRetry: exactRetry.nonce === undoNonces[0],
        sharedAdmitted: shared.nonce.length > 0,
      },
      { distinctUndoNonces: 21, exactRetry: true, sharedAdmitted: true },
    );
  } finally {
    await stopRealServer(server);
  }
});

type OpenedProposal = Awaited<ReturnType<typeof openProposal>>;
type EditorSession = Awaited<ReturnType<typeof reviseCandidate>>["session"];

async function openProposal(baseUrl: string, ns: string) {
  const prepared = await prepare(baseUrl, id(`${ns}10`), "Undo Proposal Decision Novel", ns);
  const queried = await admitProse(baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id(`${ns}11`));
  if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
    throw new Error("expected an opened Proposal");
  }
  const proposalId = queried.decision.opened_proposal.proposal_id;
  const opened = await getProposal({ baseUrl, projectId: prepared.projectId, proposalId, fetchImpl: prepared.fetchImpl });
  return { ...prepared, baseUrl, proposalId, opened };
}

async function openSession(proposal: OpenedProposal, ns: string): Promise<EditorSession> {
  const request: CreateEditorSessionRequest = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}12`),
  };
  return challenged(
    proposal.baseUrl, proposal.fetchImpl, proposal.projectId, "POST", "/api/v1/projects/{project_id}/editor-sessions",
    request.command_schema, await digestCreateEditorSession(request), id(`${ns}13`),
    (antiForgery) => createEditorSession({
      baseUrl: proposal.baseUrl, projectId: proposal.projectId, fetchImpl: proposal.fetchImpl,
      idempotencyKey: id(`${ns}13`), antiForgery, request,
    }),
  );
}

async function inspect(proposal: OpenedProposal): Promise<GetProposalResponse> {
  return getProposal({
    baseUrl: proposal.baseUrl, projectId: proposal.projectId, proposalId: proposal.proposalId, fetchImpl: proposal.fetchImpl,
  });
}

async function chapterHead(proposal: OpenedProposal): Promise<string> {
  return (await getChapter({
    baseUrl: proposal.baseUrl, projectId: proposal.projectId, chapterId: proposal.chapterId, fetchImpl: proposal.fetchImpl,
  })).chapter.current_revision.revision_id;
}

// The Proposal as the author sees it, without the identities that each new revision gets.
function authorView(inspected: GetProposalResponse) {
  let text = JSON.stringify(inspected.proposal).replaceAll(inspected.proposal.revision_id, "<revision>");
  if (inspected.proposal.validation_receipt.kind === "present") {
    text = text.replaceAll(inspected.proposal.validation_receipt.validation_receipt_id, "<validation-receipt>");
  }
  return JSON.parse(text) as unknown;
}

async function withdraw(proposal: OpenedProposal, session: EditorSession, ns: string) {
  const request: WithdrawProposalRequest = {
    command_schema: "storyos.command.withdraw-proposal.request.v1",
    withdraw_proposal_input: {
      cause: "author",
      proposal_revision_id: (await inspect(proposal)).proposal.revision_id,
      expected_closure: "open",
      expected_target_revisions: [await chapterHead(proposal)],
      withdrawal_reason: { kind: "author_withdrew", note: { kind: "omitted" } },
      editor_session_id: session.editor_session.editor_session_id,
      ...BINDING,
      correlation_id: id(`${ns}21`),
    },
  };
  const withdrawn = await challenged(
    proposal.baseUrl, proposal.fetchImpl, proposal.projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/withdrawals",
    request.command_schema, await digestWithdrawProposal(request), id(`${ns}22`),
    (antiForgery) => withdrawProposal({
      baseUrl: proposal.baseUrl, projectId: proposal.projectId, proposalId: proposal.proposalId,
      fetchImpl: proposal.fetchImpl, idempotencyKey: id(`${ns}22`), antiForgery, request,
    }),
  );
  if (withdrawn.effect.kind !== "resolved" || withdrawn.effect.author_action_sequence === null) {
    throw new Error("expected an author withdrawal");
  }
  return { sequence: withdrawn.effect.author_action_sequence, eventId: withdrawn.effect.closure_event_refs[0] ?? "" };
}

async function reopenWithdrawn(proposal: OpenedProposal, session: EditorSession, withdrawalEventId: string, ns: string) {
  const request: ReopenWithdrawnProposalRequest = {
    command_schema: "storyos.command.reopen-withdrawn-proposal.request.v1",
    reopen_withdrawn_proposal_input: {
      proposal_revision_id: (await inspect(proposal)).proposal.revision_id,
      withdrawal_event_ref: withdrawalEventId,
      expected_closure: "withdrawn",
      expected_target_revisions: [await chapterHead(proposal)],
      editor_session_id: session.editor_session.editor_session_id,
      ...BINDING,
      correlation_id: id(`${ns}31`),
    },
  };
  const reopened = await challenged(
    proposal.baseUrl, proposal.fetchImpl, proposal.projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/reopenings",
    request.command_schema, await digestReopenWithdrawnProposal(request), id(`${ns}32`),
    (antiForgery) => reopenWithdrawnProposal({
      baseUrl: proposal.baseUrl, projectId: proposal.projectId, proposalId: proposal.proposalId,
      fetchImpl: proposal.fetchImpl, idempotencyKey: id(`${ns}32`), antiForgery, request,
    }),
  );
  if (reopened.effect.kind !== "resolved") throw new Error("expected a reopen");
  return reopened.effect.author_action_sequence;
}

async function rejectAndReopen(proposal: OpenedProposal, session: EditorSession, ns: string) {
  const rejectRequest: RejectProposalOperationsRequest = {
    command_schema: "storyos.command.reject-proposal-operations.request.v1",
    reject_proposal_operations_input: {
      proposal_revision_id: proposal.opened.proposal.revision_id,
      selected_pending_operation_ids: [proposal.opened.proposal.operation_id],
      expected_target_revisions: [await chapterHead(proposal)],
      rejection_reason: { kind: "author_declined", note: { kind: "omitted" } },
      editor_session_id: session.editor_session.editor_session_id,
      ...BINDING,
      correlation_id: id(`${ns}41`),
    },
  };
  const rejected = await challenged(
    proposal.baseUrl, proposal.fetchImpl, proposal.projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
    rejectRequest.command_schema, await digestRejectProposalOperations(rejectRequest), id(`${ns}42`),
    (antiForgery) => rejectProposalOperations({
      baseUrl: proposal.baseUrl, projectId: proposal.projectId, proposalId: proposal.proposalId,
      fetchImpl: proposal.fetchImpl, idempotencyKey: id(`${ns}42`), antiForgery, request: rejectRequest,
    }),
  );
  if (rejected.effect.kind !== "resolved") throw new Error("expected a rejection");
  const beforeReopen = await inspect(proposal);
  const reopenRequest: ReopenRejectedOperationsRequest = {
    command_schema: "storyos.command.reopen-rejected-operations.request.v1",
    reopen_rejected_operations_input: {
      proposal_revision_id: beforeReopen.proposal.revision_id,
      selected_rejected_operation_ids: [proposal.opened.proposal.operation_id],
      rejection_event_refs: [rejected.effect.resolution_event_refs[0] ?? ""],
      expected_target_revisions: [await chapterHead(proposal)],
      editor_session_id: session.editor_session.editor_session_id,
      ...BINDING,
      correlation_id: id(`${ns}43`),
    },
  };
  const reopened = await challenged(
    proposal.baseUrl, proposal.fetchImpl, proposal.projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/operation-reopenings",
    reopenRequest.command_schema, await digestReopenRejectedOperations(reopenRequest), id(`${ns}44`),
    (antiForgery) => reopenRejectedOperations({
      baseUrl: proposal.baseUrl, projectId: proposal.projectId, proposalId: proposal.proposalId,
      fetchImpl: proposal.fetchImpl, idempotencyKey: id(`${ns}44`), antiForgery, request: reopenRequest,
    }),
  );
  if (reopened.effect.kind !== "resolved") throw new Error("expected a reopen");
  return { beforeReopen, sequence: reopened.effect.author_action_sequence };
}

// A Replan needs a conflicted Proposal: an Acceptance after the Chapter head moved.
async function conflictAndReplan(proposal: OpenedProposal, ns: string) {
  const { session, revised } = await reviseCandidate(proposal.baseUrl, proposal.fetchImpl, proposal.projectId, proposal.opened, `${ns}5`);
  if (revised.proposal.validation_receipt.kind !== "present") throw new Error("expected a validation receipt");
  const head = await chapterHead(proposal);
  const moved = id(`${ns}59`);
  await queryPostgres(`INSERT INTO storyos.authoritative_revisions
    SELECT owner_user_id, project_id, manuscript_object_id, '${moved}'::uuid, payload_id
    FROM storyos.authoritative_revisions WHERE project_id = '${proposal.projectId}'::uuid AND revision_id = '${head}'::uuid;
    INSERT INTO storyos.manuscript_revision_members
    SELECT owner_user_id, project_id, manuscript_object_id, '${moved}'::uuid, manuscript_block_id, block_order
    FROM storyos.manuscript_revision_members WHERE project_id = '${proposal.projectId}'::uuid AND revision_id = '${head}'::uuid;
    UPDATE storyos.authoritative_heads SET current_revision_id = '${moved}'::uuid
    WHERE project_id = '${proposal.projectId}'::uuid AND manuscript_object_id = '${proposal.chapterId}'::uuid`);
  const acceptRequest: AcceptProposalRequest = {
    command_schema: "storyos.command.accept-proposal.request.v1",
    accept_proposal_input: {
      proposal_revision_id: revised.proposal.revision_id,
      validation_receipt_id: revised.proposal.validation_receipt.validation_receipt_id,
      selected_operation_ids: [revised.proposal.operation_id],
      expected_authoritative_revision_id: moved,
      editor_session_id: session.editor_session.editor_session_id,
      ...BINDING,
      correlation_id: id(`${ns}61`),
    },
  };
  const conflicted = await challenged(
    proposal.baseUrl, proposal.fetchImpl, proposal.projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
    acceptRequest.command_schema, await digestAcceptProposal(acceptRequest), id(`${ns}62`),
    (antiForgery) => acceptProposal({
      baseUrl: proposal.baseUrl, projectId: proposal.projectId, proposalId: proposal.proposalId,
      fetchImpl: proposal.fetchImpl, idempotencyKey: id(`${ns}62`), antiForgery, request: acceptRequest,
    }),
  );
  const conflictRef = conflicted.receipt.condition_refs[0] ?? "";
  const beforeReplan = await inspect(proposal);
  const replanRequest: ReplanProposalRequest = {
    command_schema: "storyos.command.replan-proposal.request.v1",
    replan_proposal_input: {
      conflicted_proposal_revision_id: beforeReplan.proposal.revision_id,
      expected_current_proposal_head: beforeReplan.proposal.revision_id,
      expected_current_target_revisions: [moved],
      replacement_operations: [beforeReplan.proposal.operation_id],
      source_condition: { kind: "proposal_conflict", proposal_conflict_ref: conflictRef },
      editor_session_id: session.editor_session.editor_session_id,
      ...BINDING,
      correlation_id: id(`${ns}71`),
    },
  };
  const replanned = await challenged(
    proposal.baseUrl, proposal.fetchImpl, proposal.projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans",
    replanRequest.command_schema, await digestReplanProposal(replanRequest), id(`${ns}72`),
    (antiForgery) => replanProposal({
      baseUrl: proposal.baseUrl, projectId: proposal.projectId, proposalId: proposal.proposalId,
      fetchImpl: proposal.fetchImpl, idempotencyKey: id(`${ns}72`), antiForgery, request: replanRequest,
    }),
  );
  if (replanned.effect.kind !== "resolved") throw new Error("expected a replan");
  return { session, beforeReplan, sequence: replanned.effect.author_action_sequence };
}

async function postProposalUndo(proposal: OpenedProposal, session: EditorSession, sequence: string, ns: string) {
  const request = undoRequest({
    expectedFrontier: sequence,
    expectedRevisionId: await chapterHead(proposal),
    editorSessionId: session.editor_session.editor_session_id,
    correlationId: id(`${ns}81`),
  });
  const posted = await postUndo(proposal.baseUrl, proposal.fetchImpl, proposal.projectId, id(`${ns}82`), request);
  return { ...posted, request };
}

// An exact retry replays the same acknowledgement.
async function undoProposalDecision(proposal: OpenedProposal, session: EditorSession, sequence: string, ns: string) {
  const { challenge, undone, request } = await postProposalUndo(proposal, session, sequence, ns);
  const retried = await undoLatestAuthorAction({
    baseUrl: proposal.baseUrl, projectId: proposal.projectId, fetchImpl: proposal.fetchImpl,
    idempotencyKey: id(`${ns}82`), antiForgery: challenge.nonce, request,
  });
  assert.deepEqual(retried, undone);
  return undone;
}

function assertCompensated(undone: Awaited<ReturnType<typeof undoLatestAuthorAction>>, sourceSequence: string, after: GetProposalResponse) {
  assert.equal(undone.effect.kind, "compensated");
  if (undone.effect.kind !== "compensated") throw new Error("expected a Compensation");
  assert.equal(undone.effect.source_sequence, sourceSequence);
  assert.equal(undone.effect.authoritative_commit_id, "");
  assert.equal(undone.proposal_revision_id, after.proposal.revision_id);
}

// A later Proposal head move: a withdrawal and its Undo append a revision after the decision.
async function assertBarrierAfterHeadMove(proposal: OpenedProposal, session: EditorSession, sequence: string, ns: string) {
  const later = await withdraw(proposal, session, `${ns}9`);
  const reopened = await postProposalUndo(proposal, session, later.sequence, `${ns}9`);
  assert.equal(reopened.undone.effect.kind, "compensated");
  const blocked = await undoProposalDecision(proposal, session, sequence, ns);
  assert.deepEqual(blocked.effect, { kind: "unavailable", reason: "barrier" });
}

test("undoLatestAuthorAction withdraws a reopened Proposal again, and a later head move makes the reopen a Barrier", async () => {
  const started = await startProposalServer();
  try {
    await drainLeftoverWork();
    const proposal = await openProposal(started.baseUrl, "c96311");
    const session = await openSession(proposal, "c96311");
    const withdrawn = await withdraw(proposal, session, "c96311");
    const beforeReopen = await inspect(proposal);
    const sequence = await reopenWithdrawn(proposal, session, withdrawn.eventId, "c96311");
    const undone = await undoProposalDecision(proposal, session, sequence, "c96311");
    const after = await inspect(proposal);
    assertCompensated(undone, sequence, after);
    assert.equal(after.proposal.closure, "withdrawn");
    assert.deepEqual(authorView(after), authorView(beforeReopen));

    const moved = await openProposal(started.baseUrl, "c96312");
    const movedSession = await openSession(moved, "c96312");
    const movedWithdrawal = await withdraw(moved, movedSession, "c96312");
    const movedSequence = await reopenWithdrawn(moved, movedSession, movedWithdrawal.eventId, "c96312");
    await assertBarrierAfterHeadMove(moved, movedSession, movedSequence, "c96312");
  } finally {
    await stopRealServer(started.server);
  }
});

test("undoLatestAuthorAction rejects reopened operations again, and a later head move makes the reopen a Barrier", async () => {
  const started = await startProposalServer();
  try {
    await drainLeftoverWork();
    const proposal = await openProposal(started.baseUrl, "c96321");
    const session = await openSession(proposal, "c96321");
    const { beforeReopen, sequence } = await rejectAndReopen(proposal, session, "c96321");
    const undone = await undoProposalDecision(proposal, session, sequence, "c96321");
    const after = await inspect(proposal);
    assertCompensated(undone, sequence, after);
    assert.equal(after.proposal.operation_resolution, "rejected");
    assert.deepEqual(authorView(after), authorView(beforeReopen));

    const moved = await openProposal(started.baseUrl, "c96322");
    const movedSession = await openSession(moved, "c96322");
    const reopened = await rejectAndReopen(moved, movedSession, "c96322");
    await assertBarrierAfterHeadMove(moved, movedSession, reopened.sequence, "c96322");
  } finally {
    await stopRealServer(started.server);
  }
});

test("undoLatestAuthorAction restores the Proposal before a Replan, and a later head move makes the Replan a Barrier", async () => {
  const started = await startProposalServer();
  try {
    await drainLeftoverWork();
    const proposal = await openProposal(started.baseUrl, "c96331");
    const { session, beforeReplan, sequence } = await conflictAndReplan(proposal, "c96331");
    const undone = await undoProposalDecision(proposal, session, sequence, "c96331");
    const after = await inspect(proposal);
    assertCompensated(undone, sequence, after);
    // The Acceptance conflict condition stays on the conflicted revision (ADR 0044).
    assert.deepEqual(authorView(after), authorView({
      ...beforeReplan,
      proposal: { ...beforeReplan.proposal, validation: "valid", condition_refs: [], source_condition: { kind: "absent" } },
    }));

    const moved = await openProposal(started.baseUrl, "c96332");
    const replanned = await conflictAndReplan(moved, "c96332");
    await assertBarrierAfterHeadMove(moved, replanned.session, replanned.sequence, "c96332");
  } finally {
    await stopRealServer(started.server);
  }
});
