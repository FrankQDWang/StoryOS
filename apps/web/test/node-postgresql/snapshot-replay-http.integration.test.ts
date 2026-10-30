// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/activity-stream-cross-table-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import {
  activityStream, createEditorSession, digestCreateEditorSession,
  getChapter, getEditorSession, getManuscriptTree, getSnapshot, searchManuscript,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { GetEditorSessionResponse, SearchManuscriptRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { PROJECT_A, TestServer, USER_A, UUID_V7, appliedVolume, challenged, createChapterInput, createProject,
  createVolumeInput, deleteChapterInput, deleteVolumeInput, problemCode, problemOf, sanitizedProtocolFailure,
  serverForFile, startServer, updateChapterInput, updateVolumeInput } from "../support/http-harness.ts";
import type { CommandResponse } from "../support/http-harness.ts";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";

const UNKNOWN_SNAPSHOT = "018f0000-0000-7001-8000-000000000250";
const PROJECT_B = "018f0000-0000-7001-8000-000000000102";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const SEARCH_UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const ISO_INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
// The Canonical Sibling Order and historical acknowledgement tests use this Server. The other tests start their
// own Server.
const fileServer = serverForFile();


function hasProblem(error: unknown, status: number, code: string): boolean {
  const protocolError = requireStoryOSProtocolError(error);
  return protocolError.status === status
    && problemCode(protocolError) === code
    && !/Project A|Authoritative A/.test(protocolError.responseBody ?? "");
}

async function ensureCurrentWriter(baseUrl: string): Promise<GetEditorSessionResponse> {
  const existing = await queryPostgres(`SELECT current_editor_session_id::text
    FROM storyos.project_writer_generations
    WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid`);
  if (existing) {
    const session = await getEditorSession({
      baseUrl, projectId: PROJECT_A, editorSessionId: existing,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    if (session.writer.kind !== "current_writer") {
      throw new Error("the fixture did not expose the current writer");
    }
    return session;
  }
  const request = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    client_contract_revision: "storyos.web-client.release-1.v3",
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000201",
  };
  const digest = await digestCreateEditorSession(request);
  const session = await challenged(baseUrl, browserFetch(baseUrl, "session-a"), PROJECT_A, "POST",
    "/api/v1/projects/{project_id}/editor-sessions", request.command_schema, digest,
    "018f0000-0000-7001-8000-000000000202", (antiForgery) => createEditorSession({
      baseUrl, projectId: PROJECT_A, request,
      idempotencyKey: "018f0000-0000-7001-8000-000000000202",
      antiForgery, fetchImpl: browserFetch(baseUrl, "session-a"),
    }));
  if (session.writer.kind !== "current_writer") {
    throw new Error("the fixture did not create the current writer");
  }
  return session;
}

test("getSnapshot and activityStream use the Editor Session Snapshot identity at the public HTTP boundary", async () => {
  const { baseUrl, server } = await startServer();
  try {
    const session = await ensureCurrentWriter(baseUrl);
    const snapshotId = session.base_snapshot.snapshot_id;
    assert.match(snapshotId, UUID);
    const snapshot = await getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(snapshot.schema_id, "storyos.query.snapshot.response.v1");
    assert.match(snapshot.correlation_id, UUID);
    assert.deepEqual(snapshot.project_scope, {
      owner_user_id: USER_A, project_id: PROJECT_A,
    });
    assert.equal(snapshot.snapshot.snapshot_id, snapshotId);
    assert.deepEqual(snapshot.snapshot.project_scope, snapshot.project_scope);
    assert.equal(snapshot.snapshot.snapshot_kind, "canonical");
    assert.equal(snapshot.snapshot.project_activity_position, session.base_snapshot.project_activity_position);
    assert.deepEqual(snapshot.snapshot.source_watermarks, {});
    assert.deepEqual(snapshot.snapshot.projection_generations, {});
    assert.equal(snapshot.snapshot.redaction_profile, "storyos.author.v1");
    assert.equal(snapshot.snapshot.schema_profile, "storyos.public.release.1");
    assert.equal(snapshot.snapshot.replay_generation, "1");
    assert.match(snapshot.snapshot.created_at, ISO_INSTANT);
    assert.equal(snapshot.snapshot.expires_at, null);

    const streamUrl = new URL(
      `/api/v1/projects/${PROJECT_A}/activity?snapshot_id=${encodeURIComponent(snapshotId)}&protocol_release=${encodeURIComponent("storyos.public.release.1")}`,
      baseUrl,
    );
    const stream = await fetch(streamUrl, {
      headers: { origin: baseUrl, cookie: "storyos_session=session-a", accept: "text/event-stream" },
    });
    assert.equal(stream.status, 200);
    assert.match(stream.headers.get("content-type") ?? "", /^text\/event-stream\b/);
    assert.equal(stream.headers.get("cache-control"), "no-store");
    assert.equal(await stream.text(), "");
    assert.equal(await activityStream({
      baseUrl, projectId: PROJECT_A, snapshotId, protocolRelease: "storyos.public.release.1",
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), "");

    const refererOnly = await fetch(
      new URL(`/api/v1/projects/${PROJECT_A}/snapshots/${snapshotId}`, baseUrl),
      { headers: { referer: `${baseUrl}/projects/${PROJECT_A}`, cookie: "storyos_session=session-a" } },
    );
    assert.equal(refererOnly.status, 200);
    const foreignOrigin = await fetch(
      new URL(`/api/v1/projects/${PROJECT_A}/snapshots/${snapshotId}`, baseUrl),
      { headers: { origin: "https://foreign.example", cookie: "storyos_session=session-a" } },
    );
    assert.equal(foreignOrigin.status, 403);
    assert.doesNotMatch(await foreignOrigin.text(), /Project A|Authoritative A/);

    await queryPostgres(`
      UPDATE storyos.replay_floors
         SET floor_position = ${Number(snapshot.snapshot.project_activity_position) + 1}
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
         AND replay_generation = 1`);
    await assert.rejects(activityStream({
      baseUrl, projectId: PROJECT_A, snapshotId, protocolRelease: "storyos.public.release.1",
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => hasProblem(error, 409, "activity_cursor_too_old"));
    await queryPostgres(`
      UPDATE storyos.replay_floors
         SET floor_position = 0
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
         AND replay_generation = 1`);
    assert.equal(await activityStream({
      baseUrl, projectId: PROJECT_A, snapshotId, protocolRelease: "storyos.public.release.1",
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), "");

    await queryPostgres(`
      INSERT INTO storyos.replay_generations (owner_user_id, project_id, replay_generation)
      VALUES ('${USER_A}'::uuid, '${PROJECT_A}'::uuid, 2)
      ON CONFLICT DO NOTHING;
      INSERT INTO storyos.replay_floors
        (owner_user_id, project_id, replay_generation, floor_position)
      VALUES ('${USER_A}'::uuid, '${PROJECT_A}'::uuid, 2, 0)
      ON CONFLICT DO NOTHING`);
    await assert.rejects(activityStream({
      baseUrl, projectId: PROJECT_A, snapshotId, protocolRelease: "storyos.public.release.1",
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => hasProblem(error, 409, "activity_cursor_too_old"));
    const afterFloor = await getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(afterFloor.snapshot.snapshot_id, snapshotId);
    assert.equal(afterFloor.snapshot.replay_generation, "1");

    await assert.rejects(getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId: UNKNOWN_SNAPSHOT,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => hasProblem(error, 404, "resource_unavailable"));
    await assert.rejects(getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId,
      fetchImpl: browserFetch(baseUrl, "session-b"),
    }), (error) => hasProblem(error, 404, "resource_unavailable"));

    const disallowed = await fetch(new URL(`/api/v1/projects/${PROJECT_A}/snapshots/${snapshotId}`, baseUrl), {
      method: "POST",
      headers: { origin: baseUrl, cookie: "storyos_session=session-a", "content-type": "application/json" },
      body: "{}",
    });
    assert.equal(disallowed.status, 405);
    assert.equal(JSON.parse(await disallowed.text()).code, "method_not_allowed");

    await queryPostgres(`
      UPDATE storyos.project_snapshots
         SET expires_at = clock_timestamp() - interval '1 second'
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
         AND snapshot_id = '${snapshotId}'::uuid`);
    await assert.rejects(getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => hasProblem(error, 409, "snapshot_expired"));
  } finally {
    await stopRealServer(server);
  }
});

test("Editor Sessions capture nonzero Activity and preserve legacy Snapshot evidence", async () => {
  const server = await TestServer.start();
  try {
    const project = await createProject(server, { title: "Session Snapshot" });
    const { baseUrl, projectId, fetchImpl } = project;
    const volume = await project.send("createVolume", { request: {
      command_schema: "storyos.command.create-volume.request.v1",
      create_volume_input: { title: "Volume", expected_tree_revision: "1", ...project.binding() },
    } });
    if (volume.response.effect.kind !== "authoritative_applied") throw new Error("Volume must apply");
    const chapter = await project.send("createChapter", { request: {
      command_schema: "storyos.command.create-chapter.request.v1",
      create_chapter_input: { title: "Chapter", expected_tree_revision: "2", ...project.binding() },
    }, path: { volumeId: volume.response.effect.volume_id } });
    if (chapter.response.effect.kind !== "authoritative_applied") throw new Error("Chapter must apply");
    const opened = await getChapter({ baseUrl, projectId, fetchImpl,
      chapterId: chapter.response.effect.chapter_id });
    assert.equal(opened.project_activity_position, "3");
    const request = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: project.key(),
    };
    const openSession = async () => {
      const sent = await project.send("createEditorSession", { request });
      return { command: sent.command, session: sent.response };
    };
    const first = await openSession();
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
    assert.deepEqual((await first.command.send()).response, first.session);
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
    const next = await openSession();
    assert.deepEqual(next.session.writer,
      { kind: "read_only", observed_writer_generation: "1", reason: "secondary_session" });
    assert.deepEqual(next.session.base_snapshot, { ...base,
      snapshot_id: next.session.base_snapshot.snapshot_id,
      created_at: next.session.base_snapshot.created_at });
    assert.equal((await getSnapshot({ ...canonicalInput,
      snapshotId: next.session.base_snapshot.snapshot_id })).snapshot.project_activity_position, "3");
    assert.deepEqual((await next.command.send()).response, next.session);
    // An exact retry returns the captured first acknowledgement, not the changed base row.
    assert.deepEqual((await first.command.send()).response, first.session);
    assert.deepEqual((await getEditorSession(legacyInput)).base_snapshot, legacy.base_snapshot);
    assert.deepEqual((await getSnapshot(canonicalInput)).snapshot, legacySnapshot.snapshot);
    assert.deepEqual(JSON.parse(await queryPostgres(retainedQuery)), retained);
    assert.equal(await queryPostgres(`SELECT count(*) FROM storyos.editor_sessions
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${projectId}'::uuid`), "2");
  } finally {
    await server.stop();
  }
});

test("searchManuscript returns a ready empty page and fail-closes across Project Scope", async () => {
  const server = await TestServer.start();
  try {
    const { baseUrl } = server;
    const empty = await createProject(server, { title: "Search Empty" });
    const { projectId } = empty;
    const request: SearchManuscriptRequest = {
      schema_id: "storyos.query.manuscript-search.request.v1",
      selection: "manuscript",
      query_text: "alpha",
      required_watermark: null,
    };
    const page = await searchManuscript({
      baseUrl,
      projectId,
      fetchImpl: empty.fetchImpl,
      request,
    });
    assert.equal(page.schema_id, "storyos.query.manuscript-search.response.v1");
    assert.equal(page.project_scope.owner_user_id, USER_A);
    assert.equal(page.project_scope.project_id, projectId);
    assert.deepEqual(page.items, []);
    assert.equal(page.completeness, "complete");
    assert.equal(page.lag, "0");
    assert.equal(page.next_cursor, null);
    assert.equal(page.projection_kind, "manuscript_search");
    assert.match(page.source_snapshot.snapshot_id, SEARCH_UUID);
    assert.equal(page.source_snapshot.snapshot_kind, "canonical");
    assert.equal(page.projection_watermark, page.source_snapshot.project_activity_position);

    const rebuilt = await searchManuscript({
      baseUrl,
      projectId,
      fetchImpl: empty.fetchImpl,
      request,
    });
    assert.equal(rebuilt.source_snapshot.snapshot_id, page.source_snapshot.snapshot_id);
    assert.equal(rebuilt.projection_watermark, page.projection_watermark);
    assert.deepEqual(rebuilt.items, page.items);

    await assert.rejects(
      searchManuscript({
        baseUrl,
        projectId,
        fetchImpl: empty.fetchImpl,
        request: {
          ...request,
          required_watermark: String(Number(page.projection_watermark) + 1),
        },
      }),
      (error) => requireStoryOSProtocolError(error).status === 503
        && problemCode(error) === "projection_not_ready"
        && !/Search Empty/.test(requireStoryOSProtocolError(error).responseBody ?? ""),
    );

    await assert.rejects(
      searchManuscript({
        baseUrl,
        projectId,
        fetchImpl: empty.fetchImpl,
        request: { ...request, query_text: "" },
      }),
      (error) => sanitizedProtocolFailure(error, 400, /Search Empty/),
    );

    await assert.rejects(
      searchManuscript({
        baseUrl,
        projectId: PROJECT_B,
        fetchImpl: server.fetch("session-a"),
        request,
      }),
      (error) => sanitizedProtocolFailure(error, 404, /Project B|secret|Search Empty/),
    );

    const searchPath = `/api/v1/projects/${projectId}/queries/manuscript-search`;
    const unauthenticated = await fetch(new URL(searchPath, baseUrl), {
      method: "POST",
      headers: { origin: baseUrl, "content-type": "application/json" },
      body: JSON.stringify(request),
    });
    assert.equal(unauthenticated.status, 401);

    const foreignOrigin = await fetch(new URL(searchPath, baseUrl), {
      method: "POST",
      headers: {
        origin: "https://foreign.example",
        "content-type": "application/json",
        cookie: "storyos_session=session-a",
      },
      body: JSON.stringify(request),
    });
    assert.equal(foreignOrigin.status, 403);
    assert.doesNotMatch(await foreignOrigin.text(), /Search Empty/);

    const disallowedGet = await fetch(new URL(searchPath, baseUrl), {
      headers: { origin: baseUrl, cookie: "storyos_session=session-a" },
    });
    assert.equal(disallowedGet.status, 405);
  } finally {
    await server.stop();
  }
});

/** The applied effect of `createChapter`. It throws when the command did not apply. */
function appliedChapter(created: CommandResponse<"createChapter">) {
  if (created.effect.kind !== "authoritative_applied") {
    throw new Error("Create Chapter must apply");
  }
  return created.effect;
}

test("createChapter reports Canonical Sibling Order through removal, replay, and historical acks", async () => {
  const first = await createProject(fileServer, { title: "Order Novel" });
  const volume = await first.send("createVolume", createVolumeInput(first, "Volume A", "1"));
  if (volume.response.effect.kind !== "authoritative_applied") {
    throw new Error("Create Volume must apply");
  }
  const volumeId = volume.response.effect.volume_id;
  const chapterA = await first.send("createChapter", createChapterInput(first, volumeId, "Chapter A", "2"));
  const chapterB = await first.send("createChapter", createChapterInput(first, volumeId, "Chapter B", "3"));
  const chapterC = await first.send("createChapter", createChapterInput(first, volumeId, "Chapter C", "4"));
  assert.equal(appliedChapter(chapterA.response).order, "1");
  assert.equal(appliedChapter(chapterB.response).order, "2");
  assert.equal(appliedChapter(chapterC.response).order, "3");
  const treeAfterC = await getManuscriptTree({
    baseUrl: first.baseUrl,
    projectId: first.projectId,
    fetchImpl: first.fetchImpl,
  });
  const snapshotAfterC = treeAfterC.snapshot.snapshot_id;
  assert.equal(treeAfterC.snapshot.replay_generation, "1");
  const replayB = (await chapterB.command.send()).response;
  assert.equal(replayB.command_id, chapterB.response.command_id);
  assert.equal(appliedChapter(replayB).order, "2");

  const chapterBId = appliedChapter(chapterB.response).chapter_id;
  const removedB = (await first.send("deleteChapter", deleteChapterInput(first, chapterBId, "5"))).response;
  assert.equal(removedB.effect.kind, "authoritative_applied");

  const chapterD = await first.send("createChapter", createChapterInput(first, volumeId, "Chapter D", "6"));
  const createdD = appliedChapter(chapterD.response);
  assert.equal(createdD.order, "3");
  const replayD = (await chapterD.command.send()).response;
  assert.equal(replayD.command_id, chapterD.response.command_id);
  assert.equal(appliedChapter(replayD).order, "3");

  const tree = await getManuscriptTree({
    baseUrl: first.baseUrl,
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
      { title: "Chapter A", order: "1", chapter_id: appliedChapter(chapterA.response).chapter_id },
      { title: "Chapter C", order: "2", chapter_id: appliedChapter(chapterC.response).chapter_id },
      { title: "Chapter D", order: "3", chapter_id: createdD.chapter_id },
    ],
  );

  const createdDActivity = (await activityStream({
    baseUrl: first.baseUrl,
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
   WHERE receipt.receipt_id = '${chapterD.response.receipt.receipt_id}'::uuid`));
  assert.equal(durable.receipt_order, "3");
  assert.equal(durable.activity_order, "3");

  await first.send("updateChapter", updateChapterInput(first, createdD.chapter_id, "Chapter D", "1", "7"));
  const replayAfterReorder = (await chapterD.command.send()).response;
  assert.equal(appliedChapter(replayAfterReorder).order, "3");

  const removedC = (await first.send("deleteChapter",
    deleteChapterInput(first, appliedChapter(chapterC.response).chapter_id, "8"))).response;
  assert.equal(removedC.effect.kind, "authoritative_applied");
  const replayAfterDelete = (await chapterD.command.send()).response;
  assert.equal(appliedChapter(replayAfterDelete).order, "3");

  await queryPostgres(`UPDATE storyos.domain_receipts
      SET result_payload = '{}'::jsonb
    WHERE receipt_id = '${chapterB.response.receipt.receipt_id}'::uuid`);
  const historicalB = (await chapterB.command.send()).response;
  assert.equal(appliedChapter(historicalB).order, "2");
  assert.equal(historicalB.receipt.receipt_id, chapterB.response.receipt.receipt_id);
  assert.deepEqual(historicalB.receipt.authoritative_commit_ids, []);
  assert.equal(historicalB.receipt.author_action_sequence, null);
});

test("createVolume reports Canonical Sibling Order through removal, replay, and historical acks", async () => {
  const first = await createProject(fileServer, { title: "Order Novel" });
  const volumeA = await first.send("createVolume", createVolumeInput(first, "Volume A", "1"));
  const volumeB = await first.send("createVolume", createVolumeInput(first, "Volume B", "2"));
  const volumeC = await first.send("createVolume", createVolumeInput(first, "Volume C", "3"));
  assert.equal(appliedVolume(volumeA.response).order, "1");
  assert.equal(appliedVolume(volumeB.response).order, "2");
  assert.equal(appliedVolume(volumeC.response).order, "3");
  const treeAfterC = await getManuscriptTree({
    baseUrl: first.baseUrl,
    projectId: first.projectId,
    fetchImpl: first.fetchImpl,
  });
  const snapshotAfterC = treeAfterC.snapshot.snapshot_id;
  assert.equal(treeAfterC.snapshot.replay_generation, "1");
  const replayB = (await volumeB.command.send()).response;
  assert.equal(replayB.command_id, volumeB.response.command_id);
  assert.equal(appliedVolume(replayB).order, "2");

  const volumeBId = appliedVolume(volumeB.response).volume_id;
  const removedB = (await first.send("deleteVolume", deleteVolumeInput(first, volumeBId, "4"))).response;
  assert.equal(removedB.effect.kind, "authoritative_applied");

  const volumeD = await first.send("createVolume", createVolumeInput(first, "Volume D", "5"));
  const createdD = appliedVolume(volumeD.response);
  assert.equal(createdD.order, "3");
  const replayD = (await volumeD.command.send()).response;
  assert.equal(replayD.command_id, volumeD.response.command_id);
  assert.equal(appliedVolume(replayD).order, "3");

  const tree = await getManuscriptTree({
    baseUrl: first.baseUrl,
    projectId: first.projectId,
    fetchImpl: first.fetchImpl,
  });
  assert.equal(tree.tree_revision, "6");
  assert.equal(tree.snapshot.replay_generation, "1");
  assert.deepEqual(
    tree.volumes.map((volume) => ({ title: volume.title, order: volume.order, volume_id: volume.volume_id })),
    [
      { title: "Volume A", order: "1", volume_id: appliedVolume(volumeA.response).volume_id },
      { title: "Volume C", order: "2", volume_id: appliedVolume(volumeC.response).volume_id },
      { title: "Volume D", order: "3", volume_id: createdD.volume_id },
    ],
  );

  const createdDActivity = (await activityStream({
    baseUrl: first.baseUrl,
    projectId: first.projectId,
    snapshotId: snapshotAfterC,
    protocolRelease: "storyos.public.release.1",
    fetchImpl: first.fetchImpl,
  })).split("\n\n").map((block) => {
    const data = block.split("\n").find((line) => line.startsWith("data:"));
    return data === undefined ? undefined : JSON.parse(data.slice(5).trim()) as {
      event_kind?: string;
      event_schema?: string;
      payload?: { volume_id?: string; order?: string };
    };
  }).find((event) => event?.event_kind === "volume_created"
    && event.payload?.volume_id === createdD.volume_id);
  assert.equal(createdDActivity?.event_schema, "storyos.event.volume-created.v2");
  assert.equal(createdDActivity?.payload?.order, "3");

  const durable = JSON.parse(await queryPostgres(`SELECT json_build_object(
    'receipt_order', receipt.result_payload->>'order',
    'activity_order', payload.payload->>'order'
  )::text
    FROM storyos.domain_receipts AS receipt
    JOIN storyos.project_activity_event_payloads AS payload
      ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
         (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
   WHERE receipt.receipt_id = '${volumeD.response.receipt.receipt_id}'::uuid`));
  assert.equal(durable.receipt_order, "3");
  assert.equal(durable.activity_order, "3");

  await first.send("updateVolume", updateVolumeInput(first, createdD.volume_id, "Volume D", "1", "6"));
  const replayAfterReorder = (await volumeD.command.send()).response;
  assert.equal(appliedVolume(replayAfterReorder).order, "3");

  const removedC = (await first.send("deleteVolume",
    deleteVolumeInput(first, appliedVolume(volumeC.response).volume_id, "7"))).response;
  assert.equal(removedC.effect.kind, "authoritative_applied");
  const replayAfterDelete = (await volumeD.command.send()).response;
  assert.equal(appliedVolume(replayAfterDelete).order, "3");

  await queryPostgres(`UPDATE storyos.domain_receipts
      SET result_payload = '{}'::jsonb
    WHERE receipt_id = '${volumeB.response.receipt.receipt_id}'::uuid`);
  const historicalB = (await volumeB.command.send()).response;
  assert.equal(appliedVolume(historicalB).order, "1");
  assert.equal(historicalB.receipt.receipt_id, volumeB.response.receipt.receipt_id);
  assert.deepEqual(historicalB.receipt.authoritative_commit_ids, []);
  assert.equal(historicalB.receipt.author_action_sequence, null);
});
