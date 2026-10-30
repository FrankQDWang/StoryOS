// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/settle-multi-operation-selections-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import { getAgentRun, getChapter, getProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, UUID_V7, archivedTables, assertForeignRefused, createStory, drainLeftoverWork, openProposal,
  openedProposal, problemOf, rejection, serverForFile, settleOnce,
} from "../support/http-harness.ts";
import type { CommandInput, OpenedProposal } from "../support/http-harness.ts";

// The Profile Probe of the ActionOnly settlement profile (ADR 0075).
const server = serverForFile();
const WRONG_HEAD = "018f0000-0000-7001-8000-00000000fffe";

async function withdrawal({ story, opened, before, editorSessionId }: OpenedProposal) {
  return story.send("withdrawProposal", { request: {
    command_schema: "storyos.command.withdraw-proposal.request.v1",
    withdraw_proposal_input: {
      cause: "author",
      proposal_revision_id: opened.revision_id,
      expected_closure: "open",
      expected_target_revisions: [before.current_revision.revision_id],
      withdrawal_reason: { kind: "author_withdrew", note: { kind: "omitted" } },
      editor_session_id: editorSessionId,
      ...story.binding(),
    },
  }, path: { proposalId: opened.proposal_id } });
}

test("rejectProposalOperations resolves, replays, refuses, and conflicts", async () => {
  const proposal = await openedProposal(server, "Reject Proposal Novel");
  const { story, opened, before } = proposal;
  assert.equal(opened.operation_resolution, "pending");
  const discussed = await story.send("createAgentRun", { request: {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation: { kind: "new" },
      author_message: { text: "The candidate feels too formal." },
      working_target: { kind: "current_chapter", chapter_id: story.chapterIds[0] ?? "" },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...story.binding(),
    },
  } });
  if (discussed.response.effect.kind !== "admitted") throw new Error("expected an admitted discussion");
  await settleOnce();
  const feedback = await getAgentRun({ baseUrl: story.baseUrl, projectId: story.projectId,
    runId: discussed.response.effect.run_id, fetchImpl: story.fetchImpl });
  assert.equal(feedback.decision.kind, "advisory");
  assert.equal((await proposal.inspect()).operation_resolution, "pending");
  const conflicted = await story.send("rejectProposalOperations", rejection(proposal, { targetRevisionId: WRONG_HEAD }));
  assert.deepEqual(conflicted.response.effect, { kind: "conflicted", reason: "changed_head" });

  const rejected = await story.send("rejectProposalOperations", rejection(proposal));
  const { effect, receipt } = rejected.response;
  assert.equal(receipt.result, "resolved");
  assert.deepEqual(receipt.authoritative_commit_ids, []);
  if (effect.kind !== "resolved") throw new Error("expected resolved");
  assert.match(effect.author_action_sequence, /^\d+$/);
  assert.equal(effect.resolution_event_refs.length, 1);
  assert.match(effect.resolution_event_refs[0] ?? "", UUID_V7);
  assert.deepEqual(
    [effect.undo_disposition, effect.prior_resolution, effect.resulting_resolution, effect.operation_ids,
      effect.rejection_reason.kind],
    ["forward", "pending", "rejected", [opened.operation_id], "author_declined"],
  );
  assert.deepEqual(await rejected.command.send(), { response: rejected.response, body: rejected.body });
  const inspected = await proposal.inspect();
  assert.deepEqual([inspected.operation_resolution, inspected.reservation_state, inspected.revision_id],
    ["rejected", "resolved", opened.revision_id]);
  assert.deepEqual(await proposal.chapter(), before);

  const changed = rejection(proposal);
  const { status, code } = await problemOf(rejected.command.send({ request: changed.request }));
  assert.deepEqual({ status, code }, { status: 422, code: "challenge_invalid" });
  await assertForeignRefused(story, "rejectProposalOperations", rejection(proposal));
  const wrongAdmission = await problemOf(story.send("rejectProposalOperations",
    rejection(proposal, { editorSessionId: story.key() })));
  assert.equal(wrongAdmission.status, 422);
  const again = await story.send("rejectProposalOperations", rejection(proposal));
  assert.deepEqual(again.response.effect, { kind: "refused", reason: "operation_not_pending" });

  const preserved = { preserved_generation: effect.preserved_generation, preserved_validation: effect.preserved_validation,
    preserved_closure: effect.preserved_closure };
  const scope = { owner_user_id: USER_A, project_id: story.projectId, proposal_id: opened.proposal_id,
    proposal_revision_id: opened.revision_id };
  const archived = await archivedTables(story);
  assert.deepEqual(archived("proposal_rejection_receipts").find((row) =>
    (row as { result: string }).result === "proposal_operations_resolved"), { ...scope,
    rejection_receipt_id: receipt.receipt_id, selected_operation_id: opened.operation_id,
    result: "proposal_operations_resolved", rejection_reason: "author_declined", rejection_note: null, ...preserved });
  assert.deepEqual(archived("proposal_operation_resolutions"), [{ ...scope,
    resolution_event_id: effect.resolution_event_refs[0], operation_id: opened.operation_id,
    prior_resolution: "pending", resulting_resolution: "rejected", rejection_receipt_id: receipt.receipt_id,
    author_action_sequence: Number(effect.author_action_sequence) }]);
});

test("an exact retry of a rejection returns its first acknowledgement after a withdrawal and a Server restart", async () => {
  const proposal = await openedProposal(server, "Proposal Decision Replay Novel");
  const rejected = await proposal.story.send("rejectProposalOperations", rejection(proposal));
  assert.equal(rejected.response.effect.kind, "resolved");
  const withdrawn = await withdrawal(proposal);
  assert.equal(withdrawn.response.effect.kind, "resolved");
  assert.deepEqual(await rejected.command.send(), { response: rejected.response, body: rejected.body });
  await server.restart();
  assert.deepEqual(await rejected.command.send(), { response: rejected.response, body: rejected.body });
});

test("an exact retry of a reopened withdrawal returns its first acknowledgement after a rejection", async () => {
  const proposal = await openedProposal(server, "Proposal Decision Replay Novel");
  const { story, opened, before, editorSessionId } = proposal;
  const withdrawn = await withdrawal(proposal);
  if (withdrawn.response.effect.kind !== "resolved") throw new Error("expected an author withdrawal");
  const reopened = await story.send("reopenWithdrawnProposal", { request: {
    command_schema: "storyos.command.reopen-withdrawn-proposal.request.v1",
    reopen_withdrawn_proposal_input: {
      proposal_revision_id: opened.revision_id,
      withdrawal_event_ref: withdrawn.response.effect.closure_event_refs[0] ?? "",
      expected_closure: "withdrawn",
      expected_target_revisions: [before.current_revision.revision_id],
      editor_session_id: editorSessionId,
      ...story.binding(),
    },
  }, path: { proposalId: opened.proposal_id } });
  if (reopened.response.effect.kind !== "resolved") throw new Error("expected a resolved reopen");
  assert.equal(reopened.response.effect.preserved_operation_resolution, "pending");
  const rejected = await story.send("rejectProposalOperations",
    rejection(proposal, { revisionId: reopened.response.effect.resulting_proposal_revision_id }));
  assert.equal(rejected.response.effect.kind, "resolved");
  assert.deepEqual(await reopened.command.send(), { response: reopened.response, body: reopened.body });
});

test("author withdraw closes the current Proposal without changing authority", async () => {
  await drainLeftoverWork();
  const story = await createStory(server, { title: "Withdraw Proposal Novel", assistance: true });
  const chapter = async () => (await getChapter({ baseUrl: story.baseUrl, projectId: story.projectId,
    chapterId: story.chapterIds[0] ?? "", fetchImpl: story.fetchImpl })).chapter;
  const inspect = async (proposalId: string) => (await getProposal({ baseUrl: story.baseUrl,
    projectId: story.projectId, proposalId, fetchImpl: story.fetchImpl })).proposal;
  const before = await chapter();
  // openProposal refuses a decision that is not an opened prose change.
  const opened = (await openProposal(story)).proposal;
  assert.equal(opened.closure, "open");
  const stopped = await story.send("updateProjectAssistance", { request: {
    command_schema: "storyos.command.update-project-assistance.request.v1",
    update_project_assistance_input: { availability: "unavailable", expected_assistance_revision: "1",
      ...story.binding() },
  } });
  assert.equal(stopped.response.effect.kind, "authoritative_applied");
  if (stopped.response.effect.kind === "authoritative_applied") {
    assert.equal(stopped.response.effect.availability, "unavailable");
  }
  const afterStop = await inspect(opened.proposal_id);
  assert.equal(afterStop.closure, "open");
  assert.equal(afterStop.operation_resolution, "pending");
  const session = await story.openEditorSession();
  const withdrawRequest = (): CommandInput<"withdrawProposal"> => ({ request: {
    command_schema: "storyos.command.withdraw-proposal.request.v1",
    withdraw_proposal_input: {
      cause: "author",
      proposal_revision_id: opened.revision_id,
      expected_closure: "open",
      expected_target_revisions: [before.current_revision.revision_id],
      withdrawal_reason: { kind: "author_withdrew", note: { kind: "omitted" } },
      editor_session_id: session.editor_session.editor_session_id,
      ...story.binding(),
    },
  }, path: { proposalId: opened.proposal_id } });
  const withdrawInput = withdrawRequest();
  const withdrawn = (await story.send("withdrawProposal", withdrawInput)).response;
  assert.equal(withdrawn.effect.kind, "resolved");
  if (withdrawn.effect.kind !== "resolved") throw new Error("expected resolved withdrawal");
  assert.equal(withdrawn.effect.prior_closure, "open");
  assert.equal(withdrawn.effect.resulting_closure, "withdrawn");
  assert.deepEqual(withdrawn.receipt.authoritative_commit_ids, []);
  const inspected = await inspect(opened.proposal_id);
  assert.equal(inspected.closure, "withdrawn");
  assert.equal(inspected.operation_resolution, "pending");
  assert.equal(inspected.revision_id, opened.revision_id);
  assert.deepEqual(await chapter(), before);
  const foreign = await problemOf(story.as("session-b").send("withdrawProposal", withdrawInput));
  assert.ok(foreign.status === 404 && !foreign.body.includes(USER_A));
  const again = (await story.send("withdrawProposal", withdrawRequest())).response;
  assert.deepEqual(again.effect, { kind: "no_effect", reason: "closure_not_open" });
  assert.equal(again.receipt.authoritative_commit_ids.length, 0);
});
