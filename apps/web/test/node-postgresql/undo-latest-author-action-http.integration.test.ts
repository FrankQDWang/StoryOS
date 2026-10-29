// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/delete-volume-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import { getChapter, getEditorSession, getProject, getProposal } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { UUID_V7, assertForeignRefused, createStory, drainLeftoverWork, openProposal, problemOf, serverForFile,
} from "../support/http-harness.ts";
import type { CommandInput, TestProject, TestStory } from "../support/http-harness.ts";

// The Profile Probe of the Author Undo compensation (ADR 0075).
const server = serverForFile();

function undo(project: TestProject, options: {
  expectedFrontier: string; expectedRevisionId: string; editorSessionId: string;
}): CommandInput<"undoLatestAuthorAction"> {
  return { request: {
    command_schema: "storyos.command.undo-latest-author-action.request.v1",
    undo_latest_author_action_input: {
      expected_author_undo_frontier_sequence: options.expectedFrontier,
      expected_authoritative_revision_id: options.expectedRevisionId,
      editor_session_id: options.editorSessionId,
      ...project.binding(),
    },
  } };
}

/** Deletes the one Chapter of a new Project and returns the input of the Undo of that deletion. */
async function deletedChapter(title: string) {
  const story = await createStory(server, { title });
  const [chapterId = ""] = story.chapterIds;
  const session = await story.openEditorSession();
  const removed = await story.send("deleteChapter", { request: {
    command_schema: "storyos.command.delete-chapter.request.v1",
    delete_chapter_input: { expected_tree_revision: "3", ...story.binding() },
  }, path: { chapterId } });
  assert.equal(removed.response.effect.kind, "authoritative_applied");
  assert.equal(removed.response.project.open.kind, "empty");
  const editorSessionId = session.editor_session.editor_session_id;
  // The Web Client reads the Author Undo Frontier from the Editor Session before each Undo.
  const sessionFrontier = async () => (await getEditorSession({ baseUrl: story.baseUrl, projectId: story.projectId,
    editorSessionId, fetchImpl: story.fetchImpl })).author_undo_frontier_sequence ?? null;
  const expectedFrontier = await sessionFrontier();
  if (expectedFrontier === null) throw new Error("Delete Chapter must allocate an Author Action");
  assert.equal(expectedFrontier, removed.response.receipt.author_action_sequence);
  const options = { expectedFrontier, expectedRevisionId: session.base_snapshot.authoritative_head_revision_id,
    editorSessionId };
  return { story, chapterId, options, editorSessionId, sessionFrontier };
}

async function openProject(story: TestStory) {
  return (await getProject({ baseUrl: story.baseUrl, projectId: story.projectId, fetchImpl: story.fetchImpl })).project;
}

test("undoLatestAuthorAction compensates, replays, refuses, and conflicts", async () => {
  const { story, chapterId, options, sessionFrontier } = await deletedChapter("Undo Probe Novel");
  const compensated = await story.send("undoLatestAuthorAction", undo(story, options));
  const { effect, receipt } = compensated.response;
  if (effect.kind !== "compensated") throw new Error("expected a Structure Compensation");
  assert.equal(effect.source_sequence, options.expectedFrontier);
  // After the Undo, the Editor Session frontier is the next older Author Action.
  assert.deepEqual([effect.author_undo_frontier_sequence, await sessionFrontier()], ["2", "2"]);
  // A Structure Compensation writes an Authoritative Commit and no Authoritative Revision.
  assert.match(effect.authoritative_commit_id ?? "", UUID_V7);
  assert.deepEqual([Object.hasOwn(effect, "authoritative_revision"), [effect.authoritative_commit_id]],
    [false, receipt.authoritative_commit_ids]);
  assert.deepEqual(compensated.response.project.open, { kind: "current_chapter", current_chapter_id: chapterId });
  assert.deepEqual(await compensated.command.send(), { response: compensated.response, body: compensated.body });
  assert.deepEqual((await openProject(story)).open, { kind: "current_chapter", current_chapter_id: chapterId });

  const changed = undo(story, options);
  const { status, code } = await problemOf(compensated.command.send({ request: changed.request }));
  assert.deepEqual({ status, code }, { status: 422, code: "challenge_invalid" });
  const stale = await story.send("undoLatestAuthorAction", undo(story, options));
  assert.deepEqual(stale.response.effect,
    { kind: "conflicted", reason: "frontier_mismatch", current_author_undo_frontier_sequence: "2" });
  assert.deepEqual(stale.response.receipt.authoritative_commit_ids, []);
  await assertForeignRefused(story, "undoLatestAuthorAction", undo(story, options));
});

test("undoLatestAuthorAction replays its first acknowledgements after later changes and a Server restart", async () => {
  const { story, chapterId, options, editorSessionId } = await deletedChapter("Undo Freeze Novel");
  const compensated = await story.send("undoLatestAuthorAction", undo(story, options));
  assert.equal(compensated.response.effect.kind, "compensated");
  const stale = await story.send("undoLatestAuthorAction", undo(story, options));
  assert.equal(stale.response.effect.kind, "conflicted");

  const renamed = await story.send("updateProject", { request: {
    command_schema: "storyos.command.update-project.request.v1",
    update_project_input: { title: "Later Undo Title", expected_project_revision: "1", ...story.binding() },
  } });
  assert.equal(renamed.response.effect.kind, "authoritative_applied");
  const created = await story.send("createChapter", { request: {
    command_schema: "storyos.command.create-chapter.request.v1",
    create_chapter_input: { title: "Chapter B", expected_tree_revision: await story.treeRevision(), ...story.binding() },
  }, path: { volumeId: story.volumeId } });
  if (created.response.effect.kind !== "authoritative_applied") throw new Error("later Create Chapter must apply");
  const laterChapterId = created.response.effect.chapter_id;
  const openedLater = await getChapter({ baseUrl: story.baseUrl, projectId: story.projectId, chapterId: laterChapterId,
    fetchImpl: story.fetchImpl });
  const switched = await story.send("setCurrentChapter", { request: {
    command_schema: "storyos.command.set-current-chapter.request.v1",
    set_current_chapter_input: { chapter_id: laterChapterId, expected_current_chapter_id: chapterId,
      expected_target_revision_id: openedLater.chapter.current_revision.revision_id, editor_session_id: editorSessionId,
      ...story.binding() },
  } });
  assert.equal(switched.response.effect.kind, "authoritative_applied");
  const later = await openProject(story);
  assert.deepEqual([later.title, later.open],
    ["Later Undo Title", { kind: "current_chapter", current_chapter_id: laterChapterId }]);

  for (const first of [compensated, stale]) {
    assert.deepEqual(await first.command.send(), { response: first.response, body: first.body });
  }
  assert.equal(compensated.response.project.title, "Undo Freeze Novel");
  await server.restart();
  assert.deepEqual(await compensated.command.send(), { response: compensated.response, body: compensated.body });
});

test("undoLatestAuthorAction of an Operation rejection is a Barrier and keeps the rejection", async () => {
  await drainLeftoverWork();
  const story = await createStory(server, { title: "Undo Barrier Novel", assistance: true });
  const opened = (await openProposal(story)).proposal;
  const session = await story.openEditorSession();
  const head = (await getChapter({ baseUrl: story.baseUrl, projectId: story.projectId, chapterId: story.chapterIds[0] ?? "",
    fetchImpl: story.fetchImpl })).chapter.current_revision.revision_id;
  const editorSessionId = session.editor_session.editor_session_id;
  const rejected = await story.send("rejectProposalOperations", { request: {
    command_schema: "storyos.command.reject-proposal-operations.request.v1",
    reject_proposal_operations_input: {
      proposal_revision_id: opened.revision_id,
      selected_pending_operation_ids: [opened.operation_id],
      expected_target_revisions: [head],
      rejection_reason: { kind: "author_declined", note: { kind: "omitted" } },
      editor_session_id: editorSessionId,
      ...story.binding(),
    },
  }, path: { proposalId: opened.proposal_id } });
  if (rejected.response.effect.kind !== "resolved") throw new Error("expected a rejection");
  const inspect = async () => (await getProposal({ baseUrl: story.baseUrl, projectId: story.projectId,
    proposalId: opened.proposal_id, fetchImpl: story.fetchImpl })).proposal;
  const beforeUndo = await inspect();
  assert.equal(beforeUndo.operation_resolution, "rejected");

  const blocked = await story.send("undoLatestAuthorAction", undo(story, {
    expectedFrontier: rejected.response.effect.author_action_sequence, expectedRevisionId: head, editorSessionId }));
  assert.deepEqual(blocked.response.effect, { kind: "unavailable", reason: "barrier" });
  assert.deepEqual(await blocked.command.send(), { response: blocked.response, body: blocked.body });
  assert.deepEqual(await inspect(), beforeUndo);
});
