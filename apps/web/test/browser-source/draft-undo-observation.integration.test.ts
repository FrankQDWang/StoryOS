import { afterEach, expect, it } from "vitest";
import closedFixture from "../../../../generated/golden-wire/storyos-public-release-1/close-editor-flow-draft.json";
import reopenedFixture from "../../../../generated/golden-wire/storyos-public-release-1/editor-flow-draft-reopened.json";
import undoFixture from "../../../../generated/golden-wire/storyos-public-release-1/undo-latest-author-action.json";
import type { EditorFlowDraftClosed, EditorFlowDraftReopened, UndoLatestAuthorActionResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { freezeDraftUndo, observeDraftUndo, readDraftUndoJournal } from "../../src/draft-undo-journal.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";

const HEAD = "018f0000-0000-7001-8000-000000000e01";
const PROPOSAL_REVISION = "018f0000-0000-7001-8000-000000000e02";
const REVISION = "018f0000-0000-7001-8000-000000000e03";
const COMMIT = "018f0000-0000-7001-8000-000000000e04";
let close: (() => Promise<void>) | undefined;
afterEach(async () => { await close?.(); close = undefined; });

// The Undo of an Author Edit that superseded a Refused Edit Draft, with the reopened Draft and the response.
async function coupledUndo(compensation: "proposal" | "prose") {
  const test = await openJournalAppendTestWorkspace();
  close = test.close;
  const scope = test.workspace.partition.project_scope;
  const sourceClose = structuredClone(closedFixture.effect.event) as EditorFlowDraftClosed;
  Object.assign(sourceClose, { project_scope: scope, close_reason: "superseded" });
  sourceClose.source.command_digest.profile = "storyos.command.applyAuthorEdit.jcs.v1";
  const closedSequence = sourceClose.author_action_sequence!;
  const record = await freezeDraftUndo(test.workspace, sourceClose, {
    command_schema: "storyos.command.undo-latest-author-action.request.v1",
    undo_latest_author_action_input: {
      expected_author_undo_frontier_sequence: closedSequence,
      expected_authoritative_revision_id: HEAD,
      editor_session_id: test.workspace.partition.editor_session_id,
      client_contract_revision: test.workspace.partition.client_contract_revision,
      security_policy_revision: test.workspace.partition.security_policy_revision,
      correlation_id: "018f0000-0000-7001-8000-000000000e05",
    },
  }, "018f0000-0000-7001-8000-000000000e06", () => true);
  const source = { command_id: undoFixture.command_id, author_command_admission_id: undoFixture.author_command_admission_id,
    receipt_id: undoFixture.receipt.receipt_id, idempotency_key: record.group.idempotency_key,
    command_digest: record.group.frozen_request_digest };
  const reopened = structuredClone(reopenedFixture) as EditorFlowDraftReopened;
  Object.assign(reopened, { project_scope: scope, draft_id: sourceClose.draft_id, draft_revision_id: sourceClose.draft_revision_id,
    payload_digest: sourceClose.payload_digest, source_close_event_id: sourceClose.event_id,
    source_author_action_sequence: sourceClose.author_action_sequence, author_action_sequence: "4",
    created_at: undoFixture.receipt.created_at, source });
  Object.assign(reopened.handler_receipt, { project_scope: scope, author_undo_receipt_id: source.receipt_id,
    source_close_event_id: sourceClose.event_id, event_id: reopened.event_id, created_at: reopened.created_at });
  const authority = compensation === "prose"
    ? { resulting_heads: [REVISION], authoritative_revision_ids: [REVISION], authoritative_commit_ids: [COMMIT] }
    : { resulting_heads: [HEAD], authoritative_revision_ids: [], authoritative_commit_ids: [] };
  const response = {
    ...structuredClone(undoFixture),
    correlation_id: record.group.frozen_request_body.undo_latest_author_action_input.correlation_id,
    project_scope: scope,
    project: { ...undoFixture.project, project_id: scope.project_id },
    source_reopen_event: reopened,
    ...compensation === "proposal" ? { proposal_revision_id: PROPOSAL_REVISION } : {},
    receipt: { ...undoFixture.receipt, project_scope: scope,
      command_digest: record.group.frozen_request_digest, idempotency_key: record.group.idempotency_key,
      expected_heads: [HEAD], prior_heads: [HEAD], ...authority, author_action_sequence: "4",
      draft_artifact_refs: [sourceClose.draft_id], artifact_lifecycle_event_refs: [reopened.event_id] },
    effect: { kind: "compensated", source_sequence: closedSequence, author_action_sequence: "4",
      project_activity_position: "6", author_undo_frontier_sequence: "2",
      ...compensation === "prose" ? { authoritative_commit_id: COMMIT,
        authoritative_revision: { revision_id: REVISION, body: "Base", blocks: [] } } : {} },
  } as UndoLatestAuthorActionResponse;
  return { workspace: test.workspace, record, response };
}

it("a Proposal Compensation effect that omits the Authoritative Revision and Commit is valid Undo evidence", async () => {
  const { workspace, record, response } = await coupledUndo("proposal");
  await observeDraftUndo(workspace, record, { response });
  expect((await readDraftUndoJournal(workspace)).map(({ observation }) => observation)).toEqual([
    { key: `draft-undo-observation:${record.source_close.event_id}`, record_key: record.key, response }]);
});

it("a Proposal Compensation effect with an empty projection of the unchanged head is not Undo evidence", async () => {
  const { workspace, record, response } = await coupledUndo("proposal");
  const projected = { ...response, effect: { ...response.effect, authoritative_commit_id: "",
    authoritative_revision: { revision_id: HEAD, body: "", blocks: [] } } };
  await expect(observeDraftUndo(workspace, record, { response: projected })).rejects.toThrow("Undo evidence unavailable");
});

it("a prose Compensation effect requires the Authoritative Revision and Commit", async () => {
  const { workspace, record, response } = await coupledUndo("prose");
  if (response.effect.kind !== "compensated") throw new Error("expected a Compensation");
  const { authoritative_revision: _revision, ...withoutRevision } = response.effect;
  const { authoritative_commit_id: _commit, ...withoutCommit } = response.effect;
  for (const effect of [withoutRevision, withoutCommit]) {
    await expect(observeDraftUndo(workspace, record, { response: { ...response, effect } }))
      .rejects.toThrow("Undo evidence unavailable");
  }
  await observeDraftUndo(workspace, record, { response });
});
