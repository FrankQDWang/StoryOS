// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/accept-earlier-base-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { digestUndoLatestAuthorAction, getChapter, getProposal, undoLatestAuthorAction,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { UndoLatestAuthorActionRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { BINDING, REVISED, accept, acceptRequest, advanceChapterHead, challenged, drainLeftoverWork, id,
  openedBlockProposal, reject, seedAcceptanceCondition, sendAuthorEdit, startRealServer } from "../support/acceptance.ts";
import type { Started } from "../support/acceptance.ts";

const AUTHOR_TEXT = "The author rewrote this paragraph after the Proposal opened.";
const EDITED = "Hold the narrator voice in this passage.";

type Opened = Awaited<ReturnType<typeof openedBlockProposal>>;
type Proposal = Awaited<ReturnType<typeof getProposal>>["proposal"];

/** Replaces "Keep" with "Hold" in the candidate of the current Proposal Revision. */
async function editCandidate(started: Started, opened: Opened, sequence: string, ns: string) {
  const { projectId, fetchImpl } = opened.prepared;
  const current = await getProposal({ baseUrl: started.baseUrl, projectId, fetchImpl,
    proposalId: opened.revised.proposal_id });
  const edited = await sendAuthorEdit(started, opened, { kind: "replace_selection", from: 0, to: 4, text: "Hold" },
    { kind: "targeted", proposal: current.proposal }, sequence, ns);
  if (edited.effect.kind !== "proposal_revised") throw new Error(`expected proposal_revised, got ${JSON.stringify(edited.effect)}`);
  const revised = await getProposal({ baseUrl: started.baseUrl, projectId, fetchImpl,
    proposalId: opened.revised.proposal_id });
  assert.equal(revised.proposal.revision_id, edited.effect.proposal_revision_id);
  return { edited: edited.effect, proposal: revised.proposal };
}

/** The validation facts of a Proposal Revision, so that one comparison shows all of them. */
function validationOf(proposal: Proposal) {
  return { base: proposal.base_authoritative_revision_id, candidate: proposal.candidate_text,
    validation: proposal.validation,
    receipt: proposal.validation_receipt.kind === "present" ? proposal.validation_receipt.result : "absent" };
}

/** The opened Proposal with this current Revision and its Validation Receipt. */
function withRevision(opened: Opened, proposal: Proposal): Opened {
  if (proposal.validation_receipt.kind !== "present") throw new Error("expected a Validation Receipt");
  return { ...opened, revised: proposal, validationReceiptId: proposal.validation_receipt.validation_receipt_id };
}

test.each(["edit", "undo"] as const)(
  "a candidate edit on an earlier base is valid, and Acceptance applies after the %s", async (path) => {
    const started = await startRealServer();
    try {
      await drainLeftoverWork();
      const ns = path === "edit" ? "f4a1" : "f4a2";
      const opened = await openedBlockProposal(started, ns);
      const { projectId, chapterId, fetchImpl } = opened.prepared;
      const h0 = opened.base.current_revision.revision_id;
      const block = opened.base.current_revision.blocks[0]!;
      const h1 = id(`${ns}0051`);
      await advanceChapterHead(projectId, chapterId, h0, h1, [{ ...block, text: AUTHOR_TEXT }]);

      const { edited, proposal } = await editCandidate(started, opened, "2", `${ns}006`);
      assert.deepEqual(validationOf(proposal), { base: h0, candidate: EDITED, validation: "valid", receipt: "valid" });
      let target = proposal;
      if (path === "undo") {
        const request: UndoLatestAuthorActionRequest = {
          command_schema: "storyos.command.undo-latest-author-action.request.v1",
          undo_latest_author_action_input: { expected_author_undo_frontier_sequence: edited.author_action_sequence,
            expected_authoritative_revision_id: h1, editor_session_id: opened.session.editor_session.editor_session_id,
            ...BINDING, correlation_id: id(`${ns}0071`) },
        };
        const undone = await challenged(started.baseUrl, fetchImpl, projectId, "POST",
          "/api/v1/projects/{project_id}/author-actions/undo", request.command_schema,
          await digestUndoLatestAuthorAction(request), id(`${ns}0072`), (antiForgery) => undoLatestAuthorAction({
            baseUrl: started.baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}0072`), antiForgery, request }));
        assert.equal(undone.effect.kind, "compensated");
        target = (await getProposal({ baseUrl: started.baseUrl, projectId, fetchImpl,
          proposalId: opened.revised.proposal_id })).proposal;
        assert.equal(target.revision_id, undone.proposal_revision_id);
        assert.deepEqual(validationOf(target), { base: h0, candidate: REVISED, validation: "valid", receipt: "valid" });
      }

      const current = withRevision(opened, target);
      const { accepted } = await accept(started, current, acceptRequest(current, h1, `${ns}0081`), `${ns}0082`);
      if (accepted.effect.kind !== "applied") throw new Error(`expected applied, got ${JSON.stringify(accepted.effect)}`);
      assert.equal(accepted.effect.target_resolution, "block_replacement");
      assert.deepEqual(accepted.effect.authoritative_revision.blocks, [{ ...block, text: target.candidate_text }]);
    } finally {
      await stopRealServer(started.server);
    }
  });

test("a versioned edit of a different Block, then a candidate edit, and Acceptance applies", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await openedBlockProposal(started, "f4a3");
    const { projectId, chapterId } = opened.prepared;
    const h0 = opened.base.current_revision.revision_id;
    const block = opened.base.current_revision.blocks[0]!;
    const other = id("f4a30052");
    await advanceChapterHead(projectId, chapterId, h0, id("f4a30051"),
      [block, { manuscript_block_id: other, text: AUTHOR_TEXT }]);
    const versioned = await sendAuthorEdit(started, opened, { kind: "replace_block_selection", manuscript_block_id: other,
      from: AUTHOR_TEXT.length, to: AUTHOR_TEXT.length, text: " More." }, { kind: "authoritative" }, "2", "f4a3006");
    if (versioned.effect.kind !== "authoritative_applied") {
      throw new Error(`expected authoritative_applied, got ${JSON.stringify(versioned.effect)}`);
    }
    const h2 = versioned.effect.authoritative_revision.revision_id;

    const { proposal } = await editCandidate(started, opened, "3", "f4a3007");
    assert.deepEqual(validationOf(proposal), { base: h0, candidate: EDITED, validation: "valid", receipt: "valid" });
    const current = withRevision(opened, proposal);
    const { accepted } = await accept(started, current, acceptRequest(current, h2, "f4a30081"), "f4a30082");
    if (accepted.effect.kind !== "applied") throw new Error(`expected applied, got ${JSON.stringify(accepted.effect)}`);
    assert.equal(accepted.effect.target_resolution, "block_replacement");
    assert.equal(accepted.effect.authoritative_revision.body, `${EDITED}\n${AUTHOR_TEXT} More.`);
  } finally {
    await stopRealServer(started.server);
  }
});

test("a candidate edit after a recorded Proposal Conflict condition is valid, and Acceptance applies", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await openedBlockProposal(started, "f4a4");
    const { projectId, chapterId } = opened.prepared;
    const h0 = opened.base.current_revision.revision_id;
    const block = opened.base.current_revision.blocks[0]!;
    const h1 = id("f4a40051");
    await advanceChapterHead(projectId, chapterId, h0, h1, [{ ...block, text: AUTHOR_TEXT }]);
    await seedAcceptanceCondition(started.baseUrl, opened.prepared.fetchImpl, projectId, opened.revised,
      opened.session.editor_session.editor_session_id, h1, "proposal_conflict", "f4a4006");

    const { proposal } = await editCandidate(started, opened, "2", "f4a4007");
    assert.deepEqual(validationOf(proposal), { base: h0, candidate: EDITED, validation: "valid", receipt: "valid" });
    const current = withRevision(opened, proposal);
    const { accepted } = await accept(started, current, acceptRequest(current, h1, "f4a40081"), "f4a40082");
    if (accepted.effect.kind !== "applied") throw new Error(`expected applied, got ${JSON.stringify(accepted.effect)}`);
    assert.equal(accepted.effect.target_resolution, "block_replacement");
    assert.equal(accepted.effect.authoritative_revision.body, EDITED);
  } finally {
    await stopRealServer(started.server);
  }
});

test("an invalid Acceptance condition on a candidate-edited Revision still blocks Acceptance, and Reject applies", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await openedBlockProposal(started, "f4a5");
    const { projectId, chapterId, fetchImpl } = opened.prepared;
    const h0 = opened.base.current_revision.revision_id;
    const block = opened.base.current_revision.blocks[0]!;
    const h1 = id("f4a50051");
    await advanceChapterHead(projectId, chapterId, h0, h1, [{ ...block, text: AUTHOR_TEXT }]);
    const { proposal } = await editCandidate(started, opened, "2", "f4a5006");
    const current = withRevision(opened, proposal);

    const earlierReceipt = { ...current, validationReceiptId: opened.validationReceiptId };
    const invalid = await accept(started, current, acceptRequest(earlierReceipt, h1, "f4a50071"), "f4a50072");
    assert.deepEqual(invalid.accepted.effect, { kind: "invalid", reason: "invalid_validation" });
    const { accepted } = await accept(started, current, acceptRequest(current, h1, "f4a50081"), "f4a50082");
    assert.deepEqual(accepted.effect, { kind: "refused", reason: "not_eligible" });
    const rejected = await reject(started, current, h1, "f4a5009");
    assert.equal(rejected.effect.kind, "resolved");
    assert.equal((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl }))
      .chapter.current_revision.body, AUTHOR_TEXT);
  } finally {
    await stopRealServer(started.server);
  }
});

test("a candidate edit after its target Block was deleted is valid, and Acceptance refuses the missing target", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await openedBlockProposal(started, "f4a6");
    const { projectId, chapterId } = opened.prepared;
    const h0 = opened.base.current_revision.revision_id;
    const h1 = id("f4a60051");
    await advanceChapterHead(projectId, chapterId, h0, h1, [{ manuscript_block_id: id("f4a60052"), text: AUTHOR_TEXT }]);
    const { proposal } = await editCandidate(started, opened, "2", "f4a6006");
    assert.deepEqual(validationOf(proposal), { base: h0, candidate: EDITED, validation: "valid", receipt: "valid" });
    const current = withRevision(opened, proposal);
    const { accepted } = await accept(started, current, acceptRequest(current, h1, "f4a60071"), "f4a60072");
    assert.deepEqual(accepted.effect, { kind: "refused", reason: "missing_target" });
  } finally {
    await stopRealServer(started.server);
  }
});
