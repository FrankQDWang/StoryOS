// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { test } from "vitest";

import { TestServer, createStory, drainLeftoverWork, postRun, settleRun, startServer } from "../support/http-harness.ts";
import { inspectRun, runRequest, settleOnce } from "../support/acceptance.ts";
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
    // A search, two reads of Fog, a read of a number that no Chapter has, and a search by meaning. The Host refuses the
    // read and the search by meaning: the deployment has no embedding configuration.
    const used = await settled("SCRIPT:search_manuscript Lin\nSCRIPT:read_chapter 2\nSCRIPT:read_chapter 2\n"
      + "SCRIPT:read_chapter 9\nSCRIPT:hybrid_search_manuscript 谁在渡口等船？");
    const plain = await settled("Help with this passage.");
    const attempt = await queryStoryOSPostgres(`
      SELECT model_attempt_id::text FROM storyos.model_attempts
       WHERE run_id = '${used.run_id}'::uuid AND attempt_role = 'decision' AND decision_position = 0`);
    const first = await inspectRun(started.baseUrl, story, used.run_id, attempt);
    // The Tool Exposure of the request has the keyword search and the read, and no hybrid search.
    const exposed = await queryStoryOSPostgres(`
      SELECT string_agg(tool ->> 'name', ' ' ORDER BY tool ->> 'name')
        FROM storyos.model_attempts, jsonb_array_elements(payload #> '{step_snapshot,tool_exposure,tools}') AS tool
       WHERE model_attempt_id = '${attempt}'::uuid
         AND tool ->> 'name' IN ('search_manuscript', 'read_chapter', 'hybrid_search_manuscript')`);

    assert.deepEqual([used.status, plain.status], ["completed", "completed"]);
    assert.equal(exposed, "read_chapter search_manuscript");
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

// ADR 0095 section 1: with the fake embedding configuration, each question of a settled search by meaning is one entry.
// The entry has no passage text, no vector, and no score. A refused call gives no entry.
test("the Run read of getAgentRun gives each question of a search by meaning", async () => {
  await drainLeftoverWork();
  const embedding = { STORYOS_EMBEDDING_DESTINATION: "host_fake" };
  const started = await startServer({ extraEnv: embedding });
  try {
    const story = await createStory(TestServer.at(started.baseUrl),
      { title: "Hybrid Search Novel", assistance: true, chapters: ["Chapter A"] });
    const request = runRequest(story.chapterIds[0] ?? "", story.key(), "SCRIPT:hybrid_search_manuscript 谁在渡口等船？|Lin\n"
      + "SCRIPT:hybrid_search_manuscript |Lin\nSCRIPT:hybrid_search_manuscript 谁在渡口等船？");
    const created = await postRun(started.baseUrl, story.fetchImpl, story.projectId, story.key(), request);
    assert.equal(created.effect.kind, "admitted");
    const runId = created.effect.kind === "admitted" ? created.effect.run_id : "";
    // The Worker of the deployment has the same embedding configuration.
    let run = await inspectRun(started.baseUrl, story, runId);
    for (let pass = 0; pass < 8 && run.status !== "completed"; pass += 1) {
      await settleOnce(embedding);
      run = await inspectRun(started.baseUrl, story, runId);
    }
    const states = await queryStoryOSPostgres(`
      SELECT string_agg(state, ',' ORDER BY decision_position, call_position) FROM storyos.tool_calls
       WHERE run_id = '${runId}'::uuid AND function_name = 'hybrid_search_manuscript'`);

    assert.deepEqual([run.status, states, run.manuscript_uses],
      ["completed", "settled,refused,settled", [{ kind: "hybrid_search", question: "谁在渡口等船？" }]]);
  } finally {
    await stopStoryOSServer(started.server);
  }
  await drainLeftoverWork();
});
