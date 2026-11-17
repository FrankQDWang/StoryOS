// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { afterAll, test } from "vitest";

import { listMemoryEntries } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, assertForeignRefused, createStory, deleteMemoryEntryInput, drainLeftoverWork, problemOf,
  seedMemoryEntry, serverForFile } from "../support/http-harness.ts";
import type { TestProject } from "../support/http-harness.ts";
import { queryStoryOSPostgres } from "../support/node-integration.ts";

// The delete of a Memory Entry by the author at the HTTP seam (ADR 0094 sections 6 and 9).
const server = serverForFile();
afterAll(() => drainLeftoverWork());

async function versions(projectId: string): Promise<string> {
  return queryStoryOSPostgres(`SELECT string_agg(concat_ws(' ', version, state, cause ->> 'kind'), ', '
    ORDER BY memory_revision) FROM storyos.memory_entry_versions WHERE project_id = '${projectId}'::uuid`);
}

async function listed(project: TestProject): Promise<string[]> {
  return (await listMemoryEntries({ baseUrl: project.baseUrl, projectId: project.projectId,
    fetchImpl: project.fetchImpl })).entries.map((entry) => entry.memory_entry_id);
}

test("deleteMemoryEntry deletes one entry of its Project and an exact retry replays", async () => {
  await drainLeftoverWork();
  const story = await createStory(server, { title: "Memory Delete Novel" });
  const other = await createStory(server, { title: "Other Memory Delete Novel" });
  const entry = await seedMemoryEntry(story.projectId);
  const foreign = await seedMemoryEntry(other.projectId);

  // The entry of another Project of the same User is unavailable, and no row changes.
  const elsewhere = await problemOf(story.send("deleteMemoryEntry", deleteMemoryEntryInput(story, foreign, "1")));
  assert.deepEqual([elsewhere.status, elsewhere.code], [404, "resource_unavailable"]);
  assert.ok(!elsewhere.body.includes(USER_A));
  assert.equal(await versions(other.projectId), "1 current author_request");

  const stale = await story.send("deleteMemoryEntry", deleteMemoryEntryInput(story, entry, "2"));
  assert.deepEqual(stale.response.effect, { kind: "conflicted", reason: "entry_changed" });

  const first = await story.send("deleteMemoryEntry", deleteMemoryEntryInput(story, entry, "1"));
  const { effect, receipt } = first.response;
  assert.deepEqual([first.response.schema_id, receipt.command_kind, receipt.result],
    ["storyos.command.delete-memory-entry.response.v1", "deleteMemoryEntry", "authoritative_applied"]);
  if (effect.kind !== "authoritative_applied") throw new Error("the delete must apply");
  assert.deepEqual(effect, { kind: "authoritative_applied", memory_entry_id: entry, version: "2",
    memory_revision: "2", project_activity_position: effect.project_activity_position });
  assert.ok(!first.body.includes("阿青"));
  assert.deepEqual(await first.command.send(), { response: first.response, body: first.body });
  assert.deepEqual(await listed(story), []);

  const again = await story.send("deleteMemoryEntry", deleteMemoryEntryInput(story, entry, "1"));
  assert.deepEqual(again.response.effect, { kind: "no_effect", reason: "already_deleted" });
  await assertForeignRefused(story, "deleteMemoryEntry", deleteMemoryEntryInput(story, entry, "2"));
  assert.equal(await versions(story.projectId), "1 current author_request, 2 deleted author_delete");
  assert.deepEqual(await listed(other), [foreign]);
});
