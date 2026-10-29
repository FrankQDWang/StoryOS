// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/settle-multi-operation-selections-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import { getAgentRun, getChapter, getExportOperation, getProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { zipStoreFiles } from "../support/archive.ts";
import { queryStoryOSPostgres } from "../support/node-integration.ts";
import { USER_A, UUID_V7, assertForeignRefused, createStory, drainLeftoverWork, openProposal, problemOf,
  serverForFile, settleOnce,
} from "../support/http-harness.ts";
import type { CommandInput, TestStory } from "../support/http-harness.ts";

// The Profile Probe of the ActionOnly settlement profile (ADR 0075).
const server = serverForFile();
const WRONG_HEAD = "018f0000-0000-7001-8000-00000000fffe";

/** One open Proposal with one pending Proposal Operation and a writer Editor Session. */
async function openedProposal(title: string) {
  await drainLeftoverWork();
  const story = await createStory(server, { title, assistance: true });
  const chapter = async () => (await getChapter({ baseUrl: story.baseUrl, projectId: story.projectId,
    chapterId: story.chapterIds[0] ?? "", fetchImpl: story.fetchImpl })).chapter;
  const before = await chapter();
  const opened = (await openProposal(story)).proposal;
  const session = await story.openEditorSession();
  const inspect = async () => (await getProposal({ baseUrl: story.baseUrl, projectId: story.projectId,
    proposalId: opened.proposal_id, fetchImpl: story.fetchImpl })).proposal;
  return { story, before, chapter, opened, inspect, editorSessionId: session.editor_session.editor_session_id };
}

type Opened = Awaited<ReturnType<typeof openedProposal>>;

function rejection({ story, opened, before, editorSessionId }: Opened, options: {
  revisionId?: string; targetRevisionId?: string; editorSessionId?: string;
} = {}): CommandInput<"rejectProposalOperations"> {
  return { request: {
    command_schema: "storyos.command.reject-proposal-operations.request.v1",
    reject_proposal_operations_input: {
      proposal_revision_id: options.revisionId ?? opened.revision_id,
      selected_pending_operation_ids: [opened.operation_id],
      expected_target_revisions: [options.targetRevisionId ?? before.current_revision.revision_id],
      rejection_reason: { kind: "author_declined", note: { kind: "omitted" } },
      editor_session_id: options.editorSessionId ?? editorSessionId,
      ...story.binding(),
    },
  }, path: { proposalId: opened.proposal_id } };
}

async function withdrawal({ story, opened, before, editorSessionId }: Opened) {
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

/** Exports the Project archive through the Worker and returns a reader of its canonical tables. */
async function archivedTables(story: TestStory): Promise<(table: string) => unknown[]> {
  const archive = await story.send("exportProjectArchive", { request: {
    command_schema: "storyos.command.export-project-archive.request.v1",
    export_project_archive_input: { ...story.binding(), archive_profile: "storyos.project-export.v1",
      archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" },
  } });
  if (archive.response.effect.kind !== "admitted") throw new Error("expected an admitted archive");
  const exportId = archive.response.effect.export_id;
  await settleOnce();
  assert.equal((await getExportOperation({ baseUrl: story.baseUrl, projectId: story.projectId, exportId,
    fetchImpl: story.fetchImpl })).status, "ready");
  const download = await story.fetchImpl(`${story.baseUrl}/api/v1/projects/${story.projectId}/exports/${exportId}`,
    { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
  assert.equal(download.status, 200);
  const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
  return (table) => JSON.parse(new TextDecoder().decode(files.get(`canonical/${table}.json`))) as unknown[];
}

test("rejectProposalOperations resolves, replays, refuses, and conflicts", async () => {
  const proposal = await openedProposal("Reject Proposal Novel");
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
  const proposal = await openedProposal("Proposal Decision Replay Novel");
  const rejected = await proposal.story.send("rejectProposalOperations", rejection(proposal));
  assert.equal(rejected.response.effect.kind, "resolved");
  const withdrawn = await withdrawal(proposal);
  assert.equal(withdrawn.response.effect.kind, "resolved");
  assert.deepEqual(await rejected.command.send(), { response: rejected.response, body: rejected.body });
  await server.restart();
  assert.deepEqual(await rejected.command.send(), { response: rejected.response, body: rejected.body });
});

test("an exact retry of a reopened withdrawal returns its first acknowledgement after a rejection", async () => {
  const proposal = await openedProposal("Proposal Decision Replay Novel");
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

test("reopenRejectedOperations appends a pending Revision without rewriting Rejection", async () => {
  const proposal = await openedProposal("Reopen Proposal Novel");
  const { story, opened, before, editorSessionId } = proposal;
  const rejected = (await story.send("rejectProposalOperations", rejection(proposal))).response;
  assert.equal(rejected.effect.kind, "resolved");
  if (rejected.effect.kind !== "resolved") throw new Error("expected rejected");
  const resolutionEventId = rejected.effect.resolution_event_refs[0] ?? "";
  const afterReject = await proposal.inspect();
  assert.equal(afterReject.operation_resolution, "rejected");
  assert.equal(afterReject.revision_id, opened.revision_id);
  const reopening = (options: { rejectionEventId?: string } = {}): CommandInput<"reopenRejectedOperations"> => ({
    request: {
      command_schema: "storyos.command.reopen-rejected-operations.request.v1",
      reopen_rejected_operations_input: {
        proposal_revision_id: opened.revision_id,
        selected_rejected_operation_ids: [opened.operation_id],
        rejection_event_refs: [options.rejectionEventId ?? resolutionEventId],
        expected_target_revisions: [before.current_revision.revision_id],
        editor_session_id: editorSessionId,
        ...story.binding(),
      },
    }, path: { proposalId: opened.proposal_id } });
  const unavailableProof = await story.send("reopenRejectedOperations",
    reopening({ rejectionEventId: story.key() }));
  assert.deepEqual(unavailableProof.response.effect, { kind: "refused", reason: "unavailable_proof" });
  assert.equal((await proposal.inspect()).operation_resolution, "rejected");
  const reopenInput = reopening();
  const reopen = await story.challenge("reopenRejectedOperations", reopenInput);
  await queryStoryOSPostgres(`CREATE FUNCTION storyos.test_reopen_failure() RETURNS trigger
    LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'reopen fault'; END $$;
    CREATE TRIGGER test_reopen_failure BEFORE INSERT ON storyos.proposal_operation_reopenings
    FOR EACH ROW EXECUTE FUNCTION storyos.test_reopen_failure();`);
  try {
    assert.equal((await problemOf(reopen.send())).status, 503);
    assert.equal((await proposal.inspect()).operation_resolution, "rejected");
  } finally {
    await queryStoryOSPostgres(`DROP TRIGGER test_reopen_failure ON storyos.proposal_operation_reopenings;
      DROP FUNCTION storyos.test_reopen_failure();`);
  }
  const reopened = (await reopen.send()).response;
  assert.equal(reopened.effect.kind, "resolved");
  assert.equal(reopened.receipt.result, "resolved");
  if (reopened.effect.kind !== "resolved") throw new Error("expected resolved");
  assert.equal(reopened.effect.undo_disposition, "forward");
  assert.equal(reopened.effect.prior_resolution, "rejected");
  assert.equal(reopened.effect.resulting_resolution, "pending");
  assert.equal(reopened.effect.resulting_validation, "pending");
  assert.deepEqual(reopened.effect.operation_ids, [opened.operation_id]);
  assert.deepEqual(reopened.effect.rejection_event_refs, [resolutionEventId]);
  assert.match(reopened.effect.author_action_sequence, /^\d+$/);
  assert.equal(reopened.effect.state_event_refs.length, 1);
  const reopenEventId = reopened.effect.state_event_refs[0] ?? "";
  assert.match(reopenEventId, UUID_V7);
  assert.notEqual(reopened.effect.resulting_proposal_revision_id, opened.revision_id);
  assert.equal(reopened.receipt.source_proposal_revision_id, opened.revision_id);
  assert.equal(reopened.receipt.resulting_proposal_revision_id, reopened.effect.resulting_proposal_revision_id);
  assert.deepEqual(reopened.receipt.authoritative_commit_ids, []);
  assert.deepEqual((await reopen.send()).response, reopened);
  const inspected = await proposal.inspect();
  assert.equal(inspected.operation_resolution, "pending");
  assert.equal(inspected.reservation_state, "unresolved");
  assert.equal(inspected.validation, "pending");
  assert.equal(inspected.revision_id, reopened.effect.resulting_proposal_revision_id);
  assert.deepEqual(inspected.validation_receipt, { kind: "absent" });
  assert.deepEqual(await proposal.chapter(), before);
  const foreign = await problemOf(story.as("session-b").send("reopenRejectedOperations", reopenInput));
  assert.ok(foreign.status === 404 && !foreign.body.includes(USER_A));
  // The request names the source Proposal Revision, which is not the current Revision now.
  const refused = await story.send("reopenRejectedOperations", reopening());
  assert.deepEqual(refused.response.effect, { kind: "refused", reason: "stale_proposal_revision" });

  const scope = { owner_user_id: USER_A, project_id: story.projectId, proposal_id: opened.proposal_id };
  const archived = await archivedTables(story);
  assert.deepEqual(archived("proposal_operation_resolutions"), [{
    ...scope,
    resolution_event_id: resolutionEventId,
    proposal_revision_id: opened.revision_id,
    operation_id: opened.operation_id,
    prior_resolution: "pending",
    resulting_resolution: "rejected",
    rejection_receipt_id: rejected.receipt.receipt_id,
    author_action_sequence: Number(rejected.effect.author_action_sequence),
  }]);
  assert.deepEqual(archived("proposal_operation_reopenings"), [{
    ...scope,
    reopen_event_id: reopenEventId,
    source_proposal_revision_id: opened.revision_id,
    resulting_proposal_revision_id: reopened.effect.resulting_proposal_revision_id,
    operation_id: opened.operation_id,
    rejection_event_id: resolutionEventId,
    prior_resolution: "rejected",
    resulting_resolution: "pending",
    reopen_receipt_id: reopened.receipt.receipt_id,
    author_action_sequence: Number(reopened.effect.author_action_sequence),
    preserved_generation: reopened.effect.preserved_generation,
    preserved_closure: reopened.effect.preserved_closure,
  }]);
  await server.restart();
  const reloaded = await proposal.inspect();
  assert.equal(reloaded.operation_resolution, "pending");
  assert.equal(reloaded.validation, "pending");
  assert.equal(reloaded.revision_id, reopened.effect.resulting_proposal_revision_id);
  assert.deepEqual(await proposal.chapter(), before);
});
