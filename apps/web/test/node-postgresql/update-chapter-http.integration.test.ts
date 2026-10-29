// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/create-chapter-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import { getChapter, getProject } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { UUID_V7, assertForeignRefused, createStory, problemOf, serverForFile } from "../support/http-harness.ts";
import type { CommandInput, TestProject, TestStory } from "../support/http-harness.ts";

// The Profile Probe of the Structural settlement profile (ADR 0075).
const server = serverForFile();
const MISSING_CHAPTER = "018f0000-0000-7001-8000-00000000ffff";

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
