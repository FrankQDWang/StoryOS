// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/withdraw-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { getProposal, listChapterOpenProposals,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, requireStoryOSProtocolError,
  sessionFetch as browserFetch, stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { USER_A, id, startRealServer, drainLeftoverWork, prepare, admitProse } from "../support/acceptance.ts";

type Seed = {
  proposalId: string;
  chapterId?: string;
  closure?: "open" | "withdrawn" | "superseded";
  validation?: "pending" | "valid" | "invalid" | "conflicted";
  generation?: "generating" | "ready_partial" | "ready";
  resolutions?: readonly ("pending" | "applied" | "rejected")[];
  draftSource?: boolean;
};

// Each seed copies the scope, the source, and the base of the opened Proposal. The fixed test identities sort before it.
function seedSql(template: string, seed: Seed): string {
  const revisionId = seed.proposalId.replace(/-0000c7d/, "-0000c7e");
  const source = seed.draftSource === true
    ? "NULL, NULL, gen_random_uuid(), gen_random_uuid(), repeat('a', 64)"
    : "source_run_id, source_decision_id, NULL, NULL, NULL";
  const operations = (seed.resolutions ?? ["pending"]).map((resolution) =>
    `INSERT INTO storyos.proposal_operations (owner_user_id, project_id, proposal_id, operation_id,
       manuscript_block_id, resolution, reservation_state, candidate_text)
     SELECT owner_user_id, project_id, '${seed.proposalId}', gen_random_uuid(), gen_random_uuid(),
       '${resolution}', '${resolution === "pending" ? "unresolved" : "resolved"}', 'Candidate'
       FROM storyos.proposals WHERE proposal_id = '${template}';`).join("\n");
  return `
    INSERT INTO storyos.proposals (owner_user_id, project_id, proposal_id, kind, chapter_id,
      manuscript_block_id, source_run_id, source_decision_id, source_draft_id,
      source_draft_revision_id, source_draft_payload_digest)
    SELECT owner_user_id, project_id, '${seed.proposalId}', kind,
      ${seed.chapterId === undefined ? "chapter_id" : `'${seed.chapterId}'`}, manuscript_block_id, ${source}
      FROM storyos.proposals WHERE proposal_id = '${template}';
    INSERT INTO storyos.proposal_revisions (owner_user_id, project_id, proposal_id, revision_id,
      generation, validation, closure, candidate_text, base_authoritative_revision_id)
    SELECT owner_user_id, project_id, '${seed.proposalId}', '${revisionId}', '${seed.generation ?? "ready"}',
      '${seed.validation ?? "valid"}', '${seed.closure ?? "open"}', 'Candidate', base_authoritative_revision_id
      FROM storyos.proposal_revisions WHERE proposal_id = '${template}';
    INSERT INTO storyos.proposal_heads (owner_user_id, project_id, proposal_id, current_revision_id)
    SELECT owner_user_id, project_id, '${seed.proposalId}', '${revisionId}'
      FROM storyos.proposals WHERE proposal_id = '${template}';
    ${operations}`;
}

async function seed(template: string, seeds: readonly Seed[]): Promise<void> {
  await queryPostgres(`BEGIN; SET LOCAL session_replication_role = replica;
    ${seeds.map((item) => seedSql(template, item)).join("\n")}
    COMMIT;`);
}

const seeded = (index: number) => id(`c7d${index.toString(16).padStart(5, "0")}`);

test("listChapterOpenProposals gives the open Proposals of one Chapter in identity order", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("c7a1"), "Open Proposal List Novel", "c7b");
    const run = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("c7c1"));
    if (run.decision.kind !== "prose_change" || run.decision.opened_proposal.kind !== "present") {
      throw new Error("expected opened prose");
    }
    const opened = (await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: run.decision.opened_proposal.proposal_id, fetchImpl: prepared.fetchImpl })).proposal;
    await seed(opened.proposal_id, [
      { proposalId: seeded(3), resolutions: ["applied", "pending"] },
      { proposalId: seeded(1), validation: "conflicted" },
      { proposalId: seeded(2), validation: "invalid", generation: "ready_partial" },
      { proposalId: seeded(4), resolutions: ["applied", "applied"] },
      { proposalId: seeded(5), resolutions: ["rejected"] },
      { proposalId: seeded(6), closure: "withdrawn" },
      { proposalId: seeded(7), closure: "superseded" },
      { proposalId: seeded(8), chapterId: id("c7f1") },
      { proposalId: seeded(9), draftSource: true },
    ]);
    const list = () => listChapterOpenProposals({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    const entry = (proposalId: string, revisionId: string) => ({ proposal_id: proposalId,
      revision_id: revisionId, kind: opened.kind, source: opened.source });
    const listed = await list();
    assert.deepEqual({ ...listed, correlation_id: "" }, {
      schema_id: "storyos.query.chapter-open-proposals.response.v1",
      correlation_id: "",
      project_scope: { owner_user_id: USER_A, project_id: prepared.projectId },
      chapter_id: prepared.chapterId,
      completeness: "complete",
      proposals: [
        ...[1, 2, 3].map((index) => entry(seeded(index), seeded(index).replace(/-0000c7d/, "-0000c7e"))),
        entry(opened.proposal_id, opened.revision_id),
      ],
    });

    await seed(opened.proposal_id, Array.from({ length: 61 }, (_, index) => ({ proposalId: seeded(16 + index) })));
    const truncated = await list();
    assert.equal(truncated.completeness, "truncated");
    assert.deepEqual(truncated.proposals.map((item) => item.proposal_id),
      [1, 2, 3, ...Array.from({ length: 61 }, (_, index) => 16 + index)].map(seeded));

    await assert.rejects(
      () => listChapterOpenProposals({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        chapterId: prepared.chapterId, fetchImpl: browserFetch(started.baseUrl, "session-b") }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        const body = String(protocol.responseBody);
        return protocol.status === 404 && !body.includes(USER_A) && !body.includes(opened.proposal_id);
      },
    );
    await assert.rejects(() => listChapterOpenProposals({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: id("c7f1"), fetchImpl: prepared.fetchImpl }),
    (error) => requireStoryOSProtocolError(error).status === 404);
  } finally {
    await stopRealServer(started.server);
  }
});
