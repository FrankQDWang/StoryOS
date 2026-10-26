import { expect, it } from "vitest";

import { appendAuthorEdit } from "../../src/editor-session.ts";
import { freezeOneIntentSubmission, submitOnePendingAuthorEdit } from "../../src/author-edit-submission.ts";
import { digestJournalValue, readJournalSnapshot, rebuildPendingProjection, validateJournalSnapshot }
  from "../../src/local-edit-journal.ts";
import { digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readLocalRecovery } from "../../src/local-edit-recovery.ts";
import { recoveryMaterial, recoveryPrefix } from "../../src/local-recovery-record.ts";
import { FIRST_APPEND_EDIT, openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import type { EditorWorkspace, JournalIntentRecord, JournalPayloadChain, JournalSubmissionGroup }
  from "../../src/editor-types.ts";

const PROPOSAL_HEAD = "018f0000-0000-7001-8000-000000000092";
const DRAFT = "018f0000-0000-7001-8000-000000000090";
const RECORD = "01a17826-69fb-7dce-9460-9c09d1db0950";
const GROUP = "01a17826-69fb-7080-9a76-5ee58918cf62";
const SCOPE = { owner_user_id: "018f0000-0000-7001-8000-000000000001",
  project_id: "018f0000-0000-7001-8000-000000000002" };
const DIGEST = { algorithm: "sha256", profile: "storyos.command.closeEditorFlowDraft.jcs.v1",
  value_hex_lowercase: "a1fd9ea206bbfa729f6a35ff2d44dabf8311370a816a2eeff0a75ab94356434a" };
// A Draft Discard row and its lost-response observation, as an earlier client stored them (ADR 0073).
const DISCARD_ROWS = [{ key: `discard-observation:${RECORD}:unresolved`, record_key: `discard:${DRAFT}`,
  schema_id: "storyos.local-edit-journal.refused-edit-discard.v1.observation", kind: "unresolved" }, {
  key: `discard:${DRAFT}`, schema_id: "storyos.local-edit-journal.refused-edit-discard.v1",
  explicit_command_record_id: RECORD, local_intent_sequence: 1,
  journal_partition_id: `${SCOPE.owner_user_id}:${SCOPE.project_id}:018f0000-0000-7001-8000-000000000021:1:binding:test:1:`
    + "storyos.web-client.release-1.v3:storyos.web-security-policy.release-1.v1:storyos.foundation.absolute.v1",
  project_scope: SCOPE, editor_session_id: "018f0000-0000-7001-8000-000000000021", writer_generation: "1",
  exact_semantic_payload_ref: GROUP, semantic_payload_digest: DIGEST,
  exact_target_head_anchor_bindings: { draft_id: DRAFT, draft_revision_id: DRAFT, payload_digest: "a".repeat(64),
    expected_closure: "open" },
  editor_contract_revision: "storyos.editor-contract.release-1.v3", command_kind: "closeEditorFlowDraft",
  created_at: "2026-10-26T12:19:05.851Z", author_visible_decision_ref: { draft_id: DRAFT, close_reason: "abandoned" },
  group: { journal_submission_group_id: GROUP, command_kind: "closeEditorFlowDraft",
    command_schema: "storyos.command.close-editor-flow-draft.request.v1",
    digest_profile: "storyos.command.closeEditorFlowDraft.jcs.v1", frozen_request_body_ref: GROUP,
    frozen_request_digest_input_ref: GROUP, frozen_at: "2026-10-26T12:19:05.851Z",
    ordered_coverage: [{ local_intent_sequence: 1, intent_record_ref: RECORD, payload_digest: DIGEST }],
    covered_sequence_range: { first: 1, last: 1 }, frozen_payload_coverage_digest: { algorithm: "sha256",
      profile: "storyos.local-edit-journal.submission-coverage.sha256.v1",
      value_hex_lowercase: "6c89b2e1bb55474d38e699ebb2db5bbb65cbe0364bd9b2ee3fd9d3262453e0ee" },
    action_class: "explicit_editor_command", api_major: 1, method: "POST",
    route_template: "/api/v1/projects/{project_id}/drafts/{draft_id}/closures",
    idempotency_key: "01a17826-69fb-7ca3-94b0-c606166d9b9d",
    frozen_request_body: { command_schema: "storyos.command.close-editor-flow-draft.request.v1",
      close_editor_flow_draft_input: { draft_id: DRAFT, draft_kind: "refused_edit",
        source_current_draft_revision_id: DRAFT, source_draft_payload_digest: "a".repeat(64),
        expected_closure: "open", close_reason: "abandoned", editor_session_id: "018f0000-0000-7001-8000-000000000021",
        writer_generation: "1", client_contract_revision: "storyos.web-client.release-1.v3",
        security_policy_revision: "storyos.web-security-policy.release-1.v1",
        correlation_id: "01a17826-69fb-7b32-a69b-a5c3c97c3be0" } },
    frozen_request_digest: DIGEST } }];

const request = <T,>(value: IDBRequest<T>) => new Promise<T>((resolve, reject) => {
  value.onsuccess = () => resolve(value.result);
  value.onerror = () => reject(value.error);
});

/** Turns the one pending record into the crossing edit that an earlier client stored. */
async function storeEarlierCrossingEdit(workspace: EditorWorkspace) {
  await appendAuthorEdit(workspace, FIRST_APPEND_EDIT);
  const blockId = workspace.session.base_snapshot.materialized_revision.blocks[0]!.manuscript_block_id;
  const unit = { normalized_primitives: [{ kind: "replace_structured_selection" as const,
    replacement: [{ block_kind: "paragraph" as const, text: "Lost" }] }],
  selection_snapshot: { coordinate_profile: "storyos.editor.ordered-source.v1", from: 0, to: 3,
    ordered_selection: { sources: [{ owner: { kind: "manuscript" as const, manuscript_block_id: blockId },
      coordinate_profile: "prosemirror-token-utf16.v1", from: 0, to: 4, block_kind: "paragraph" as const,
      source_text: "Base" }, { owner: { kind: "proposal" as const, proposal_id: "018f0000-0000-7001-8000-000000000093",
      operation_id: "018f0000-0000-7001-8000-000000000094", revision_id: PROPOSAL_HEAD,
      manuscript_block_id: blockId }, coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 3,
    block_kind: "paragraph" as const, source_text: "Old" }],
    anchor: { source_index: 0, source_offset: 0 }, head: { source_index: 1, source_offset: 3 } } } };
  const [payloadDigest, bodyDigest] = await Promise.all([digestJournalValue(unit, crypto),
    digestJournalValue("Base", crypto)]);
  const transaction = workspace.database.transaction(["intents", "payload_chains", "metadata"], "readwrite");
  const [record] = await request(transaction.objectStore("intents").getAll()) as JournalIntentRecord[];
  const [chain] = await request(transaction.objectStore("payload_chains").getAll()) as JournalPayloadChain[];
  transaction.objectStore("intents").put({ ...record!, author_edit_unit: unit, payload_digest: payloadDigest,
    expected_proposal_heads: [PROPOSAL_HEAD], observed_ownership_partition: "mixed" });
  transaction.objectStore("payload_chains").put({ ...chain!, ordered_patch_refs: [{ ...chain!.ordered_patch_refs[0]!,
    normalized_primitives: unit.normalized_primitives, resulting_payload_digest: bodyDigest }] });
  for (const row of DISCARD_ROWS) transaction.objectStore("metadata").put(row);
  await new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error);
  });
}

it("never sends a stored crossing edit or Draft Discard of an earlier client and keeps the Chapter saved", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    await storeEarlierCrossingEdit(test.workspace);
    const workspace = await test.reopen();
    const requests: string[] = [];
    const fetchImpl: typeof fetch = async (input) => {
      requests.push(String(input));
      throw new Error(`Unexpected request ${String(input)}`);
    };

    expect(workspace.openedProjection).toMatchObject({ body: "Base", save_state: "saved", unsettled_intent_count: 0 });
    expect(await rebuildPendingProjection(workspace))
      .toMatchObject({ body: "Base", save_state: "saved", unsettled_intent_count: 0 });
    await expect(submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin, fetchImpl }))
      .rejects.toThrow("One pending Author Edit is required");
    expect(requests).toEqual([]);

    // The next edit of the author forms its own group without the stored crossing edit.
    await appendAuthorEdit(workspace, { ...FIRST_APPEND_EDIT, createdAt: new Date().toISOString() });
    expect(await rebuildPendingProjection(workspace))
      .toMatchObject({ body: "Base!", save_state: "saving", unsettled_intent_count: 1 });
    const group = await freezeOneIntentSubmission(workspace);
    expect(group.frozen_request_body.author_edit_units.map((unit) => unit.normalized_primitives[0]?.kind))
      .toEqual(["replace_block_selection"]);
    expect(group.covered_sequence_range).toEqual({ first: 2, last: 2 });
    expect((await readJournalSnapshot(workspace)).records).toHaveLength(2);
  } finally { await test.close(); }
});

it("starts a new payload chain after a stored Draft retry of an earlier client, so a reload replays only the new edit", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    // The stored Draft retry applied "!" in its payload chain. The Server never applied it.
    await appendAuthorEdit(test.workspace, FIRST_APPEND_EDIT);
    const transaction = test.workspace.database.transaction("intents", "readwrite");
    const [record] = await request(transaction.objectStore("intents").getAll()) as JournalIntentRecord[];
    transaction.objectStore("intents").put({ ...record!, retry_source: { kind: "draft_retry",
      source_draft_kind: "refused_edit", source_draft_id: DRAFT, source_current_draft_revision_id: DRAFT,
      source_draft_payload_digest: "a".repeat(64), expected_source_draft_closure: "open",
      selected_payload_range: { kind: "exact_structured_range", coordinate_profile: "storyos.draft-replacement.block-utf16.v1",
        from: { block_index: 0, offset: 0 }, to: { block_index: 0, offset: 1 }, slice_digest: "b".repeat(64) } } });
    await new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onabort = () => reject(transaction.error);
    });
    const workspace = await test.reopen();
    expect(workspace.openedProjection).toMatchObject({ body: "Base", save_state: "saved", unsettled_intent_count: 0 });

    await appendAuthorEdit(workspace, { ...FIRST_APPEND_EDIT, createdAt: new Date().toISOString() });
    const reopened = await test.reopen();
    expect(reopened.openedProjection).toMatchObject({ body: "Base!", save_state: "saving", unsettled_intent_count: 1 });
    expect((await freezeOneIntentSubmission(reopened)).covered_sequence_range).toEqual({ first: 2, last: 2 });
  } finally { await test.close(); }
});

it("shows no local recovery text for a stored Draft retry group of an earlier client", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    await appendAuthorEdit(test.workspace, FIRST_APPEND_EDIT);
    const frozen = await freezeOneIntentSubmission(test.workspace);
    const material = recoveryMaterial(await validateJournalSnapshot(test.workspace,
      await readJournalSnapshot(test.workspace)), frozen);
    const retrySource = { kind: "draft_retry", source_draft_kind: "refused_edit", source_draft_id: DRAFT,
      source_current_draft_revision_id: DRAFT, source_draft_payload_digest: "a".repeat(64),
      expected_source_draft_closure: "open", selected_payload_range: { kind: "exact_structured_range",
        coordinate_profile: "storyos.draft-replacement.block-utf16.v1", from: { block_index: 0, offset: 0 },
        to: { block_index: 0, offset: 1 }, slice_digest: "b".repeat(64) } };
    const body = { ...frozen.frozen_request_body,
      retry_source: retrySource } as JournalSubmissionGroup["frozen_request_body"];
    const group = { ...frozen, frozen_request_body: body,
      frozen_request_digest: await digestApplyAuthorEdit(body, crypto),
      settlement: { kind: "outcome_query_requires_reconfirmation", command_id: "018f0000-0000-7001-8000-000000000031",
        author_command_admission_id: "018f0000-0000-7001-8000-000000000032",
        reconfirmation_reason: "direct_edit_intent_unrecoverable", recovery_draft_ref: null } } as JournalSubmissionGroup;
    const transaction = test.workspace.database.transaction(["intents", "submission_groups", "metadata"], "readwrite");
    const [record] = await request(transaction.objectStore("intents").getAll()) as JournalIntentRecord[];
    transaction.objectStore("intents").put({ ...record!, retry_source: retrySource });
    transaction.objectStore("submission_groups").put(group);
    // Another Editor Session of the Project kept the same group for manual re-entry.
    transaction.objectStore("metadata").put({ key: `${recoveryPrefix}earlier-session:${group.journal_submission_group_id}`,
      value: { ...material, group, disposition: "retained_for_manual_reentry" } });
    await new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onabort = () => reject(transaction.error);
    });
    const workspace = await test.reopen();

    expect(workspace.openedProjection).toMatchObject({ body: "Base", save_state: "saved", unsettled_intent_count: 0 });
    expect(await readLocalRecovery(workspace)).toEqual([]);
  } finally { await test.close(); }
});
