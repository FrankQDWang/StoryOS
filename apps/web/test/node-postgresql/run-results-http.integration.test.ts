// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/derived-decision-kinds-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import { stopStoryOSServer as stopRealServer, queryStoryOSPostgres as queryPostgres } from "../support/node-integration.ts";
import { PROSE, TestServer, admitRun, createStory, drainLeftoverWork, freshNamespace, id, inspectRun, postRun,
  prepare, runRequest, settleHeld, settleOnce, settleRun, startServer, waitFor } from "../support/http-harness.ts";
import { fakeExaMcp } from "../support/research-journey-fakes.ts";

const ADVISORY = "This passage is inspectable Host-fake advice. It is not Authoritative State.";
// The Worker takes the test-only research key from its process environment and reads no env file (ADR 0050).
const WORKER_ENV = { STORYOS_EXA_API_KEY: "test-only-exa-key" };
// The fake records the Research Artifact in one RunStep, then makes the Proposal in the next RunStep.
const RESEARCH_THEN_PROPOSAL = "SCRIPT:submit_research propose web_search "
  + '{"summary":"The tides of 1890.","claims":[{"text":"The tide rose at ten.","citations":["$SOURCE"]}]}';

test("the Run read gives each result in the order in which the model made them", async () => {
  const mcp = await fakeExaMcp(fileURLToPath(new URL("../../../..", import.meta.url)));
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const ns = freshNamespace();
    const prepared = await prepare(started.baseUrl, id(`${ns}c0`), "Results Novel", `${ns}d`);
    const created = await admitRun(started.baseUrl, prepared, id(`${ns}e1`), RESEARCH_THEN_PROPOSAL);
    await queryPostgres(`
      UPDATE storyos.processing_destination_identity_evidence_revisions SET endpoint = '${mcp.base}'
       WHERE project_id = '${prepared.projectId}'::uuid AND evidence_kind = 'mcp_remote_boundary'`);
    await settleOnce(WORKER_ENV);
    const run = await inspectRun(started.baseUrl, prepared, created.effect.run_id);
    assert.equal(run.status, "completed");

    const [researchArtifactId, researchDecisionId] = (await queryPostgres(`
      SELECT research_artifact_id::text || '|' || decision_id::text FROM storyos.research_artifacts
       WHERE run_id = '${created.effect.run_id}'::uuid`)).split("|");
    // The derived decision keeps the Proposal for a client that reads one kind (ADR 0091 section 6).
    assert.ok(run.decision.kind === "prose_change" && run.decision.opened_proposal.kind === "present");
    const proposalId = run.decision.opened_proposal.proposal_id;
    assert.deepEqual(run.results?.map((result) => result.kind), ["research_artifact", "proposal", "answer"]);
    const [research, proposal, answer] = run.results ?? [];
    assert.deepEqual(research, { kind: "research_artifact", decision_id: researchDecisionId,
      research_artifact_id: researchArtifactId });
    assert.ok(proposal?.kind === "proposal");
    assert.deepEqual([proposal.decision_id, proposal.proposal_id, proposal.locations],
      [run.decision.decision_id, proposalId, run.decision.locations]);
    assert.deepEqual(proposal.locations.map((location) => [location.chapter_id, location.candidate_text]),
      [[prepared.chapterId, PROSE]]);
    assert.ok(answer?.kind === "answer");
    assert.equal(answer.text, ADVISORY);
    assert.notEqual(answer.decision_id, proposal.decision_id);

    // A read of one Model Attempt gives the Decision of its RunStep and no results of the Run.
    const closing = await inspectRun(started.baseUrl, prepared, created.effect.run_id,
      run.model_attempt.kind === "present" ? run.model_attempt.model_attempt_id : "");
    assert.ok(closing.decision.kind === "advisory");
    assert.equal(closing.decision.text, ADVISORY);
    assert.equal(closing.results, undefined);
  } finally {
    await stopRealServer(started.server);
    await mcp.close();
  }
});

// ADR 0094 section 5: each settled `write_memory` call is a result of its own Run, in the Scope of its Project.
test("the Run read of a Project gives only the Memory writes of its own Run, also after a cancel", async () => {
  await drainLeftoverWork();
  const started = await startServer();
  try {
    const server = TestServer.at(started.baseUrl);
    /** Admits one Run in `story` that writes the entry `title`, and gives its identity. */
    const admitted = async (story: Awaited<ReturnType<typeof createStory>>, title: string) => {
      const create = { type: "other", title, description: "一条记忆。", text: `${title}会再出现。` };
      const request = runRequest(story.chapterIds[0] ?? "", story.key(), `SCRIPT:write_memory create ${JSON.stringify(create)}`);
      const created = await postRun(started.baseUrl, story.fetchImpl, story.projectId, story.key(), request);
      assert.equal(created.effect.kind, "admitted");
      return created.effect.kind === "admitted" ? created.effect.run_id : "";
    };
    /** Creates one Project, and admits and settles one Run that writes the entry `title`. */
    const written = async (title: string) => {
      const story = await createStory(server, { title: `${title} Novel`, assistance: true, chapters: ["Chapter A"] });
      return { story, run: await settleRun(started.baseUrl, story, await admitted(story, title)) };
    };
    const bell = await written("铜铃");
    const boat = await written("林舟");

    for (const [{ run }, title] of [[bell, "铜铃"], [boat, "林舟"]] as const) {
      assert.equal(run.status, "completed");
      assert.deepEqual(run.results?.map((result) => result.kind), ["memory_write", "answer"]);
      const [write] = run.results ?? [];
      assert.ok(write?.kind === "memory_write");
      assert.deepEqual([write.action, write.title], ["create", title]);
      // A Memory write gives no decision kind: the public decision stays the final answer.
      assert.equal(run.decision.kind, "advisory");
    }
    // The other Project has no such Run.
    await assert.rejects(inspectRun(started.baseUrl, boat.story, bell.run.run_id));

    // The author cancels a Run after its write settled. The hold files are in a folder of this test only (#1619).
    const holds = mkdtempSync(join(tmpdir(), "storyos-1577-"));
    const [dispatchHold, streamHold] = [join(holds, "dispatch.hold"), join(holds, "stream.hold")];
    try {
      const runId = await admitted(bell.story, "海雾");
      const cancel = await bell.story.challenge("cancelAgentRun", { path: { runId }, request: {
        command_schema: "storyos.command.cancel-agent-run.request.v1", cancel_agent_run_input: bell.story.binding() } });
      writeFileSync(dispatchHold, "hold");
      writeFileSync(streamHold, "hold");
      const worker = settleHeld({ STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: dispatchHold,
        STORYOS_TEST_FAKE_STREAM_HOLD_PATH: streamHold });
      const read = () => inspectRun(started.baseUrl, bell.story, runId);
      // The Worker waits at the first request, then at the end of its stream.
      await waitFor(read, (run) => run.status === "claimed" && run.model_attempt.kind === "present");
      unlinkSync(dispatchHold);
      await waitFor(read, (run) => run.items.length > 0);
      // The next request waits, and the first response runs its write.
      writeFileSync(dispatchHold, "hold");
      unlinkSync(streamHold);
      await waitFor(read, (run) => run.results?.some((result) => result.kind === "memory_write") === true);
      assert.equal((await cancel.send()).response.effect.kind, "applied");
      unlinkSync(dispatchHold);
      await worker;
      const cancelled = await settleRun(started.baseUrl, bell.story, runId);
      assert.equal(cancelled.status, "cancelled");
      assert.deepEqual(cancelled.results?.map((result) => result.kind === "memory_write"
        ? [result.action, result.title] : result.kind), [["create", "海雾"]]);
    } finally {
      rmSync(holds, { recursive: true, force: true });
    }
  } finally {
    await stopRealServer(started.server);
  }
  await drainLeftoverWork();
});
