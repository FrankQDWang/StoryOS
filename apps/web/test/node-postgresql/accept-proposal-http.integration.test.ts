// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/edit-proposal-candidate-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import { getChapter, getProposal } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { GetProposalResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { PROSE, REVISED, reviseCandidate } from "../support/acceptance.ts";
import { UUID_V7, assertForeignRefused, createStory, drainLeftoverWork, freshNamespace, openProposal, problemOf,
  serverForFile,
} from "../support/http-harness.ts";
import type { CommandInput, TestStory } from "../support/http-harness.ts";

// The Profile Probe of the AuthoritativeRevision settlement profile (ADR 0075). The Author Edit test of
// project-http is its second case.
const server = serverForFile();

/** A Proposal with a revised, validated candidate on the Chapter Head, and the writer Editor Session of the revision. */
async function revisedProposal(title: string) {
  await drainLeftoverWork();
  const story = await createStory(server, { title, assistance: true });
  const chapter = async () => (await getChapter({ baseUrl: story.baseUrl, projectId: story.projectId,
    chapterId: story.chapterIds[0] ?? "", fetchImpl: story.fetchImpl })).chapter;
  const before = await chapter();
  const opened = await openProposal(story);
  assert.equal(opened.proposal.candidate_text, PROSE);
  const { session, revised } = await reviseCandidate(story.baseUrl, story.fetchImpl, story.projectId, opened,
    freshNamespace());
  assert.equal(revised.proposal.candidate_text, REVISED);
  const inspect = async () => (await getProposal({ baseUrl: story.baseUrl, projectId: story.projectId,
    proposalId: opened.proposal.proposal_id, fetchImpl: story.fetchImpl })).proposal;
  return { story, before, chapter, inspect, opened: opened.proposal, revised: revised.proposal, session };
}

function acceptance(story: TestStory, proposal: GetProposalResponse["proposal"], options: {
  validationReceiptId?: string; expectedRevisionId: string; editorSessionId: string;
}): CommandInput<"acceptProposal"> {
  if (proposal.validation_receipt.kind !== "present") throw new Error("expected a Validation Receipt");
  return { request: {
    command_schema: "storyos.command.accept-proposal.request.v1",
    accept_proposal_input: {
      proposal_revision_id: proposal.revision_id,
      validation_receipt_id: options.validationReceiptId ?? proposal.validation_receipt.validation_receipt_id,
      selected_operation_ids: [proposal.operation_id],
      expected_authoritative_revision_id: options.expectedRevisionId,
      editor_session_id: options.editorSessionId,
      ...story.binding(),
    },
  }, path: { proposalId: proposal.proposal_id } };
}

// acceptProposal has no Conflicted outcome since ADR 0064, so the Author Edit test of project-http proves the conflict.
test("acceptProposal applies, replays, and refuses", async () => {
  const { story, before, chapter, inspect, revised, session } = await revisedProposal("Accept Proposal Novel");
  const editorSessionId = session.editor_session.editor_session_id;
  const expectedRevisionId = before.current_revision.revision_id;
  const accepted = await story.send("acceptProposal", acceptance(story, revised, { expectedRevisionId, editorSessionId }));
  const { effect, receipt } = accepted.response;
  assert.equal(receipt.result, "applied");
  if (effect.kind !== "applied") throw new Error("expected applied");
  assert.match(effect.authoritative_commit_id, UUID_V7);
  assert.match(effect.authoritative_revision.revision_id, UUID_V7);
  assert.equal(effect.authoritative_revision.body, REVISED);
  assert.deepEqual(await accepted.command.send(), { response: accepted.response, body: accepted.body });
  const inspected = await inspect();
  assert.deepEqual([inspected.operation_resolution, inspected.reservation_state, inspected.revision_id],
    ["applied", "resolved", revised.revision_id]);
  const after = await chapter();
  assert.deepEqual([after.current_revision.body, after.current_revision.revision_id],
    [REVISED, effect.authoritative_revision.revision_id]);

  const changed = acceptance(story, revised, { expectedRevisionId, editorSessionId: story.key() });
  const { status, code } = await problemOf(accepted.command.send({ request: changed.request }));
  assert.deepEqual({ status, code }, { status: 422, code: "challenge_invalid" });
  await assertForeignRefused(story, "acceptProposal", acceptance(story, revised, { expectedRevisionId, editorSessionId }));
  const wrongAdmission = await problemOf(story.send("acceptProposal",
    acceptance(story, revised, { expectedRevisionId, editorSessionId: story.key() })));
  assert.equal(wrongAdmission.status, 422);
  const again = await story.send("acceptProposal", acceptance(story, revised,
    { expectedRevisionId: after.current_revision.revision_id, editorSessionId }));
  assert.deepEqual(again.response.effect, { kind: "refused", reason: "operation_not_pending" });
});

// The recovery seed runs this test: it retains one invalid Acceptance condition.
test("acceptProposal retains invalid_validation across reload and refuses another attempt until revision", async () => {
  const { story, before, chapter, inspect, opened, revised, session } = await revisedProposal("Refuse Acceptance Novel");
  const editorSessionId = session.editor_session.editor_session_id;
  const expectedRevisionId = before.current_revision.revision_id;
  if (opened.validation_receipt.kind !== "present") throw new Error("expected a Validation Receipt");
  const stale = await story.send("acceptProposal", acceptance(story, opened, { expectedRevisionId, editorSessionId }));
  assert.deepEqual(stale.response.effect, { kind: "refused", reason: "stale_proposal_revision" });

  const failed = await story.send("acceptProposal", acceptance(story, revised,
    { validationReceiptId: opened.validation_receipt.validation_receipt_id, expectedRevisionId, editorSessionId }));
  assert.deepEqual(failed.response.effect, { kind: "invalid", reason: "invalid_validation" });
  assert.deepEqual(failed.response.receipt.condition_refs, []);
  const expected = { ...revised, validation: "invalid", condition_refs: [] };
  assert.deepEqual(await inspect(), expected);
  const replayed = await Promise.all([0, 1].map(() => failed.command.send()));
  for (const replay of replayed) assert.deepEqual(replay, { response: failed.response, body: failed.body });

  const refused = await story.send("acceptProposal", acceptance(story, revised, { expectedRevisionId, editorSessionId }));
  assert.deepEqual(refused.response.effect, { kind: "refused", reason: "not_eligible" });
  await server.restart();
  assert.deepEqual(await inspect(), expected);
  assert.deepEqual(await chapter(), before);

  const pending = await getProposal({ baseUrl: story.baseUrl, projectId: story.projectId,
    proposalId: revised.proposal_id, fetchImpl: story.fetchImpl });
  assert.equal(pending.proposal.operation_resolution, "pending");
  const recovered = (await reviseCandidate(story.baseUrl, story.fetchImpl, story.projectId, pending,
    freshNamespace(), session)).revised.proposal;
  assert.deepEqual([recovered.validation, recovered.condition_refs], ["valid", []]);
  const applied = await story.send("acceptProposal", acceptance(story, recovered, { expectedRevisionId, editorSessionId }));
  assert.equal(applied.response.effect.kind, "applied");
  assert.deepEqual(await failed.command.send(), { response: failed.response, body: failed.body });
});
