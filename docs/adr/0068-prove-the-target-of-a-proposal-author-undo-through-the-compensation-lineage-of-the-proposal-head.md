---
status: accepted
---

# Prove the Target of a Proposal Author Undo Through the Compensation Lineage of the Proposal Head

The ticket [Let Undo Go Back Many Steps Through Proposal Candidate Edits and Withdrawal](https://github.com/FrankQDWang/StoryOS/issues/1249) owns this ADR.

## Context

The author makes candidate edit E1 and then candidate edit E2 on one Proposal. Author Undo compensates E2. The Compensation appends a new Proposal Revision to the Proposal head. E1 is now the Author Undo Frontier.

Two Compensations found their source only when the Proposal head was the Revision of the source action. They are the Compensation of a candidate edit and of an author withdrawal. ADR 0044 recorded this rule: "a Proposal head that moved after the Forward action makes the Forward action a Barrier." Thus the Undo of E1 gave `barrier`.

On 2026-10-21 the author decided that all Author Undo of a Proposal action goes back many steps, as in an ordinary editor. This is the same rule as prose Undo ([ADR 0067](0067-prove-the-target-of-author-undo-through-the-compensation-lineage-of-the-chapter-head.md)). This decision covers candidate edits and author withdrawals. [#1244](https://github.com/FrankQDWang/StoryOS/issues/1244) covers the three Proposal decisions.

## Decision

- **Proposal head lineage.** The lineage Revision of a Proposal head is the Revision that the head restores. Start at the current head. A recorded Proposal Compensation can have appended that Revision. Then go to the prior Revision of the compensated Forward action, and continue. When no recorded Proposal Compensation explains the Revision, the walk stops there.
- **Evidence.** The walk uses only existing records. The Undo Receipt payload of a Proposal Compensation records the appended Revision (`proposal_revision_id`) and its parent (`source_proposal_revision_id`). A step is valid only when the parent of the Revision is the recorded parent. The Compensation Author Action names the compensated Forward action. The walk does not compare text.
- **Prior Revision.** For a candidate edit, a Replan, and a Reopen, the prior Revision is the parent of the Revision that the Forward action wrote. For an author withdrawal, the prior Revision is the withdrawn Revision itself, because the withdrawal changes the closure in place.
- **Candidate edit target.** The Undo of a candidate edit compensates when the lineage Revision of the current head is the Revision that the edit wrote. It restores the candidate of the edited Operation before the edit.
- **Withdrawal target.** The Undo of an author withdrawal compensates when two conditions are true. The current head is withdrawn. The lineage Revision of the current head is the withdrawn Revision. The Compensation reopens the Proposal.
- **Compensation records.** The Compensation appends its Revision to the current head, as the Chapter rule of ADR 0067 does. Thus the Proposal history stays linear. The Undo Receipt payload records the appended Revision and the current head as its parent. The replay of an exact retry finds the appended Revision through these two ids.
- **Barrier.** These conditions still stop the chain with the reason `barrier`. No new reason code is added.
  - The Frontier has the Barrier disposition of ADR 0044, for example `rejectProposalOperations`, `completeReadyPartialProposal`, `continueProposalGeneration`, or `undoLatestAuthorAction` with `reversal_required`.
  - A writer that the records cannot explain moved the Proposal head. Examples are an append of Worker generation, the reopen Revision of an Acceptance Compensation, and an old Compensation with no Receipt payload.
  - The source is a candidate edit from before migration 0085 on a Block that is not the primary Block. Its prior candidate is not known.
  - The source is an author withdrawal, and the current head is not withdrawn.
- **Editor.** The Web Client does not change. After each Undo, it reads the next Author Undo Frontier from the Editor Session. The Server accepts that Undo or gives its reason. An unavailable Undo keeps the editor writable (ADR 0067).
- **Storage.** No new column and no new table. Migration 0096 replaces one trigger function. The Receipt trigger of migration 0066 required that the parent of the appended Revision is the edited Revision. Migration 0096 replaces only the Proposal edit Compensation branch of `storyos.require_draft_retry_settlement()`. All other branches stay as migration 0066 wrote them.
- **Trigger branch.** The new branch requires four conditions. The appended Revision is the recorded `proposal_revision_id`. It is in the same Proposal as the edited Revision. Its parent is the recorded `source_proposal_revision_id`. That parent is a Revision of the same Proposal.
- **Division of the proof.** The trigger proves only that the records agree and that they stay in one Proposal. The adapter proves the lineage. A recursive walk in a trigger function adds a second copy of the lineage rule and a cost to each Undo Receipt write. The adapter walk runs in the same Serializable transaction, and the head compare-and-set stops a concurrent change.
- **Proposal decisions.** The Undo of `replanProposal`, `reopenRejectedOperations`, and `reopenWithdrawnProposal` keeps the ADR 0044 rule for now. A Proposal head that moved after the decision makes it a Barrier. The lineage walk already goes through their Compensations as steps. [#1244](https://github.com/FrankQDWang/StoryOS/issues/1244) changes their load, parent, compare-and-set, and payload in the same way. When a Receipt trigger branch has the same limit for them, #1244 replaces that branch in the same way.

## Replaced rules

- ADR 0044 said: "For the three new compensations, a Proposal head that moved after the Forward action makes the Forward action a Barrier." For a candidate edit and an author withdrawal, the lineage rule of this ADR now applies. Until #1244, that sentence still applies to the three Proposal decisions.
- The Compensation of an author withdrawal appended its Revision to the withdrawn Revision. It now appends its Revision to the current head.

## Considered options

- **Record the writer on each Proposal Revision.** Add a column for the writer Compensation, as migration 0085 did. Rejected: the Undo Receipt payload already records the two ids. A new column also adds a backfill and an archive export change.
- **Compare the text.** Rejected: this is an inference from content, as ADR 0067 records.
- **Append the Compensation to the source Revision.** Rejected: the Proposal history gets a branch, and the head compare-and-set becomes complex.
- **Prove the lineage in the trigger.** Rejected for the reasons in "Division of the proof".

## Consequences

- The adapter tests in `command_sequence_tests/proposal_undo_chain_tests.rs` prove the chains, the Barriers, and the trigger branch. The HTTP tests in `undo-latest-author-action-http.integration.test.ts` follow the Editor Session frontier step by step.
- Two chains stay out of scope. One goes through the Compensation of an Acceptance. The other goes through the Compensation of a Refused Edit Draft expansion. Separate tickets own them.
