// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/derived-decision-kinds-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import { stopStoryOSServer as stopRealServer, queryStoryOSPostgres as queryPostgres } from "../support/node-integration.ts";
import { PROSE, admitRun, drainLeftoverWork, freshNamespace, id, inspectRun, prepare, settleOnce, startServer }
  from "../support/http-harness.ts";
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
