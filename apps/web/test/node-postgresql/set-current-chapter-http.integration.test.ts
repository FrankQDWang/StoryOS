// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/update-chapter-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import { getChapter, getEditorSession, getProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, UUID_V7, createStory, problemOf, serverForFile } from "../support/http-harness.ts";
import type { CommandInput, TestProject, TestStory } from "../support/http-harness.ts";

// The Profile Probe of the ChapterSelection settlement profile (ADR 0075).
const server = serverForFile();
const MISSING_CHAPTER = "018f0000-0000-7001-8000-00000000ffff";
const WRONG_HEAD = "018f0000-0000-7001-8000-00000000fffe";

function select(story: TestProject, editorSessionId: string, options: {
  chapterId: string; expectedCurrentChapterId: string; expectedTargetRevisionId: string;
}): CommandInput<"setCurrentChapter"> {
  return { request: {
    command_schema: "storyos.command.set-current-chapter.request.v1",
    set_current_chapter_input: {
      chapter_id: options.chapterId,
      expected_current_chapter_id: options.expectedCurrentChapterId,
      expected_target_revision_id: options.expectedTargetRevisionId,
      editor_session_id: editorSessionId,
      ...story.binding(),
    },
  } };
}

/** A Project with Chapters A and B, a writer Editor Session, and the Head Revision of each Chapter. */
async function twoChapterStory(title: string) {
  const story = await createStory(server, { title, chapters: ["Chapter A", "Chapter B"] });
  const session = await story.openEditorSession();
  const [chapterA = "", chapterB = ""] = story.chapterIds;
  const head = async (chapterId: string) => (await getChapter({ baseUrl: story.baseUrl, projectId: story.projectId,
    chapterId, fetchImpl: story.fetchImpl })).chapter.current_revision.revision_id;
  return { story, editorSessionId: session.editor_session.editor_session_id, chapterA, chapterB,
    revisionA: await head(chapterA), revisionB: await head(chapterB) };
}

async function openProject(story: TestStory) {
  return (await getProject({ baseUrl: story.baseUrl, projectId: story.projectId, fetchImpl: story.fetchImpl })).project;
}

test("setCurrentChapter applies, replays, refuses, and conflicts", async () => {
  const { story, editorSessionId, chapterA, chapterB, revisionB } = await twoChapterStory("Current Probe Novel");
  const toB = { chapterId: chapterB, expectedCurrentChapterId: chapterA, expectedTargetRevisionId: revisionB };
  const applied = await story.send("setCurrentChapter", select(story, editorSessionId, toB));
  const { effect, receipt } = applied.response;
  assert.equal(applied.response.schema_id, "storyos.command.set-current-chapter.response.v1");
  assert.deepEqual([receipt.command_kind, receipt.result, receipt.author_action_sequence, receipt.authoritative_commit_ids],
    ["setCurrentChapter", "authoritative_applied", "4", []]);
  if (effect.kind !== "authoritative_applied") throw new Error("Set Current Chapter must apply");
  assert.equal(effect.current_chapter_id, chapterB);
  assert.match(effect.base_snapshot_id, UUID_V7);
  assert.deepEqual(applied.response.project.open, { kind: "current_chapter", current_chapter_id: chapterB });
  assert.deepEqual(await applied.command.send(), { response: applied.response, body: applied.body });
  assert.deepEqual((await openProject(story)).open, { kind: "current_chapter", current_chapter_id: chapterB });
  const editor = await getEditorSession({ baseUrl: story.baseUrl, projectId: story.projectId, editorSessionId,
    fetchImpl: story.fetchImpl });
  assert.deepEqual([editor.base_snapshot.chapter_id, editor.base_snapshot.authoritative_head_revision_id],
    [chapterB, revisionB]);

  const changed = select(story, editorSessionId, { ...toB, expectedTargetRevisionId: WRONG_HEAD });
  const { status, code } = await problemOf(applied.command.send({ request: changed.request }));
  assert.deepEqual({ status, code }, { status: 422, code: "challenge_invalid" });

  const stale = await story.send("setCurrentChapter", select(story, editorSessionId,
    { chapterId: chapterA, expectedCurrentChapterId: chapterA, expectedTargetRevisionId: revisionB }));
  assert.deepEqual(stale.response.effect, { kind: "conflicted", reason: "stale_current_chapter" });
  assert.equal(stale.response.receipt.author_action_sequence, null);
  const wrongHead = await story.send("setCurrentChapter", select(story, editorSessionId,
    { chapterId: chapterA, expectedCurrentChapterId: chapterB, expectedTargetRevisionId: WRONG_HEAD }));
  assert.deepEqual(wrongHead.response.effect, { kind: "conflicted", reason: "wrong_target_head" });
  const already = await story.send("setCurrentChapter", select(story, editorSessionId,
    { chapterId: chapterB, expectedCurrentChapterId: chapterB, expectedTargetRevisionId: revisionB }));
  assert.deepEqual(already.response.effect, { kind: "no_effect", reason: "already_current" });
  const missing = await story.send("setCurrentChapter", select(story, editorSessionId,
    { ...toB, chapterId: MISSING_CHAPTER, expectedCurrentChapterId: chapterB }));
  assert.deepEqual(missing.response.effect, { kind: "refused", reason: "invalid_chapter_join" });
  const foreign = await problemOf(story.as("session-b").send("setCurrentChapter", select(story, editorSessionId, toB)));
  assert.equal(foreign.status, 404);
  assert.ok(!foreign.body.includes(USER_A));

  const archived = await story.send("archiveProject", { request: {
    command_schema: "storyos.command.archive-project.request.v1",
    archive_project_input: { expected_project_revision: "1", ...story.binding() },
  } });
  assert.equal(archived.response.effect.kind, "authoritative_applied");
  const refused = await story.send("setCurrentChapter", select(story, editorSessionId,
    { ...toB, chapterId: chapterA, expectedCurrentChapterId: chapterB }));
  assert.deepEqual(refused.response.effect, { kind: "refused", reason: "archived_project" });
});

test("setCurrentChapter replays its first acknowledgement after later changes and a Server restart", async () => {
  const { story, editorSessionId, chapterA, chapterB, revisionA, revisionB } = await twoChapterStory("Current Freeze Novel");
  const first = await story.send("setCurrentChapter", select(story, editorSessionId,
    { chapterId: chapterB, expectedCurrentChapterId: chapterA, expectedTargetRevisionId: revisionB }));
  assert.deepEqual([first.response.project.title, first.response.project.open],
    ["Current Freeze Novel", { kind: "current_chapter", current_chapter_id: chapterB }]);

  const back = await story.send("setCurrentChapter", select(story, editorSessionId,
    { chapterId: chapterA, expectedCurrentChapterId: chapterB, expectedTargetRevisionId: revisionA }));
  assert.equal(back.response.effect.kind, "authoritative_applied");
  const renamed = await story.send("updateProject", { request: {
    command_schema: "storyos.command.update-project.request.v1",
    update_project_input: { title: "Later Current Title", expected_project_revision: "1", ...story.binding() },
  } });
  assert.equal(renamed.response.effect.kind, "authoritative_applied");
  const later = await openProject(story);
  assert.deepEqual([later.title, later.open], ["Later Current Title", { kind: "current_chapter", current_chapter_id: chapterA }]);

  assert.deepEqual(await first.command.send(), { response: first.response, body: first.body });
  await server.restart();
  assert.deepEqual(await first.command.send(), { response: first.response, body: first.body });
});
