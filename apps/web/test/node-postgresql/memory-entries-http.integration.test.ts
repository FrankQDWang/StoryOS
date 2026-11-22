// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { randomBytes, randomUUID } from "node:crypto";
import { test } from "vitest";

import { listMemoryEntries } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres, requireStoryOSProtocolError, sessionFetch } from "../support/node-integration.ts";
import { USER_A, admitRun, createStory, deleteChapterInput, drainLeftoverWork, flagMemoryEntry, id, prepare,
  seedMemoryEntry, serverForFile, settleOnce } from "../support/http-harness.ts";

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
  await assert.rejects(() => listMemoryEntries({ baseUrl: started.baseUrl, projectId: story.projectId, fetchImpl: stranger }),
    (error) => unavailable(error, USER_A, "第三章说他不撑船。"));
});

/** The statement that adds one to the Memory revision of the Project and gives the new revision. */
function nextRevision(projectId: string, entries = 0): string {
  return `UPDATE storyos.project_memory_heads SET memory_revision = memory_revision + 1,
      next_entry_number = next_entry_number + ${entries}
    WHERE owner_user_id = '${USER_A}' AND project_id = '${projectId}'
    RETURNING memory_revision, next_entry_number - ${entries} AS entry_number`;
}

/** Writes the part of the background work of the character `title`, as the capture of a Chapter writes it. */
async function backgroundPart(projectId: string, title: string): Promise<string> {
  const memoryEntryId = randomUUID();
  await queryStoryOSPostgres(`
    WITH head AS (${nextRevision(projectId, 1)})
    INSERT INTO storyos.memory_entry_versions (owner_user_id, project_id, memory_entry_id, entry_number, version,
      memory_revision, state, entry_type, origin, title, description, text, cause, source_references)
    SELECT '${USER_A}', '${projectId}', '${memoryEntryId}', entry_number, 1, memory_revision, 'current', 'character',
      'chapter', '${title}', '渡口的人。', '${title}在渡口。', '{"kind": "chapter_capture"}', '[]' FROM head`);
  return memoryEntryId;
}

/**
 * Writes the version 2 of the part of the background work `memoryEntryId` with the result of one intent pass of the
 * Run `runId`, as an intent pass writes it (ADR 0101 section 4). The pass read the version 1 of the part and the
 * version 1 of the part of the author `authorEntryId`.
 */
async function recordIntent(projectId: string, memoryEntryId: string, runId: string, chapterId: string,
  authorEntryId: string | null, intent: string, questions: readonly string[]): Promise<void> {
  const passId = randomUUID();
  const [author, version] = authorEntryId === null ? ["NULL", "NULL"] : [`'${authorEntryId}'::uuid`, "1"];
  await queryStoryOSPostgres(`
    WITH pass AS (INSERT INTO storyos.intent_passes (owner_user_id, project_id, pass_id, run_id, memory_entry_id,
        entry_version, author_entry_id, author_version, chapter_id, outcome)
      VALUES ('${USER_A}', '${projectId}', '${passId}', '${runId}', '${memoryEntryId}', 1, ${author}, ${version},
        '${chapterId}', 'recorded')),
         head AS (${nextRevision(projectId)})
    INSERT INTO storyos.memory_entry_versions (owner_user_id, project_id, memory_entry_id, entry_number, version,
      memory_revision, state, entry_type, origin, title, description, text, cause, source_references, intent,
      intent_questions, intent_chapter_id, intent_author_entry_id, intent_author_version)
    SELECT owner_user_id, project_id, memory_entry_id, entry_number, 2, head.memory_revision, state, entry_type,
      origin, title, description, text, '{"kind": "intent_pass", "pass_id": "${passId}"}', source_references,
      '${intent}', '${JSON.stringify(questions)}', '${chapterId}', ${author}, ${version}
      FROM storyos.memory_entry_versions, head
     WHERE project_id = '${projectId}' AND memory_entry_id = '${memoryEntryId}' AND version = 1`);
}

/** Writes a new version of the entry `memoryEntryId` with a new text. */
async function changeText(projectId: string, memoryEntryId: string): Promise<void> {
  await queryStoryOSPostgres(`
    WITH head AS (${nextRevision(projectId)})
    INSERT INTO storyos.memory_entry_versions (owner_user_id, project_id, memory_entry_id, entry_number, version,
      memory_revision, state, entry_type, origin, title, description, text, cause, source_references)
    SELECT owner_user_id, project_id, memory_entry_id, entry_number, version + 1, head.memory_revision, state,
      entry_type, origin, title, description, text || '后来他走了。', '{"kind": "author_request"}', source_references
      FROM storyos.memory_entry_versions, head
     WHERE project_id = '${projectId}' AND memory_entry_id = '${memoryEntryId}'
     ORDER BY version DESC LIMIT 1`);
}

test("listMemoryEntries gives the current intent result only for a character entry of the background work (ADR 0101 sections 4 and 6)", async () => {
  await drainLeftoverWork();
  const story = await createStory(started, { title: "Memory Intent Novel", chapters: ["第一章", "第二章"] });
  const [kept = "", removed = ""] = story.chapterIds;
  const prepared = { projectId: story.projectId, fetchImpl: story.fetchImpl, chapterId: kept };
  const ns = randomBytes(3).toString("hex");
  const first = await admitRun(started.baseUrl, prepared, id(`${ns}a1`), [
    write("create", { type: "character", title: "林舟", description: "船夫。", text: "他记得母亲的话。" }),
    write("create", { type: "foreshadowing", title: "灯塔", description: "灯塔的灯。", text: "灯每夜亮一次。" }),
  ].join("\n"));
  await settleOnce();
  const second = await admitRun(started.baseUrl, prepared, id(`${ns}b1`), write("create", {
    type: "relationship", title: "林舟与阿青", description: "旧友。", text: "他们一起长大。" }));
  await settleOnce();
  const list = async () => (await listMemoryEntries({ baseUrl: story.baseUrl, projectId: story.projectId,
    fetchImpl: story.fetchImpl })).entries;
  const author = (await list()).find((entry) => entry.title === "林舟");
  assert.ok(author);
  const linZhou = await backgroundPart(story.projectId, "林舟");
  const aQing = await backgroundPart(story.projectId, "阿青");
  const runId = (run: typeof first) => run.effect.kind === "admitted" ? run.effect.run_id : "";
  await recordIntent(story.projectId, linZhou, runId(first), kept, author.memory_entry_id,
    "他想在涨水前把船修好，再去问母亲那封信的事。", ["母亲的信在谁手里？", "他知道阿青要走吗？"]);
  await recordIntent(story.projectId, aQing, runId(second), removed, null, "她想离开渡口。", []);
  assert.equal((await story.send("deleteChapter",
    deleteChapterInput(story, removed, await story.treeRevision()))).response.effect.kind, "authoritative_applied");

  // Only the two parts of the background work have the result. The Chapter of a deleted result is unavailable.
  const intents = async () => Object.fromEntries((await list()).map((entry) =>
    [`${entry.title} ${entry.origin}`, "intent" in entry ? entry.intent : "none"]));
  const shown = await intents();
  const recorded = (title: string) => {
    const result = shown[`${title} chapter`];
    return typeof result === "object" ? result.recorded_at : "";
  };
  assert.match(recorded("林舟"), DATE_TIME);
  assert.match(recorded("阿青"), DATE_TIME);
  assert.deepEqual(shown, {
    "灯塔 author_request": "none",
    "林舟 author_request": "none",
    "林舟 chapter": { text: "他想在涨水前把船修好，再去问母亲那封信的事。",
      questions: ["母亲的信在谁手里？", "他知道阿青要走吗？"], recorded_at: recorded("林舟"),
      chapter: { kind: "chapter", chapter_id: kept, title: "第一章", availability: "available" } },
    "阿青 chapter": { text: "她想离开渡口。", questions: [], recorded_at: recorded("阿青"),
      chapter: { kind: "chapter", chapter_id: removed, title: null, availability: "unavailable" } },
    "林舟与阿青 author_request": "none",
  });

  // A change of the part of the background work, and then a change of the part of the author, ends the result.
  await changeText(story.projectId, aQing);
  assert.deepEqual([(await intents())["阿青 chapter"], typeof (await intents())["林舟 chapter"]], ["none", "object"]);
  await changeText(story.projectId, author.memory_entry_id);
  assert.equal((await intents())["林舟 chapter"], "none");
});
