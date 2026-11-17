// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { test } from "vitest";

import { listMemoryEntries } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres, requireStoryOSProtocolError, sessionFetch } from "../support/node-integration.ts";
import { USER_A, admitRun, createStory, drainLeftoverWork, flagMemoryEntry, id, prepare, seedMemoryEntry, serverForFile,
  settleOnce } from "../support/http-harness.ts";

// The read of the Memory section of the settings page at the HTTP seam (ADR 0094 section 9).
const started = serverForFile();
const DATE_TIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;

function write(action: string, argumentsObject: Record<string, unknown>): string {
  return `SCRIPT:write_memory ${action} ${JSON.stringify(argumentsObject)}`;
}

function unavailable(error: unknown, ...hidden: string[]): boolean {
  const protocol = requireStoryOSProtocolError(error);
  const body = String(protocol.responseBody);
  return protocol.status === 404 && hidden.every((value) => !body.includes(value));
}

test("listMemoryEntries gives the current entries of one Project in Memory Index order", async () => {
  await drainLeftoverWork();
  const ns = randomBytes(3).toString("hex");
  const prepared = await prepare(started.baseUrl, id(`${ns}a1`), "Memory List Novel", `${ns}b`);
  const project = { baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl };
  assert.deepEqual((await listMemoryEntries(project)).entries, []);

  // Entry 1 is a character with a second version, entry 2 is a foreshadowing, and entry 3 is deleted.
  const message = [
    write("create", { type: "character", title: "林舟", description: "船夫。", text: "他不说话。\n<b>他看灯。</b>" }),
    write("create", { type: "foreshadowing", title: "灯塔", description: "灯塔的灯。", text: "灯每夜亮一次。" }),
    write("create", { type: "other", title: "旧事", description: "一件旧事。", text: "x" }),
    write("change", { entry: 1, description: "沉默的船夫。" }),
    write("delete", { entry: 3 }),
  ].join("\n");
  const run = await admitRun(started.baseUrl, prepared, id(`${ns}c1`), message);
  await settleOnce();
  const counts = await queryStoryOSPostgres(`SELECT count(*)::text FROM storyos.memory_entry_versions
    WHERE project_id = '${prepared.projectId}'::uuid`);
  assert.equal(counts, "5");

  const listed = await listMemoryEntries(project);
  const [foreshadowing, character] = listed.entries;
  assert.ok(foreshadowing && character);
  assert.match(foreshadowing.changed_at, DATE_TIME);
  assert.match(character.changed_at, DATE_TIME);
  const source = { kind: "conversation", conversation_id: run.conversation_id, availability: "available" };
  assert.deepEqual({ ...listed, correlation_id: "" }, {
    schema_id: "storyos.query.memory-entries.response.v1",
    correlation_id: "",
    project_scope: { owner_user_id: USER_A, project_id: prepared.projectId },
    entries: [
      { memory_entry_id: foreshadowing.memory_entry_id, entry_number: "2", version: "1", entry_type: "foreshadowing",
        title: "灯塔", description: "灯塔的灯。", text: "灯每夜亮一次。", origin: "author_request", source,
        changed_at: foreshadowing.changed_at },
      { memory_entry_id: character.memory_entry_id, entry_number: "1", version: "2", entry_type: "character",
        title: "林舟", description: "沉默的船夫。", text: "他不说话。\n<b>他看灯。</b>", origin: "author_request", source,
        changed_at: character.changed_at },
    ],
  });

  // Another Project of the same User has no entry, and another User cannot read the Project.
  const elsewhere = await prepare(started.baseUrl, id(`${ns}d1`), "Other Memory Novel", `${ns}e`);
  assert.deepEqual((await listMemoryEntries({ ...project, projectId: elsewhere.projectId })).entries, []);
  const stranger = sessionFetch(started.baseUrl, "session-b");
  await assert.rejects(() => listMemoryEntries({ ...project, fetchImpl: stranger }),
    (error) => unavailable(error, USER_A, "林舟", "灯塔"));
  await assert.rejects(() => listMemoryEntries({ ...project, projectId: elsewhere.projectId, fetchImpl: stranger }),
    (error) => unavailable(error, USER_A));
});

test("listMemoryEntries gives the conflict flag only for a flagged entry of its own Project", async () => {
  await drainLeftoverWork();
  const story = await createStory(started, { title: "Memory Flag Novel" });
  const other = await createStory(started, { title: "Other Memory Flag Novel" });
  const flagged = await seedMemoryEntry(story.projectId);
  const plain = await seedMemoryEntry(other.projectId);
  const chapterId = story.chapterIds[0] ?? "";
  await flagMemoryEntry(story.projectId, flagged, "第三章说他不撑船。", { kind: "chapter", chapter_id: chapterId });
  const list = (project: typeof story) => listMemoryEntries({ baseUrl: project.baseUrl, projectId: project.projectId,
    fetchImpl: project.fetchImpl });

  const [entry] = (await list(story)).entries;
  const [unflagged] = (await list(other)).entries;
  assert.ok(entry && unflagged);
  assert.match(entry.flag?.flagged_at ?? "", DATE_TIME);
  // The flag version keeps the text and the time of the last change. The entry with no flag has no flag key.
  assert.deepEqual([entry.version, entry.text, entry.flag, unflagged.memory_entry_id, "flag" in unflagged],
    ["2", "阿青在渡口撑船。", { reason: "第三章说他不撑船。", flagged_at: entry.flag?.flagged_at,
      source: { kind: "chapter", chapter_id: chapterId, title: "Chapter A", availability: "available" } },
    plain, false]);
  const stranger = sessionFetch(started.baseUrl, "session-b");
  await assert.rejects(() => list({ ...story, fetchImpl: stranger }),
    (error) => unavailable(error, USER_A, "第三章说他不撑船。"));
});
