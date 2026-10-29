// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/acceptance-refusal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import { getChapter, getManuscriptTree, getProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { UUID_V7, appliedVolume, assertForeignRefused, createChapterInput, createProject, createStory, createVolumeInput,
  deleteVolumeInput, problemOf, serverForFile, updateChapterInput, updateVolumeInput,
} from "../support/http-harness.ts";
import type { TestStory } from "../support/http-harness.ts";
import { queryStoryOSPostgres as queryPostgres } from "../support/node-integration.ts";

// The Profile Probe of the Structural settlement profile (ADR 0075).
const server = serverForFile();
const MISSING_CHAPTER = "018f0000-0000-7001-8000-00000000ffff";
const MISSING_VOLUME = "018f0000-0000-7001-8000-00000000ffff";

async function openProject(story: TestStory) {
  return (await getProject({ baseUrl: story.baseUrl, projectId: story.projectId, fetchImpl: story.fetchImpl })).project;
}

test("updateChapter applies, replays, refuses, and conflicts", async () => {
  const story = await createStory(server, { title: "Chapter Probe Novel", chapters: ["Chapter A", "Chapter B"] });
  const [chapterId = ""] = story.chapterIds;
  const applied = await story.send("updateChapter", updateChapterInput(story, chapterId, "Chapter B", "2", "4"));
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

  const changed = updateChapterInput(story, chapterId, "Changed Retry", "2", "5");
  const { status, code } = await problemOf(applied.command.send({ request: changed.request }));
  assert.deepEqual({ status, code }, { status: 422, code: "challenge_invalid" });

  const stale = await story.send("updateChapter", updateChapterInput(story, chapterId, "Chapter B", "2", "4"));
  assert.deepEqual(stale.response.effect, { kind: "conflicted", reason: "stale_tree_revision" });
  assert.deepEqual([stale.response.receipt.authoritative_commit_ids, stale.response.receipt.author_action_sequence],
    [[], null]);
  const unchanged = await story.send("updateChapter", updateChapterInput(story, chapterId, "Chapter B", "2", "5"));
  assert.deepEqual(unchanged.response.effect, { kind: "no_effect", reason: "unchanged" });
  const invalidOrder = await story.send("updateChapter", updateChapterInput(story, chapterId, "Chapter B", "3", "5"));
  assert.deepEqual(invalidOrder.response.effect, { kind: "refused", reason: "invalid_order" });
  const missing = await story.send("updateChapter", updateChapterInput(story, MISSING_CHAPTER, "Chapter B", "1", "5"));
  assert.deepEqual(missing.response.effect, { kind: "refused", reason: "invalid_chapter_join" });
  assert.equal((await problemOf(story.send("updateChapter", updateChapterInput(story, chapterId, "", "2", "5")))).status, 400);
  await assertForeignRefused(story, "updateChapter", updateChapterInput(story, chapterId, "Stolen Chapter", "1", "5"));

  const archived = await story.send("archiveProject", { request: {
    command_schema: "storyos.command.archive-project.request.v1",
    archive_project_input: { expected_project_revision: "1", ...story.binding() },
  } });
  assert.equal(archived.response.effect.kind, "authoritative_applied");
  const refused = await story.send("updateChapter", updateChapterInput(story, chapterId, "Chapter A", "1", "5"));
  assert.deepEqual(refused.response.effect, { kind: "refused", reason: "archived_project" });
});

test("updateChapter replays its first acknowledgements after later changes and a Server restart", async () => {
  const story = await createStory(server, { title: "Chapter Freeze Novel", chapters: ["Chapter A", "Chapter B"] });
  const [chapterA = "", chapterB = ""] = story.chapterIds;
  const session = await story.openEditorSession();
  const applied = await story.send("updateChapter", updateChapterInput(story, chapterA, "Chapter A Renamed", "1", "4"));
  const unchanged = await story.send("updateChapter", updateChapterInput(story, chapterA, "Chapter A Renamed", "1", "5"));
  const stale = await story.send("updateChapter", updateChapterInput(story, chapterA, "Stale Chapter", "1", "3"));
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
