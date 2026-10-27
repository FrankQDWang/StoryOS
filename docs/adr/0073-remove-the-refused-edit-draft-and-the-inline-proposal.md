---
status: accepted
---

# Remove the Refused Edit Draft and the Inline Proposal

On 2026-10-26, the author accepted the plan of [Remove the Refused Edit Draft, its expansion, and the inline Proposal path](https://github.com/FrankQDWang/StoryOS/issues/1273). Three tickets deliver it in this order: the Web ([#1276](https://github.com/FrankQDWang/StoryOS/issues/1276)), the Server and the Draft contract ([#1277](https://github.com/FrankQDWang/StoryOS/issues/1277)), and the `inline_edit` Proposal kind ([#1278](https://github.com/FrankQDWang/StoryOS/issues/1278)).

## Context

Before [ADR 0072](0072-replace-only-the-first-owner-part-of-a-crossing-author-edit.md), an author edit over a selection that crossed the Chapter text and a Proposal candidate was one `mixed` Author Edit. The Server refused it and created a Refused Edit Draft. The author can retry, discard, or expand that Draft into a Proposal, and Author Undo can reverse each of these Draft commands. After ADR 0072, the Web editor sends no such edit, so no Web input creates a Draft.

An Inline Proposal (`inline_edit`) holds a Proposal Anchor in one Chapter block. It adds anchor proofs to Acceptance, Replan, Author Edit, and Author Undo. A Block Proposal does the same work for the author with no anchor.

No production deployment exists ([ADR 0022](0022-prefer-widely-validated-hosted-infrastructure.md)). The local development databases hold no Draft row and no Proposal row.

## Decision

- **Refused Edit Draft.** Release 1 has no Refused Edit Draft. No command creates, retries, closes, expands, or undoes one. The commands `closeEditorFlowDraft` and `expandRefusedEditDraftToProposal`, the `applyAuthorEdit` effect `refused_to_draft`, the field `retry_source`, the primitive `replace_structured_selection`, and the Author Undo effect `draft_compensated` go.
- **Inline Proposal.** Release 1 has no Inline Proposal. The Proposal kinds are `block_edit` and `reversal`. The Proposal Anchor, the Acceptance target resolutions `proven_range` and `base_block_replacement`, and the inline rebind Revision go.
- **Web.** The Web shows no Refused Edit Draft and no Inline Proposal, and it sends no Draft command. It shows a stored `inline_edit` Proposal nowhere: no candidate, no control, and no status.
- **Local Edit Journal read rule.** A stored record of a removed path counts as resolved, and the Journal never sends it. These are a record with a `replace_structured_selection` primitive or a `draft_retry` source, and a Submission Group of such a record. Also included are a Submission Group settled with `refused_to_draft`, and the Draft Discard, expansion, and Draft Undo metadata rows. The Journal drops the replacement text of such a record. The Server never applied that text. The editor contract revision does not change, so each other stored record stays readable.
- **Old client.** After the Server ticket, a request with a removed field or primitive does not match the closed request schema. The Server answers HTTP 422 `invalid_request_shape` before admission and writes nothing.
- **Stored rows.** The Server tickets add guard constraints to the dormant Draft and Anchor tables and to `domain_receipts`. A stored row of a removed path stops the migration. The migrations delete no data, and the tables stay in the Project archive format.

## Superseded parts

This record supersedes these parts of earlier records:

- [ADR 0003](0003-specify-manuscript-revision-and-proposal-state-machine.md): the Refused Edit Draft and the Inline Proposal parts of the state machine contract.
- [ADR 0043](0043-settle-every-project-command-through-declared-profiles-of-one-sequence.md): the Draft command rules, the expansion target read, and the Draft replay rules.
- [ADR 0044](0044-settle-author-edit-acceptance-and-author-undo-through-the-command-sequence.md): the disposition rows of `closeEditorFlowDraft` and `expandRefusedEditDraftToProposal`. Also the reopen of a superseded Refused Edit Draft and the applied edit of an Inline Proposal in the `applyAuthorEdit` rows.
- [ADR 0063](0063-bind-each-proposal-anchor-set-to-a-proposal-revision.md): the Proposal Anchor rules. Its open Proposal list rules stay.
- [ADR 0064](0064-accept-a-proposal-on-an-earlier-base-by-replacing-its-target-text.md): the target resolutions `proven_range` and `base_block_replacement`.
- [ADR 0068](0068-prove-the-target-of-a-proposal-author-undo-through-the-compensation-lineage-of-the-proposal-head.md): the expansion target and the inline rebind lineage.
- [ADR 0072](0072-replace-only-the-first-owner-part-of-a-crossing-author-edit.md): the consequences for the remaining Draft path and the Chapter edit next to an Inline Proposal.

## Considered options

- Keep the Draft and show a reason for its refusal. Rejected: after ADR 0072 no Web input creates a Draft, so the path only adds proofs, commands, and Undo kinds.
- Drop the dormant tables in the migrations. Rejected: the Draft families have triggers on shared tables, so a drop must emit those trigger functions again. That is a large risk for no product effect.
- Show the dropped Journal text in the local unsent text panel. Rejected: the Server never applied that text, and no stored record of it exists outside a disposable test run.
- Change the editor contract revision. Rejected: each stored Journal record then becomes unreadable.

## Consequences

- The Web ticket removes the Web Draft and inline code and the packaged inline journey. The Server ticket removes the Draft contract and Server code. The last ticket removes the `inline_edit` kind.
- The Server ticket removes the three Draft routes, the Draft events, and the Draft shapes of the contract. Migration `0099_remove_refused_edit_draft.sql` adds `CHECK (false)` to `draft_artifacts` and refuses the Draft command and result kinds in `domain_receipts`. The Domain Receipt keeps `draft_artifact_refs` and `artifact_lifecycle_event_refs`, and they are always empty. Stored Undo conflict Receipts can still name `source_binding_changed`, so that reason stays.
- The Web keeps local types for the removed stored shapes. Thus the Local Edit Journal read rule still reads and resolves an old record.
- The primary Stage 4 evidence of `S4-REQ-005` moves from the removed inline journey to the production host journey.
- The last ticket removes the inline Proposal path from the Server and the contract. Migration `0100_remove_inline_proposal.sql` limits the Proposal kind to `block_edit` and `reversal` and adds `CHECK (false)` to `proposal_anchors`. The dormant `proposal_anchors` table and its functions and triggers stay, and the Project archive format does not change. A stored Receipt with the resolution `proven_range` or `base_block_replacement` does not decode, so its replay gives `unavailable`.
- The `applyAuthorEdit` rule for a Chapter edit next to a pending Proposal head stays. It applies to each pending head, so it also applies to a stored `inline_edit` head.
- The primary Stage 3 evidence of `S3-EVD-003` moves from the removed inline Proposal HTTP test to `open-block-proposal-http.integration.test.ts`. That test examines the target identity, validation, refusal, the explicit decision, and the non-destructive state. `replan-proposal-http.integration.test.ts` now supports it for the conflict, the Replan, and the restart.
- The HTTP evidence of answer A-012 of [ADR 0066](0066-record-the-answers-to-the-reference-model-contract-questions.md) moves to `edit-proposal-candidate-http.integration.test.ts`. An unknown Operation in the target of a Block Proposal edit gives `Refused` with `target_mismatch`. The [ADR 0068](0068-prove-the-target-of-a-proposal-author-undo-through-the-compensation-lineage-of-the-proposal-head.md) chain through a replacing Inline Acceptance has no test, because no such Acceptance exists.
- Both modes of the recovery drill keep two Acceptance conditions, because no Inline Replan condition exists.
- [Keep the inline rebind lineage](https://github.com/FrankQDWang/StoryOS/issues/1257) has no effect after the last ticket.
