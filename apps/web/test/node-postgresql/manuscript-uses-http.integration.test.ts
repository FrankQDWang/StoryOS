// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { test } from "vitest";

import { TestServer, createStory, drainLeftoverWork, postRun, settleRun, startServer } from "../support/http-harness.ts";
import { inspectRun, runRequest } from "../support/acceptance.ts";
import { queryStoryOSPostgres, stopStoryOSServer } from "../support/node-integration.ts";

// ADR 0095 section 1: the Run read names each Chapter that the Agent read and each term that it searched.
test("the Run read of getAgentRun gives each manuscript Tool use of the Run", async () => {
  await drainLeftoverWork();
  const started = await startServer();
  try {
    const story = await createStory(TestServer.at(started.baseUrl),
      { title: "Manuscript Use Novel", assistance: true, chapters: ["Chapter A", "Fog"] });
    /** Admits one Run with `text`, and runs the Worker until the Run settles. */
    const settled = async (text: string) => {
      const request = runRequest(story.chapterIds[0] ?? "", story.key(), text);
      const created = await postRun(started.baseUrl, story.fetchImpl, story.projectId, story.key(), request);
      assert.equal(created.effect.kind, "admitted");
      return settleRun(started.baseUrl, story, created.effect.kind === "admitted" ? created.effect.run_id : "");
    };
    // A search, two reads of Fog, and a read of a number that no Chapter has. The Host refuses the last read.
    const used = await settled(
      "SCRIPT:search_manuscript Lin\nSCRIPT:read_chapter 2\nSCRIPT:read_chapter 2\nSCRIPT:read_chapter 9");
    const plain = await settled("Help with this passage.");
    const attempt = await queryStoryOSPostgres(`
      SELECT model_attempt_id::text FROM storyos.model_attempts
       WHERE run_id = '${used.run_id}'::uuid AND attempt_role = 'decision' AND decision_position = 0`);
    const first = await inspectRun(started.baseUrl, story, used.run_id, attempt);

    assert.deepEqual([used.status, plain.status], ["completed", "completed"]);
    assert.deepEqual(used.manuscript_uses, [
      { kind: "keyword_search", query: "Lin" },
      { kind: "chapter_read", chapter_id: story.chapterIds[1], title: "Fog" },
    ]);
    // A Run with no entry and a read of one Model Attempt have no such field.
    assert.deepEqual([Object.hasOwn(plain, "manuscript_uses"), Object.hasOwn(first, "manuscript_uses")],
      [false, false]);
  } finally {
    await stopStoryOSServer(started.server);
  }
  await drainLeftoverWork();
});
