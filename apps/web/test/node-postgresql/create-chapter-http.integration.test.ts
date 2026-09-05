import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  activityStream,
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
  digestUpdateChapter,
  getChapter,
  getEditorSession,
  getManuscriptTree,
  getSnapshot,
  updateChapter,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateChapterRequest,
  CreateChapterResponse,
  CreateEditorSessionRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  DeleteChapterRequest,
  UpdateChapterRequest,
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

test("Editor Sessions capture nonzero Activity and preserve legacy Snapshot evidence", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const { projectId, fetchImpl } = await createEmpty(baseUrl, "session-a",
      "018f0000-0000-7001-8000-000000001001", "Session Snapshot",
      "018f0000-0000-7001-8000-000000001000");
    const volume = await postVolume(baseUrl, fetchImpl, projectId,
      "018f0000-0000-7001-8000-000000001002",
      volumeRequest("Volume", "1", "018f0000-0000-7001-8000-000000001003"));
    if (volume.created.effect.kind !== "authoritative_applied") throw new Error("Volume must apply");
    const chapter = await postChapter(baseUrl, fetchImpl, projectId, volume.created.effect.volume_id,
      "018f0000-0000-7001-8000-000000001004",
      chapterRequest("Chapter", "2", "018f0000-0000-7001-8000-000000001005"));
    if (chapter.created.effect.kind !== "authoritative_applied") throw new Error("Chapter must apply");
    const opened = await getChapter({ baseUrl, projectId, fetchImpl,
      chapterId: chapter.created.effect.chapter_id });
    assert.equal(opened.project_activity_position, "3");
    const request: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000001006",
    };
    const digest = await digestCreateEditorSession(request);
    async function openSession(idempotencyKey: string) {
      const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
        baseUrl, projectId, fetchImpl,
        request: { method: "POST", route_template: "/api/v1/projects/{project_id}/editor-sessions",
          command_schema: request.command_schema, canonical_command_digest: digest,
          idempotency_key: idempotencyKey },
      }));
      const input = { baseUrl, projectId, fetchImpl, request, idempotencyKey,
        antiForgery: challenge.nonce };
      return { input, session: await createEditorSession(input) };
    }
    const first = await openSession("018f0000-0000-7001-8000-000000001007");
    const base = first.session.base_snapshot;
    assert.match(base.snapshot_id, UUID_V7);
    assert.deepEqual(base, {
      snapshot_id: base.snapshot_id,
      chapter_id: opened.chapter.chapter_id,
      project_activity_position: "3",
      authoritative_head_revision_id: opened.chapter.current_revision.revision_id,
      proposal_head_revision_ids: [],
      target_refs: [`manuscript:${opened.chapter.chapter_id}`],
      observed_ownership_partition: "authoritative",
      materialized_revision: opened.chapter.current_revision,
      materialized_payload_digest: { algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
        value_hex_lowercase: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
      created_at: base.created_at,
    });
    assert.deepEqual(first.session.writer, { kind: "current_writer", writer_generation: "1" });
    assert.deepEqual(await createEditorSession(first.input), first.session);
    const canonicalInput = { baseUrl, projectId, fetchImpl, snapshotId: base.snapshot_id };
    assert.equal((await getSnapshot(canonicalInput)).snapshot.project_activity_position, "3");

    // Reproduce the old writer's stamp after the public commands have proven position 3.
    await queryPostgres(`UPDATE storyos.editor_session_base_snapshots SET project_activity_position = 0
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${projectId}'::uuid
        AND snapshot_id = '${base.snapshot_id}'::uuid;
      UPDATE storyos.project_snapshots SET project_activity_position = 0
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${projectId}'::uuid
        AND snapshot_id = '${base.snapshot_id}'::uuid`);
    const legacyInput = { baseUrl, projectId, fetchImpl,
      editorSessionId: first.session.editor_session.editor_session_id };
    const legacy = await getEditorSession(legacyInput);
    const legacySnapshot = await getSnapshot(canonicalInput);
    assert.equal(legacy.base_snapshot.project_activity_position, "0");
    const retainedQuery = `SELECT json_build_object(
      'base', (SELECT to_jsonb(s) FROM storyos.editor_session_base_snapshots s
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${projectId}'::uuid
          AND snapshot_id = '${base.snapshot_id}'::uuid),
      'snapshot', (SELECT to_jsonb(s) FROM storyos.project_snapshots s
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${projectId}'::uuid
          AND snapshot_id = '${base.snapshot_id}'::uuid),
      'counters', (SELECT to_jsonb(c) FROM storyos.scope_counters c
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${projectId}'::uuid))::text`;
    const retained = JSON.parse(await queryPostgres(retainedQuery));
    const next = await openSession("018f0000-0000-7001-8000-000000001008");
    assert.deepEqual(next.session.writer,
      { kind: "read_only", observed_writer_generation: "1", reason: "secondary_session" });
    assert.deepEqual(next.session.base_snapshot, { ...base,
      snapshot_id: next.session.base_snapshot.snapshot_id,
      created_at: next.session.base_snapshot.created_at });
    assert.equal((await getSnapshot({ ...canonicalInput,
      snapshotId: next.session.base_snapshot.snapshot_id })).snapshot.project_activity_position, "3");
    assert.deepEqual(await createEditorSession(next.input), next.session);
    assert.deepEqual(await createEditorSession(first.input),
      { ...first.session, base_snapshot: legacy.base_snapshot });
    assert.deepEqual((await getEditorSession(legacyInput)).base_snapshot, legacy.base_snapshot);
    assert.deepEqual((await getSnapshot(canonicalInput)).snapshot, legacySnapshot.snapshot);
    assert.deepEqual(JSON.parse(await queryPostgres(retainedQuery)), retained);
    assert.equal(await queryPostgres(`SELECT count(*) FROM storyos.editor_sessions
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${projectId}'::uuid`), "2");
  } finally {
    await stopRealServer(server);
  }
});

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

function appliedChapter(created: CreateChapterResponse) {
  if (created.effect.kind !== "authoritative_applied") {
    throw new Error("Create Chapter must apply");
  }
  return created.effect;
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
  return updateChapter({
    baseUrl,
    projectId,
    chapterId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
}

test("createChapter reports Canonical Sibling Order through removal, replay, and historical acks", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000000b31",
      "Order Novel",
      "018f0000-0000-7001-8000-000000000b30",
    );
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000b51",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000b41"),
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
      "018f0000-0000-7001-8000-000000000b52",
      chapterRequest("Chapter A", "2", "018f0000-0000-7001-8000-000000000b42"),
    );
    const chapterB = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000b53",
      chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-000000000b43"),
    );
    const chapterC = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000b54",
      chapterRequest("Chapter C", "4", "018f0000-0000-7001-8000-000000000b44"),
    );
    assert.equal(appliedChapter(chapterA.created).order, "1");
    assert.equal(appliedChapter(chapterB.created).order, "2");
    assert.equal(appliedChapter(chapterC.created).order, "3");
    const treeAfterC = await getManuscriptTree({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    const snapshotAfterC = treeAfterC.snapshot.snapshot_id;
    assert.equal(treeAfterC.snapshot.replay_generation, "1");
    const replayB = await createChapter({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000b53",
      antiForgery: chapterB.challenge.nonce,
      request: chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-000000000b43"),
    });
    assert.equal(replayB.command_id, chapterB.created.command_id);
    assert.equal(appliedChapter(replayB).order, "2");

    const chapterBId = appliedChapter(chapterB.created).chapter_id;
    const removedB = await deleteOwned(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      chapterBId,
      "018f0000-0000-7001-8000-000000000b56",
      deleteRequest("5", "018f0000-0000-7001-8000-000000000b46"),
    );
    assert.equal(removedB.effect.kind, "authoritative_applied");

    const chapterD = await postChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000b55",
      chapterRequest("Chapter D", "6", "018f0000-0000-7001-8000-000000000b45"),
    );
    const createdD = appliedChapter(chapterD.created);
    assert.equal(createdD.order, "3");
    const replayD = await createChapter({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000b55",
      antiForgery: chapterD.challenge.nonce,
      request: chapterRequest("Chapter D", "6", "018f0000-0000-7001-8000-000000000b45"),
    });
    assert.equal(replayD.command_id, chapterD.created.command_id);
    assert.equal(appliedChapter(replayD).order, "3");

    const tree = await getManuscriptTree({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(tree.tree_revision, "7");
    assert.equal(tree.snapshot.replay_generation, "1");
    assert.deepEqual(
      tree.volumes[0]?.chapters.map((chapter) => ({
        title: chapter.title,
        order: chapter.order,
        chapter_id: chapter.chapter_id,
      })),
      [
        { title: "Chapter A", order: "1", chapter_id: appliedChapter(chapterA.created).chapter_id },
        { title: "Chapter C", order: "2", chapter_id: appliedChapter(chapterC.created).chapter_id },
        { title: "Chapter D", order: "3", chapter_id: createdD.chapter_id },
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
        payload?: { chapter_id?: string; order?: string };
      };
    }).find((event) => event?.event_kind === "chapter_created"
      && event.payload?.chapter_id === createdD.chapter_id);
    assert.equal(createdDActivity?.event_schema, "storyos.event.chapter-created.v2");
    assert.equal(createdDActivity?.payload?.order, "3");

    const durable = JSON.parse(await queryPostgres(`SELECT json_build_object(
      'receipt_order', receipt.result_payload->>'order',
      'activity_order', payload.payload->>'order'
    )::text
      FROM storyos.domain_receipts AS receipt
      JOIN storyos.project_activity_event_payloads AS payload
        ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
           (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
     WHERE receipt.receipt_id = '${chapterD.created.receipt.receipt_id}'::uuid`));
    assert.equal(durable.receipt_order, "3");
    assert.equal(durable.activity_order, "3");

    await patchChapter(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      createdD.chapter_id,
      "018f0000-0000-7001-8000-000000000b58",
      updateRequest("Chapter D", "1", "7", "018f0000-0000-7001-8000-000000000b48"),
    );
    const replayAfterReorder = await createChapter({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000b55",
      antiForgery: chapterD.challenge.nonce,
      request: chapterRequest("Chapter D", "6", "018f0000-0000-7001-8000-000000000b45"),
    });
    assert.equal(appliedChapter(replayAfterReorder).order, "3");

    const removedC = await deleteOwned(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      appliedChapter(chapterC.created).chapter_id,
      "018f0000-0000-7001-8000-000000000b57",
      deleteRequest("8", "018f0000-0000-7001-8000-000000000b47"),
    );
    assert.equal(removedC.effect.kind, "authoritative_applied");
    const replayAfterDelete = await createChapter({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000b55",
      antiForgery: chapterD.challenge.nonce,
      request: chapterRequest("Chapter D", "6", "018f0000-0000-7001-8000-000000000b45"),
    });
    assert.equal(appliedChapter(replayAfterDelete).order, "3");

    await queryPostgres(`UPDATE storyos.domain_receipts
        SET result_payload = '{}'::jsonb
      WHERE receipt_id = '${chapterB.created.receipt.receipt_id}'::uuid`);
    const historicalB = await createChapter({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000b53",
      antiForgery: chapterB.challenge.nonce,
      request: chapterRequest("Chapter B", "3", "018f0000-0000-7001-8000-000000000b43"),
    });
    assert.equal(appliedChapter(historicalB).order, "2");
    assert.equal(historicalB.receipt.receipt_id, chapterB.created.receipt.receipt_id);
  } finally {
    await stopRealServer(server);
  }
});
