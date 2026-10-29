// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/acceptance-refusal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import { activityStream, getChapter, getManuscriptTree, getProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { UUID_V7, assertForeignRefused, createProject, createStory, problemOf, serverForFile,
} from "../support/http-harness.ts";
import type { CommandInput, CommandResponse, TestProject, TestStory } from "../support/http-harness.ts";
import { queryStoryOSPostgres as queryPostgres } from "../support/node-integration.ts";

// The Profile Probe of the Structural settlement profile (ADR 0075).
const server = serverForFile();
const MISSING_CHAPTER = "018f0000-0000-7001-8000-00000000ffff";
const MISSING_VOLUME = "018f0000-0000-7001-8000-00000000ffff";

function update(story: TestProject, chapterId: string, title: string, order: string,
  expectedTreeRevision: string): CommandInput<"updateChapter"> {
  return {
    request: {
      command_schema: "storyos.command.update-chapter.request.v1",
      update_chapter_input: { title, order, expected_tree_revision: expectedTreeRevision, ...story.binding() },
    },
    path: { chapterId },
  };
}

async function openProject(story: TestStory) {
  return (await getProject({ baseUrl: story.baseUrl, projectId: story.projectId, fetchImpl: story.fetchImpl })).project;
}

function createVolumeInput(project: TestProject, title: string, expectedTreeRevision: string): CommandInput<"createVolume"> {
  return { request: {
    command_schema: "storyos.command.create-volume.request.v1",
    create_volume_input: { title, expected_tree_revision: expectedTreeRevision, ...project.binding() },
  } };
}

function updateVolumeInput(project: TestProject, volumeId: string, title: string, order: string,
  expectedTreeRevision: string): CommandInput<"updateVolume"> {
  return { request: {
    command_schema: "storyos.command.update-volume.request.v1",
    update_volume_input: { title, order, expected_tree_revision: expectedTreeRevision, ...project.binding() },
  }, path: { volumeId } };
}

function deleteVolumeInput(project: TestProject, volumeId: string, expectedTreeRevision: string): CommandInput<"deleteVolume"> {
  return { request: {
    command_schema: "storyos.command.delete-volume.request.v1",
    delete_volume_input: { expected_tree_revision: expectedTreeRevision, ...project.binding() },
  }, path: { volumeId } };
}

function createChapterInput(project: TestProject, volumeId: string, title: string,
  expectedTreeRevision: string): CommandInput<"createChapter"> {
  return { request: {
    command_schema: "storyos.command.create-chapter.request.v1",
    create_chapter_input: { title, expected_tree_revision: expectedTreeRevision, ...project.binding() },
  }, path: { volumeId } };
}

function deleteChapterInput(project: TestProject, chapterId: string, expectedTreeRevision: string): CommandInput<"deleteChapter"> {
  return { request: {
    command_schema: "storyos.command.delete-chapter.request.v1",
    delete_chapter_input: { expected_tree_revision: expectedTreeRevision, ...project.binding() },
  }, path: { chapterId } };
}

function appliedChapter(created: CommandResponse<"createChapter">) {
  if (created.effect.kind !== "authoritative_applied") {
    throw new Error("Create Chapter must apply");
  }
  return created.effect;
}

function appliedVolume(created: CommandResponse<"createVolume">) {
  if (created.effect.kind !== "authoritative_applied") {
    throw new Error("Create Volume must apply");
  }
  return created.effect;
}

test("updateChapter applies, replays, refuses, and conflicts", async () => {
  const story = await createStory(server, { title: "Chapter Probe Novel", chapters: ["Chapter A", "Chapter B"] });
  const [chapterId = ""] = story.chapterIds;
  const applied = await story.send("updateChapter", update(story, chapterId, "Chapter B", "2", "4"));
  const { effect, receipt } = applied.response;
  assert.equal(applied.response.schema_id, "storyos.command.update-chapter.response.v1");
  assert.deepEqual([receipt.command_kind, receipt.result, receipt.author_action_sequence],
    ["updateChapter", "authoritative_applied", "4"]);
  assert.equal(receipt.authoritative_commit_ids.length, 1);
  assert.match(receipt.authoritative_commit_ids[0] ?? "", UUID_V7);
  if (effect.kind !== "authoritative_applied") throw new Error("Update Chapter must apply");
  assert.match(effect.project_activity_position, /^[1-9][0-9]*$/);
  assert.deepEqual(effect, { kind: "authoritative_applied", chapter_id: chapterId, title: "Chapter B", order: "2",
    tree_revision: "5", project_activity_position: effect.project_activity_position });
  assert.deepEqual(applied.response.project.open, { kind: "current_chapter", current_chapter_id: chapterId });
  assert.deepEqual(await applied.command.send(), { response: applied.response, body: applied.body });

  const changed = update(story, chapterId, "Changed Retry", "2", "5");
  const { status, code } = await problemOf(applied.command.send({ request: changed.request }));
  assert.deepEqual({ status, code }, { status: 422, code: "challenge_invalid" });

  const stale = await story.send("updateChapter", update(story, chapterId, "Chapter B", "2", "4"));
  assert.deepEqual(stale.response.effect, { kind: "conflicted", reason: "stale_tree_revision" });
  assert.deepEqual([stale.response.receipt.authoritative_commit_ids, stale.response.receipt.author_action_sequence],
    [[], null]);
  const unchanged = await story.send("updateChapter", update(story, chapterId, "Chapter B", "2", "5"));
  assert.deepEqual(unchanged.response.effect, { kind: "no_effect", reason: "unchanged" });
  const invalidOrder = await story.send("updateChapter", update(story, chapterId, "Chapter B", "3", "5"));
  assert.deepEqual(invalidOrder.response.effect, { kind: "refused", reason: "invalid_order" });
  const missing = await story.send("updateChapter", update(story, MISSING_CHAPTER, "Chapter B", "1", "5"));
  assert.deepEqual(missing.response.effect, { kind: "refused", reason: "invalid_chapter_join" });
  assert.equal((await problemOf(story.send("updateChapter", update(story, chapterId, "", "2", "5")))).status, 400);
  await assertForeignRefused(story, "updateChapter", update(story, chapterId, "Stolen Chapter", "1", "5"));

  const archived = await story.send("archiveProject", { request: {
    command_schema: "storyos.command.archive-project.request.v1",
    archive_project_input: { expected_project_revision: "1", ...story.binding() },
  } });
  assert.equal(archived.response.effect.kind, "authoritative_applied");
  const refused = await story.send("updateChapter", update(story, chapterId, "Chapter A", "1", "5"));
  assert.deepEqual(refused.response.effect, { kind: "refused", reason: "archived_project" });
});

test("updateChapter replays its first acknowledgements after later changes and a Server restart", async () => {
  const story = await createStory(server, { title: "Chapter Freeze Novel", chapters: ["Chapter A", "Chapter B"] });
  const [chapterA = "", chapterB = ""] = story.chapterIds;
  const session = await story.openEditorSession();
  const applied = await story.send("updateChapter", update(story, chapterA, "Chapter A Renamed", "1", "4"));
  const unchanged = await story.send("updateChapter", update(story, chapterA, "Chapter A Renamed", "1", "5"));
  const stale = await story.send("updateChapter", update(story, chapterA, "Stale Chapter", "1", "3"));
  assert.deepEqual([applied, unchanged, stale].map(({ response }) =>
    [response.receipt.result, response.project.title, response.project.open]),
  [
    ["authoritative_applied", "Chapter Freeze Novel", { kind: "current_chapter", current_chapter_id: chapterA }],
    ["no_effect", "Chapter Freeze Novel", { kind: "current_chapter", current_chapter_id: chapterA }],
    ["conflicted", "Chapter Freeze Novel", { kind: "current_chapter", current_chapter_id: chapterA }],
  ]);

  const openedB = await getChapter({ baseUrl: story.baseUrl, projectId: story.projectId, chapterId: chapterB,
    fetchImpl: story.fetchImpl });
  const switched = await story.send("setCurrentChapter", { request: {
    command_schema: "storyos.command.set-current-chapter.request.v1",
    set_current_chapter_input: { chapter_id: chapterB, expected_current_chapter_id: chapterA,
      expected_target_revision_id: openedB.chapter.current_revision.revision_id,
      editor_session_id: session.editor_session.editor_session_id, ...story.binding() },
  } });
  assert.equal(switched.response.effect.kind, "authoritative_applied");
  const renamed = await story.send("updateProject", { request: {
    command_schema: "storyos.command.update-project.request.v1",
    update_project_input: { title: "Later Chapter Title", expected_project_revision: "1", ...story.binding() },
  } });
  assert.equal(renamed.response.effect.kind, "authoritative_applied");
  const later = await openProject(story);
  assert.deepEqual([later.title, later.open], ["Later Chapter Title", { kind: "current_chapter", current_chapter_id: chapterB }]);

  for (const first of [applied, unchanged, stale]) {
    assert.deepEqual(await first.command.send(), { response: first.response, body: first.body });
  }
  await server.restart();
  assert.deepEqual(await applied.command.send(), { response: applied.response, body: applied.body });
});

test("createChapter places a new Chapter atomically beside its exact live anchor", async () => {
  const project = await createProject(server, { title: "Chapter placement" });
  const { baseUrl, projectId, fetchImpl } = project;
  const volume = await project.send("createVolume", createVolumeInput(project, "Volume", "1"));
  assert.ok(volume.response.effect.kind === "authoritative_applied");
  const volumeId = volume.response.effect.volume_id;
  const tree = () => getManuscriptTree({ baseUrl, projectId, fetchImpl })
    .then(({ correlation_id: _correlationId, ...facts }) => facts);
  const add = async (title: string, placement?: { kind: "before" | "after"; chapter_id: string }) => {
    const input = createChapterInput(project, volumeId, title, (await tree()).tree_revision);
    if (placement !== undefined) Object.assign(input.request.create_chapter_input, { placement });
    return { input, ...await project.send("createChapter", input) };
  };
  const first = await add("First");
  assert.ok(first.response.effect.kind === "authoritative_applied");
  const firstId = first.response.effect.chapter_id;
  const source = await getChapter({ baseUrl, projectId, chapterId: firstId, fetchImpl });
  const last = await add("Last");
  assert.ok(last.response.effect.kind === "authoritative_applied");
  const before = await add("Before last", { kind: "before", chapter_id: last.response.effect.chapter_id });
  assert.ok(before.response.effect.kind === "authoritative_applied");
  assert.equal(before.response.effect.order, "2");
  const after = await add("After first", { kind: "after", chapter_id: firstId });
  assert.ok(after.response.effect.kind === "authoritative_applied");
  assert.equal(after.response.effect.order, "2");
  const placed = await tree();
  assert.deepEqual(placed.volumes[0]?.chapters.map((chapter) => [chapter.title, chapter.order]),
    [["First", "1"], ["After first", "2"], ["Before last", "3"], ["Last", "4"]]);
  const replay = (await before.command.send()).response;
  assert.deepEqual(replay, before.response);
  assert.deepEqual(await tree(), placed);
  const preserved = await getChapter({ baseUrl, projectId, chapterId: firstId, fetchImpl });
  assert.deepEqual(preserved.chapter, source.chapter);
  assert.deepEqual((await getProject({ baseUrl, projectId, fetchImpl })).project, first.response.project);
  for (const anchor of [MISSING_VOLUME, volumeId]) {
    const refused = await add("Invalid anchor", { kind: "before", chapter_id: anchor });
    assert.deepEqual(refused.response.effect, { kind: "refused", reason: "invalid_placement" });
    assert.deepEqual(await tree(), placed);
  }
  const otherVolume = await project.send("createVolume", createVolumeInput(project, "Other Volume", placed.tree_revision));
  assert.ok(otherVolume.response.effect.kind === "authoritative_applied");
  const foreignAnchor = createChapterInput(project, otherVolume.response.effect.volume_id, "Wrong Volume anchor",
    (await tree()).tree_revision);
  Object.assign(foreignAnchor.request.create_chapter_input, { placement: { kind: "after", chapter_id: firstId } });
  const unchanged = await tree();
  const foreign = await project.send("createChapter", foreignAnchor);
  assert.deepEqual(foreign.response.effect, { kind: "refused", reason: "invalid_placement" });
  assert.deepEqual(await tree(), unchanged);
  const stale = await project.send("createChapter", { request: before.input.request, path: before.input.path });
  assert.deepEqual(stale.response.effect, { kind: "conflicted", reason: "stale_tree_revision" });
  assert.deepEqual(await tree(), unchanged);
});

test("createChapter reports Canonical Sibling Order through removal, replay, and historical acks", async () => {
  const first = await createProject(server, { title: "Order Novel" });
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

  await first.send("updateChapter", update(first, createdD.chapter_id, "Chapter D", "1", "7"));
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

test("createChapter distinguishes historical absence from damaged new-format evidence", async () => {
  const first = await createProject(server, { title: "Chapter History Novel" });
  const volume = await first.send("createVolume", createVolumeInput(first, "Volume A", "1"));
  if (volume.response.effect.kind !== "authoritative_applied") {
    throw new Error("Create Volume must apply");
  }
  const applied = await first.send("createChapter",
    createChapterInput(first, volume.response.effect.volume_id, "Chapter A", "2"));
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
       AND idempotency_key = '${applied.command.key}'::uuid;
  `);
  const historical = await problemOf(applied.command.send());
  assert.deepEqual([historical.status, historical.code], [409, "historical_acknowledgement_unavailable"]);
  await queryPostgres(`
    UPDATE storyos.command_idempotency
       SET acknowledgement_format = 'command_response_project.v1',
           response_project = '{"broken":true}'::jsonb
     WHERE project_id = '${first.projectId}'::uuid
       AND idempotency_key = '${applied.command.key}'::uuid;
  `);
  const damaged = await problemOf(applied.command.send());
  assert.equal(damaged.status, 503);
  assert.notEqual(damaged.code, "historical_acknowledgement_unavailable");
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
});

test("createVolume reports Canonical Sibling Order through removal, replay, and historical acks", async () => {
  const first = await createProject(server, { title: "Order Novel" });
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

test("a zero expected Tree Revision or a zero order is an invalid request", async () => {
  const owned = await createProject(server, { title: "Zero Novel" });
  const invalidRequest = async (call: Promise<unknown>) => {
    const { status, code } = await problemOf(call);
    assert.deepEqual({ status, code }, { status: 400, code: "invalid_request" });
  };
  await invalidRequest(owned.send("createVolume", createVolumeInput(owned, "Volume Zero", "0")));
  const volume = await owned.send("createVolume", createVolumeInput(owned, "Volume A", "1"));
  if (volume.response.effect.kind !== "authoritative_applied") throw new Error("Create Volume must apply");
  await invalidRequest(owned.send("updateVolume",
    updateVolumeInput(owned, volume.response.effect.volume_id, "Volume A", "0", "2")));
  const tree = await getManuscriptTree({ baseUrl: owned.baseUrl, projectId: owned.projectId, fetchImpl: owned.fetchImpl });
  assert.equal(tree.tree_revision, "2");
});

test("deleteVolume removes an empty Volume, refuses a nonempty Volume, and honors deletion", async () => {
  const project = await createProject(server, { title: "Delete Volume Novel" });
  const { baseUrl, projectId, fetchImpl } = project;
  const volumeA = (await project.send("createVolume", createVolumeInput(project, "Volume A", "1"))).response;
  const volumeB = (await project.send("createVolume", createVolumeInput(project, "Volume B", "2"))).response;
  const volumeAId = appliedVolume(volumeA).volume_id;
  const volumeBId = appliedVolume(volumeB).volume_id;
  const chapter = (await project.send("createChapter", createChapterInput(project, volumeBId, "Chapter B", "3"))).response;
  assert.equal(chapter.effect.kind, "authoritative_applied");
  const treeBefore = await getManuscriptTree({ baseUrl, projectId, fetchImpl });
  assert.deepEqual(treeBefore.volumes.map((volume) => volume.title), ["Volume A", "Volume B"]);

  const refused = (await project.send("deleteVolume", deleteVolumeInput(project, volumeBId, "4"))).response;
  assert.equal(refused.effect.kind, "refused");
  if (refused.effect.kind === "refused") {
    assert.equal(refused.effect.reason, "nonempty_volume");
  }
  assert.deepEqual(refused.receipt.authoritative_commit_ids, []);
  assert.equal(refused.receipt.author_action_sequence, null);
  const treeAfterRefuse = await getManuscriptTree({ baseUrl, projectId, fetchImpl });
  assert.equal(treeAfterRefuse.tree_revision, treeBefore.tree_revision);
  assert.deepEqual(
    treeAfterRefuse.volumes.map((volume) => ({ title: volume.title, order: volume.order })),
    [
      { title: "Volume A", order: "1" },
      { title: "Volume B", order: "2" },
    ],
  );

  // The retry below solves a new Command Challenge with the same idempotency key and the same request.
  const removedInput = { ...deleteVolumeInput(project, volumeAId, "4"), key: project.key() };
  const removed = (await project.send("deleteVolume", removedInput)).response;
  assert.equal(removed.effect.kind, "authoritative_applied");
  assert.equal(removed.receipt.command_kind, "deleteVolume");
  assert.match(removed.command_id, UUID_V7);
  assert.equal(removed.receipt.authoritative_commit_ids.length, 1);
  assert.match(removed.receipt.authoritative_commit_ids[0] ?? "", UUID_V7);
  assert.equal(removed.receipt.author_action_sequence, "4");
  const treeAfterA = await getManuscriptTree({ baseUrl, projectId, fetchImpl });
  assert.deepEqual(
    treeAfterA.volumes.map((volume) => ({ title: volume.title, order: volume.order })),
    [{ title: "Volume B", order: "1" }],
  );

  const retried = (await project.send("deleteVolume", removedInput)).response;
  assert.equal(retried.effect.kind, "authoritative_applied");
  assert.equal(retried.command_id, removed.command_id);
  assert.deepEqual(
    retried.receipt.authoritative_commit_ids,
    removed.receipt.authoritative_commit_ids,
  );
  assert.equal(retried.receipt.author_action_sequence, "4");

  const alreadyRemoved = (await project.send("deleteVolume", deleteVolumeInput(project, volumeAId, "5"))).response;
  assert.equal(alreadyRemoved.effect.kind, "no_effect");
  if (alreadyRemoved.effect.kind === "no_effect") {
    assert.equal(alreadyRemoved.effect.reason, "already_removed");
  }
  assert.deepEqual(alreadyRemoved.receipt.authoritative_commit_ids, []);
  assert.equal(alreadyRemoved.receipt.author_action_sequence, null);

  const revived = (await project.send("createChapter", createChapterInput(project, volumeAId, "Revived", "5"))).response;
  assert.equal(revived.effect.kind, "refused");
  if (revived.effect.kind === "refused") {
    assert.equal(revived.effect.reason, "invalid_volume_join");
  }
  const treeAfterRevive = await getManuscriptTree({ baseUrl, projectId, fetchImpl });
  assert.equal(treeAfterRevive.tree_revision, treeAfterA.tree_revision);
  assert.deepEqual(treeAfterRevive.volumes.map((volume) => volume.title), ["Volume B"]);

  const foreign = await problemOf(project.as("session-b").send("deleteVolume", deleteVolumeInput(project, volumeBId, "5")));
  assert.equal(foreign.status, 404);
});

// One Project admits at most 10 Command Challenges in one 60-second window.
test("deleteVolume freezes applied, no-effect, conflict, and refusal acknowledgements after later title and Current Chapter changes", async () => {
  const first = await createProject(server, { title: "Delete Volume Freeze Novel" });
  const volumeA = (await first.send("createVolume", createVolumeInput(first, "Volume A", "1"))).response;
  const volumeB = (await first.send("createVolume", createVolumeInput(first, "Volume B", "2"))).response;
  const volumeAId = appliedVolume(volumeA).volume_id;
  const volumeBId = appliedVolume(volumeB).volume_id;
  const chapter = (await first.send("createChapter", createChapterInput(first, volumeBId, "Chapter B", "3"))).response;
  assert.equal(chapter.effect.kind, "authoritative_applied");
  if (chapter.effect.kind !== "authoritative_applied") {
    throw new Error("Create Chapter must apply");
  }
  const chapterId = chapter.effect.chapter_id;
  const refused = await first.send("deleteVolume", deleteVolumeInput(first, volumeBId, "4"));
  const refusedBody = refused.body;
  assert.equal(refused.response.effect.kind, "refused");
  assert.equal(refused.response.project.title, "Delete Volume Freeze Novel");
  assert.equal(refused.response.project.open.kind, "current_chapter");
  const applied = await first.send("deleteVolume", deleteVolumeInput(first, volumeAId, "4"));
  const appliedBody = applied.body;
  assert.equal(applied.response.effect.kind, "authoritative_applied");
  assert.equal(applied.response.project.title, "Delete Volume Freeze Novel");
  assert.equal(applied.response.project.open.kind, "current_chapter");
  if (applied.response.project.open.kind !== "current_chapter") {
    throw new Error("Delete Volume must keep Chapter B");
  }
  assert.equal(applied.response.project.open.current_chapter_id, chapterId);
  const unchanged = await first.send("deleteVolume", deleteVolumeInput(first, volumeAId, "5"));
  const unchangedBody = unchanged.body;
  assert.equal(unchanged.response.receipt.result, "no_effect");
  assert.equal(unchanged.response.project.title, "Delete Volume Freeze Novel");
  const stale = await first.send("deleteVolume", deleteVolumeInput(first, volumeAId, "4"));
  const staleBody = stale.body;
  assert.equal(stale.response.receipt.result, "conflicted");
  assert.equal(stale.response.project.title, "Delete Volume Freeze Novel");
  const later = (await first.send("updateProject", { request: {
    command_schema: "storyos.command.update-project.request.v1",
    update_project_input: { title: "Later Delete Volume Title", expected_project_revision: "1", ...first.binding() },
  } })).response;
  assert.equal(later.project.title, "Later Delete Volume Title");
  const frozen = await applied.command.send();
  assert.deepEqual(frozen.response, applied.response);
  assert.equal(frozen.body, appliedBody);
  assert.equal(frozen.response.project.title, "Delete Volume Freeze Novel");
  if (frozen.response.project.open.kind !== "current_chapter") {
    throw new Error("retry must keep Chapter B");
  }
  assert.equal(frozen.response.project.open.current_chapter_id, chapterId);
  const frozenRefused = await refused.command.send();
  assert.deepEqual(frozenRefused.response, refused.response);
  assert.equal(frozenRefused.body, refusedBody);
  const frozenUnchanged = await unchanged.command.send();
  assert.deepEqual(frozenUnchanged.response, unchanged.response);
  assert.equal(frozenUnchanged.body, unchangedBody);
  const frozenStale = await stale.command.send();
  assert.deepEqual(frozenStale.response, stale.response);
  assert.equal(frozenStale.body, staleBody);
  const opened = await getProject({
    baseUrl: first.baseUrl,
    projectId: first.projectId,
    fetchImpl: first.fetchImpl,
  });
  assert.equal(opened.project.title, "Later Delete Volume Title");
  assert.equal(opened.project.open.kind, "current_chapter");
  if (opened.project.open.kind !== "current_chapter") {
    throw new Error("GET must report Chapter B");
  }
  assert.equal(opened.project.open.current_chapter_id, chapterId);
  const receiptCount = await queryPostgres(`
    SELECT count(*) FROM storyos.domain_receipts
     WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'deleteVolume';
  `);
  assert.equal(receiptCount, "4");
  assert.equal(
    await queryPostgres(`
      SELECT count(*) FROM storyos.authoritative_commits
       WHERE project_id = '${first.projectId}'::uuid
         AND receipt_id = '${applied.response.receipt.receipt_id}'::uuid;
    `),
    "1",
  );
  await server.restart();
  const afterRestart = await applied.command.send();
  assert.deepEqual(afterRestart.response, applied.response);
  assert.equal(afterRestart.body, appliedBody);
  assert.equal(
    await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'deleteVolume';
    `),
    receiptCount,
  );
});
