// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/open-block-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { test } from "vitest";

import {
  getProjectConversation,
  listProjectConversations,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
} from "../support/node-integration.ts";
import { USER_A, admitRun, drainLeftoverWork, id, prepare, serverForFile, settleOnce } from "../support/http-harness.ts";

const started = serverForFile();

// The first message is longer than the label bound of 80 characters, and each character of the start is CJK.
const FIRST = `${"雨".repeat(79)}落在窗沿。`;
const OTHER = "Write about the storm.";
const SECOND = "Keep the voice of the narrator.";
const DATE_TIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;

/** The rows of the Project in the tables that a Run and a read could change. */
async function rowCounts(projectId: string): Promise<string> {
  return queryPostgres(`
    SELECT (SELECT count(*) FROM storyos.domain_receipts WHERE project_id = '${projectId}'::uuid)::text || ' '
        || (SELECT count(*) FROM storyos.agent_run_events WHERE project_id = '${projectId}'::uuid)::text || ' '
        || (SELECT string_agg(status, ',' ORDER BY run_id) FROM storyos.agent_runs
             WHERE project_id = '${projectId}'::uuid);
  `);
}

/** Moves each Run event time of each Run of the Conversation back by 73 hours. */
async function moveBack(projectId: string, conversationId: string): Promise<void> {
  await queryPostgres(`
    UPDATE storyos.agent_run_events AS event SET recorded_at = event.recorded_at - interval '73 hours'
      FROM storyos.agent_runs AS run
     WHERE run.project_id = '${projectId}'::uuid AND run.conversation_id = '${conversationId}'::uuid
       AND (event.owner_user_id, event.project_id, event.run_id) = (run.owner_user_id, run.project_id, run.run_id);
  `);
}

function unavailable(error: unknown, ...hidden: string[]): boolean {
  const protocol = requireStoryOSProtocolError(error);
  const body = String(protocol.responseBody);
  return protocol.status === 404 && hidden.every((value) => !body.includes(value));
}

test("listProjectConversations and getProjectConversation give the Conversations of one Project", async () => {
  await drainLeftoverWork();
  const ns = randomBytes(3).toString("hex");
  const prepared = await prepare(started.baseUrl, id(`${ns}a1`), "Conversation List Novel", `${ns}b`);
  const project = { baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl };
  const list = () => listProjectConversations(project);
  const read = (conversationId: string) => getProjectConversation({ ...project, conversationId });
  assert.deepEqual((await list()).conversations, []);

  const first = await admitRun(started.baseUrl, prepared, id(`${ns}c1`), FIRST);
  await settleOnce();
  const other = await admitRun(started.baseUrl, prepared, id(`${ns}c2`), OTHER);
  await settleOnce();
  const second = await admitRun(started.baseUrl, prepared, id(`${ns}c3`), SECOND,
    { kind: "existing", conversation_id: first.conversation_id });
  await settleOnce();
  const before = await rowCounts(prepared.projectId);

  const listed = await list();
  const conversation = await read(first.conversation_id);
  const otherRuns = (await read(other.conversation_id)).runs;
  const [firstRun, secondRun] = conversation.runs;
  assert.ok(firstRun && secondRun && otherRuns[0]);
  assert.match(firstRun.created_at, DATE_TIME);
  assert.deepEqual({ ...listed, correlation_id: "" }, {
    schema_id: "storyos.query.project-conversations.response.v1",
    correlation_id: "",
    project_scope: { owner_user_id: USER_A, project_id: prepared.projectId },
    completeness: "complete",
    conversations: [
      { conversation_id: first.conversation_id, label: "雨".repeat(79) + "落",
        first_run_created_at: firstRun.created_at, newest_run_created_at: secondRun.created_at,
        newest_run_status: "completed" },
      { conversation_id: other.conversation_id, label: OTHER,
        first_run_created_at: otherRuns[0].created_at, newest_run_created_at: otherRuns[0].created_at,
        newest_run_status: "completed" },
    ],
  });
  assert.deepEqual({ ...conversation, correlation_id: "" }, {
    schema_id: "storyos.query.project-conversation.response.v1",
    correlation_id: "",
    project_scope: { owner_user_id: USER_A, project_id: prepared.projectId },
    conversation_id: first.conversation_id,
    completeness: "complete",
    earlier_turns_expired: false,
    runs: [
      { run_id: first.effect.run_id, author_message: FIRST, chapter_id: prepared.chapterId, status: "completed",
        created_at: firstRun.created_at },
      { run_id: second.effect.run_id, author_message: SECOND, chapter_id: prepared.chapterId, status: "completed",
        created_at: secondRun.created_at },
    ],
  });
  assert.deepEqual(otherRuns.map((run) => run.run_id), [other.effect.run_id]);

  // The newest Run event of the Conversation is more than 72 hours old at the time of the read.
  await moveBack(prepared.projectId, first.conversation_id);
  const moved = await rowCounts(prepared.projectId);
  assert.equal((await read(first.conversation_id)).earlier_turns_expired, true);
  assert.equal((await read(other.conversation_id)).earlier_turns_expired, false);
  assert.equal(await rowCounts(prepared.projectId), moved);
  assert.equal(moved, before);

  // Another User, a Conversation of another Project, and an unknown Conversation are unavailable.
  const stranger = browserFetch(started.baseUrl, "session-b");
  await assert.rejects(() => listProjectConversations({ ...project, fetchImpl: stranger }),
    (error) => unavailable(error, USER_A, first.conversation_id));
  await assert.rejects(() => getProjectConversation({ ...project, fetchImpl: stranger,
    conversationId: first.conversation_id }), (error) => unavailable(error, USER_A, FIRST));
  const elsewhere = await prepare(started.baseUrl, id(`${ns}d1`), "Other Conversation Novel", `${ns}e`);
  assert.deepEqual((await listProjectConversations({ ...project, projectId: elsewhere.projectId })).conversations, []);
  await assert.rejects(() => getProjectConversation({ ...project, projectId: elsewhere.projectId,
    conversationId: first.conversation_id }), (error) => unavailable(error, FIRST));
  await assert.rejects(() => read(id(`${ns}f1`)), (error) => unavailable(error));
});
