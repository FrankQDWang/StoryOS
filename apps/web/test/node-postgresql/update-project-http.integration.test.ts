// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/snapshot-replay-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { getChapter, getProject, listProjects } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { TestServer, UUID_V7, assertForeignRefused, createProject, createStory, freshNamespace, id, problemOf,
  serverForFile, waitForSettledKey,
} from "../support/http-harness.ts";
import type { CommandInput, TestProject } from "../support/http-harness.ts";

// The Profile Probe of the ActivityOnly settlement profile (ADR 0075).
const server = serverForFile();

function rename(project: TestProject, title: string, expectedProjectRevision: string): CommandInput<"updateProject"> {
  return { request: {
    command_schema: "storyos.command.update-project.request.v1",
    update_project_input: { title, expected_project_revision: expectedProjectRevision, ...project.binding() },
  } };
}

async function openProject(project: TestProject) {
  return (await getProject({ baseUrl: project.baseUrl, projectId: project.projectId,
    fetchImpl: project.fetchImpl })).project;
}

test("updateProject applies, replays, refuses, and conflicts", async () => {
  const project = await createProject(server, { title: "Empty Novel" });
  const applied = await project.send("updateProject", rename(project, "Renamed Novel", "1"));
  const { effect, receipt } = applied.response;
  assert.equal(applied.response.schema_id, "storyos.command.update-project.response.v1");
  assert.deepEqual([receipt.command_kind, receipt.result, applied.response.project.title],
    ["updateProject", "authoritative_applied", "Renamed Novel"]);
  assert.match(applied.response.command_id, UUID_V7);
  if (effect.kind !== "authoritative_applied") throw new Error("Update Project must apply");
  assert.deepEqual(effect, { kind: "authoritative_applied", title: "Renamed Novel", revision: "2",
    project_activity_position: effect.project_activity_position });
  assert.deepEqual(await applied.command.send(), { response: applied.response, body: applied.body });
  const listed = (await listProjects({ baseUrl: project.baseUrl, fetchImpl: project.fetchImpl })).projects
    .find((item) => item.project_scope.project_id === project.projectId);
  assert.deepEqual([listed?.title, listed?.revision], ["Renamed Novel", "2"]);

  const changed = rename(project, "Changed Retry", "1");
  const { status, code } = await problemOf(applied.command.send({ request: changed.request }));
  assert.deepEqual({ status, code }, { status: 422, code: "challenge_invalid" });

  const unchanged = await project.send("updateProject", rename(project, "Renamed Novel", "2"));
  assert.deepEqual(unchanged.response.effect, { kind: "no_effect", reason: "title_unchanged" });
  const stale = await project.send("updateProject", rename(project, "Stale Title", "1"));
  assert.deepEqual(stale.response.effect, { kind: "conflicted", reason: "stale_project_revision" });
  assert.equal((await problemOf(project.send("updateProject", rename(project, "", "2")))).status, 400);
  await assertForeignRefused(project, "updateProject", rename(project, "Stolen Title", "2"));
  assert.equal((await openProject(project)).title, "Renamed Novel");

  const lost = await project.challenge("updateProject", rename(project, "Ack Lost Novel", "2"));
  await assert.rejects(lost.send({ fetchImpl: async (input, init) => {
    await (await project.fetchImpl(input, init)).arrayBuffer();
    throw new Error("simulated acknowledgement delivery loss");
  } }), /simulated acknowledgement delivery loss/);
  const recovered = await lost.send();
  assert.notEqual(recovered.response.command_id, applied.response.command_id);
  assert.deepEqual([recovered.response.effect.kind, recovered.response.project.title],
    ["authoritative_applied", "Ack Lost Novel"]);
});

test("updateProject replays its first acknowledgements after later changes and a Server restart", async () => {
  const story = await createStory(server, { title: "Project Freeze Novel", chapters: ["Chapter A", "Chapter B"] });
  const [chapterA = "", chapterB = ""] = story.chapterIds;
  const applied = await story.send("updateProject", rename(story, "Renamed Novel", "1"));
  const unchanged = await story.send("updateProject", rename(story, "Renamed Novel", "2"));
  const stale = await story.send("updateProject", rename(story, "Stale Title", "1"));
  const atA = { kind: "current_chapter", current_chapter_id: chapterA };
  assert.deepEqual([applied, unchanged, stale].map(({ response }) =>
    [response.receipt.result, response.project.title, response.project.open]),
  [["authoritative_applied", "Renamed Novel", atA], ["no_effect", "Renamed Novel", atA], ["conflicted", "Renamed Novel", atA]]);

  const later = await story.send("updateProject", rename(story, "Later Novel", "2"));
  assert.equal(later.response.effect.kind, "authoritative_applied");
  const session = await story.openEditorSession();
  const openedB = await getChapter({ baseUrl: story.baseUrl, projectId: story.projectId, chapterId: chapterB,
    fetchImpl: story.fetchImpl });
  const switched = await story.send("setCurrentChapter", { request: {
    command_schema: "storyos.command.set-current-chapter.request.v1",
    set_current_chapter_input: { chapter_id: chapterB, expected_current_chapter_id: chapterA,
      expected_target_revision_id: openedB.chapter.current_revision.revision_id,
      editor_session_id: session.editor_session.editor_session_id, ...story.binding() },
  } });
  assert.equal(switched.response.effect.kind, "authoritative_applied");
  const opened = await openProject(story);
  assert.deepEqual([opened.title, opened.open], ["Later Novel", { kind: "current_chapter", current_chapter_id: chapterB }]);

  for (const first of [applied, unchanged, stale]) {
    assert.deepEqual(await first.command.send(), { response: first.response, body: first.body });
  }
  await server.restart();
  assert.deepEqual(await applied.command.send(), { response: applied.response, body: applied.body });
});

test("updateProject first acknowledgement excludes a later committed rename", async () => {
  const heldKey = id(`${freshNamespace()}ffff`);
  const holdPath = join(tmpdir(), `storyos-ack-hold-${heldKey}.flag`);
  writeFileSync(holdPath, "hold");
  const held = await TestServer.start({
    STORYOS_TEST_ACK_HOLD_PATH: holdPath,
    STORYOS_TEST_ACK_HOLD_IDEMPOTENCY_KEY: heldKey,
  });
  try {
    const project = await createProject(held, { title: "Empty Novel" });
    const first = await project.challenge("updateProject", { ...rename(project, "Held Novel", "1"), key: heldKey });
    const firstAck = first.send();
    await waitForSettledKey(project, heldKey);
    const later = await project.send("updateProject", rename(project, "Later Held", "2"));
    assert.equal(later.response.project.title, "Later Held");
    unlinkSync(holdPath);
    const acknowledged = await firstAck;
    assert.deepEqual([acknowledged.response.receipt.result, acknowledged.response.project.title],
      ["authoritative_applied", "Held Novel"]);
    assert.equal((await openProject(project)).title, "Later Held");
    assert.deepEqual(await first.send(), acknowledged);
  } finally {
    try {
      unlinkSync(holdPath);
    } catch {
      // The test deletes the hold file on the success path.
    }
    await held.stop();
  }
});
