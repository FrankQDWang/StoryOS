// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/complete-fake-model-decision-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import { getResearchArtifact } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { PROSE, admitRun, drainLeftoverWork, freshNamespace, id, inspectRun, prepare, settleOnce, startServer }
  from "../support/http-harness.ts";
import { fakeExaMcp } from "../support/research-journey-fakes.ts";

const ADVISORY = "This passage is inspectable Host-fake advice. It is not Authoritative State.";
// The Worker takes the test-only research key from its process environment and reads no env file (ADR 0050).
const WORKER_ENV = { STORYOS_EXA_API_KEY: "test-only-exa-key" };
const RESEARCH = "SCRIPT:submit_research once web_search "
  + '{"summary":"The tides of 1890.","claims":[{"text":"The tide rose at ten.","citations":["$SOURCE"]}]}';

test("the tool loop fake derives advisory, prose_change with locations, and research_synthesis", async () => {
  const mcp = await fakeExaMcp(fileURLToPath(new URL("../../../..", import.meta.url)));
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const ns = freshNamespace();
    const prepared = await prepare(started.baseUrl, id(`${ns}c0`), "Derived Novel", `${ns}d`);
    const settled = async (key: string, text: string, before: () => Promise<unknown> = async () => {}) => {
      const created = await admitRun(started.baseUrl, prepared, id(`${ns}${key}`), text);
      await before();
      await settleOnce(WORKER_ENV);
      const run = await inspectRun(started.baseUrl, prepared, created.effect.run_id);
      assert.equal(run.status, "completed");
      return run.decision;
    };

    // A final answer with no Proposal and no Research Artifact is advisory (ADR 0091).
    const advisory = await settled("e1", "Help with this passage.");
    assert.ok(advisory.kind === "advisory");
    assert.equal(advisory.text, ADVISORY);
    assert.equal(advisory.selected, true);

    // A `propose_edit` call opens a Proposal, and the decision gives its locations.
    const prose = await settled("e2", "Revise this passage: keep the voice.");
    assert.ok(prose.kind === "prose_change");
    assert.equal(prose.authoritative, false);
    assert.equal(prose.opened_proposal.kind, "present");
    assert.deepEqual(prose.locations?.map((location) => [location.chapter_id, location.candidate_text]),
      [[prepared.chapterId, PROSE]]);

    // A `submit_research` call writes a Research Artifact, and the decision gives its identity.
    const research = await settled("e3", RESEARCH, () => queryPostgres(`
      UPDATE storyos.processing_destination_identity_evidence_revisions SET endpoint = '${mcp.base}'
       WHERE project_id = '${prepared.projectId}'::uuid AND evidence_kind = 'mcp_remote_boundary'`));
    assert.ok(research.kind === "research_synthesis");
    const { artifact } = await getResearchArtifact({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      researchArtifactId: research.research_artifact_id, fetchImpl: prepared.fetchImpl });
    assert.deepEqual([artifact.summary, artifact.claims.map((claim) => claim.text)],
      ["The tides of 1890.", ["The tide rose at ten."]]);
    assert.deepEqual(mcp.calls().map((call) => (call.params as { name: string }).name), ["web_search_exa"]);
  } finally {
    await stopRealServer(started.server);
    await mcp.close();
  }
});
