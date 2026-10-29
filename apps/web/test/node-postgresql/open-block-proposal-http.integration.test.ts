// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/compact-active-context-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { test } from "vitest";

import {
  getAgentRun,
  getChapter,
  getProposal,
  listChapterOpenProposals,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  GetProposalResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";
import { PROSE, USER_A, UUID_V7, admitProse, drainLeftoverWork, id, prepare,
  startServer } from "../support/http-harness.ts";

function assertOpenedProposal(queried: Awaited<ReturnType<typeof getAgentRun>>): string {
  assert.equal(queried.status, "completed");
  assert.equal(queried.decision.kind, "prose_change");
  if (queried.decision.kind !== "prose_change") throw new Error("expected prose");
  assert.equal(queried.decision.opened_proposal.kind, "present");
  if (queried.decision.opened_proposal.kind !== "present") throw new Error("expected opened");
  assert.match(queried.decision.opened_proposal.proposal_id, UUID_V7);
  return queried.decision.opened_proposal.proposal_id;
}

function assertSameProposal(first: GetProposalResponse, second: GetProposalResponse) {
  assert.deepEqual(
    { ...first.proposal, source: first.proposal.source, validation_receipt: first.proposal.validation_receipt },
    { ...second.proposal, source: second.proposal.source, validation_receipt: second.proposal.validation_receipt },
  );
}

test("Worker opens one Block Proposal in place without changing Authoritative prose", async () => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("f111"), "Open Proposal Novel", "f2");
    const before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const queried = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("f131"));
    const proposalId = assertOpenedProposal(queried);
    if (queried.decision.kind !== "prose_change") throw new Error("expected prose");
    const inspected = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(inspected.proposal.kind, "block_edit");
    assert.equal(inspected.proposal.generation, "ready");
    assert.equal(inspected.proposal.validation, "valid");
    assert.equal(inspected.proposal.closure, "open");
    assert.equal(inspected.proposal.operation_resolution, "pending");
    assert.equal(inspected.proposal.reservation_state, "unresolved");
    assert.equal(inspected.proposal.candidate_text, PROSE);
    assert.equal(inspected.proposal.chapter_id, prepared.chapterId);
    assert.equal(inspected.proposal.base_authoritative_revision_id, before.chapter.current_revision.revision_id);
    assert.match(inspected.proposal.proposal_id, UUID_V7);
    assert.match(inspected.proposal.revision_id, UUID_V7);
    assert.match(inspected.proposal.operation_id, UUID_V7);
    assert.match(inspected.proposal.manuscript_block_id, UUID_V7);
    assert.deepEqual(inspected.proposal.source, {
      kind: "agent_run_decision",
      run_id: queried.run_id,
      decision_id: queried.decision.decision_id,
    });
    assert.equal(inspected.proposal.validation_receipt.kind, "present");
    if (inspected.proposal.validation_receipt.kind !== "present") throw new Error("expected receipt");
    assert.match(inspected.proposal.validation_receipt.validation_receipt_id, UUID_V7);
    assert.equal(inspected.proposal.validation_receipt.result, "valid");
    const reloaded = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId,
      fetchImpl: prepared.fetchImpl,
    });
    assertSameProposal(inspected, reloaded);
    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(after.chapter, before.chapter);
    await assert.rejects(
      () => getProposal({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        proposalId,
        fetchImpl: browserFetch(started.baseUrl, "session-b"),
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        const body = String(protocol.responseBody);
        return protocol.status === 404 && !body.includes(USER_A) && !body.includes(PROSE);
      },
    );
    await assert.rejects(
      () => getProposal({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        proposalId: id("dead"),
        fetchImpl: prepared.fetchImpl,
      }),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );
  } finally {
    await stopRealServer(started.server);
  }
});

test("Worker keeps two non-overlapping Block Proposals and refuses a reserved current Block", async () => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("f141"), "Reservation Novel", "f5");
    const first = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("f151"));
    const firstId = assertOpenedProposal(first);
    const overlap = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("f153"));
    assert.equal(overlap.decision.kind, "prose_change");
    if (overlap.decision.kind !== "prose_change") throw new Error("expected prose");
    assert.deepEqual(overlap.decision.opened_proposal, { kind: "absent" });
    const secondBlockId = id("f155");
    await queryPostgres(`
      WITH first AS (
        SELECT owner_user_id, project_id, manuscript_object_id, revision_id, manuscript_block_id
          FROM storyos.manuscript_revision_members
         WHERE project_id = '${prepared.projectId}'::uuid
           AND manuscript_object_id = '${prepared.chapterId}'::uuid
         ORDER BY block_order
         LIMIT 1
      ),
      ins_block AS (
        INSERT INTO storyos.manuscript_blocks
          (owner_user_id, project_id, manuscript_block_id, manuscript_object_id, block_kind)
        SELECT owner_user_id, project_id, '${secondBlockId}'::uuid, manuscript_object_id, 'paragraph'
          FROM first
      ),
      ins_member AS (
        INSERT INTO storyos.manuscript_revision_members
          (owner_user_id, project_id, manuscript_object_id, revision_id, manuscript_block_id, block_order)
        SELECT owner_user_id, project_id, manuscript_object_id, revision_id, '${secondBlockId}'::uuid, 2
          FROM first
      )
      UPDATE storyos.authoritative_payloads AS payload
         SET canonical_bytes = convert_to(
           json_build_object(
             'format', 'storyos.manuscript-payload.v1',
             'schema_version', 1,
             'coordinate_version', 1,
             'blocks', json_build_array(
               json_build_object(
                 'manuscript_block_id', first.manuscript_block_id,
                 'block_kind', 'paragraph',
                 'text', ''
               ),
               json_build_object(
                 'manuscript_block_id', '${secondBlockId}',
                 'block_kind', 'paragraph',
                 'text', ''
               )
             )
           )::text,
           'UTF8'
         )
        FROM first
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
              revision.revision_id) =
             (first.owner_user_id, first.project_id, first.manuscript_object_id, first.revision_id)
       WHERE (payload.owner_user_id, payload.project_id, payload.payload_id) =
             (revision.owner_user_id, revision.project_id, revision.payload_id);
    `);
    const second = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("f157"));
    const secondId = assertOpenedProposal(second);
    assert.notEqual(secondId, firstId);
    const firstProposal = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: firstId,
      fetchImpl: prepared.fetchImpl,
    });
    const secondProposal = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: secondId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(firstProposal.proposal.reservation_state, "unresolved");
    assert.equal(secondProposal.proposal.reservation_state, "unresolved");
    assert.notEqual(firstProposal.proposal.manuscript_block_id, secondProposal.proposal.manuscript_block_id);
    assert.equal(firstProposal.proposal.chapter_id, secondProposal.proposal.chapter_id);
  } finally {
    await stopRealServer(started.server);
  }
});

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

// The seeded Proposal identities are fixed, so that they sort before the opened Proposal. Each Project has its own
// Proposal identities.
const seeded = (index: number) => id(`c7d${index.toString(16).padStart(5, "0")}`);

test("listChapterOpenProposals gives the open Proposals of one Chapter in identity order", async () => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const prepared = await prepare(started.baseUrl, id(`${ns}a1`), "Open Proposal List Novel", `${ns}b`);
    const run = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id(`${ns}c1`));
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
