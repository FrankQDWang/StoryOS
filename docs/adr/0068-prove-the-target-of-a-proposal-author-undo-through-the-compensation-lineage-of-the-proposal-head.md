---
status: accepted
---

# Prove the Target of a Proposal Author Undo Through the Compensation Lineage of the Proposal Head

The ticket [Let Undo Go Back Many Steps Through Proposal Candidate Edits and Withdrawal](https://github.com/FrankQDWang/StoryOS/issues/1249) owns this ADR. The ticket [Let Undo Go Back Through a Proposal Decision](https://github.com/FrankQDWang/StoryOS/issues/1244) extends it to the three Proposal decisions. The ticket [Keep the Editor Writable When Undo Reaches a Refused Edit Expansion](https://github.com/FrankQDWang/StoryOS/issues/1251) extends it to the Undo of a Refused Edit Draft expansion.

## Context

The author makes candidate edit E1 and then candidate edit E2 on one Proposal. Author Undo compensates E2. The Compensation appends a new Proposal Revision to the Proposal head. E1 is now the Author Undo Frontier.

Five Compensations found their source only when the Proposal head was the Revision of the source action. They are the Compensations of a candidate edit, of an author withdrawal, and of the three Proposal decisions: `replanProposal`, `reopenRejectedOperations`, and `reopenWithdrawnProposal`. ADR 0044 recorded this rule: "a Proposal head that moved after the Forward action makes the Forward action a Barrier." Thus the Undo of E1 gave `barrier`.

On 2026-10-21 the author decided that all Author Undo of a Proposal action goes back many steps, as in an ordinary editor. This is the same rule as prose Undo ([ADR 0067](0067-prove-the-target-of-author-undo-through-the-compensation-lineage-of-the-chapter-head.md)). This decision covers candidate edits and author withdrawals ([#1249](https://github.com/FrankQDWang/StoryOS/issues/1249)) and the three Proposal decisions ([#1244](https://github.com/FrankQDWang/StoryOS/issues/1244)).

The Draft Compensation of a Refused Edit Draft expansion X had a different rule. It found X only when the Proposal head was the Revision of X with the validation `pending`. Also, the base of that Revision had to be the Chapter head. After X, a candidate edit E1, and the Undo of E1, the head is a Compensation Revision, and the validation is not `pending`. Thus the Undo of X gave `conflicted` with `source_binding_changed`, and the editor became read-only. The same occurred after a withdrawal and its Undo, and after a prose edit and its Undo ([#1251](https://github.com/FrankQDWang/StoryOS/issues/1251)).

## Decision

- **Proposal head lineage.** A Proposal Head Lineage Revision of a Proposal head is a Revision that the head restores. The current head is one. When a recorded Proposal Compensation appended one, the prior Revision of the compensated Forward action is also one. The walk continues until a Revision that no recorded Proposal Compensation explains.
- **Each step counts.** A target can be a Revision that a Compensation wrote, for example a head that the author withdrew after an Undo. Thus a target must be one of the Proposal Head Lineage Revisions, not only the last one.
- **Evidence.** The walk uses only existing records. The Undo Receipt payload of a Proposal Compensation records the appended Revision (`proposal_revision_id`) and its parent (`source_proposal_revision_id`). A step is valid only when the parent of the Revision is the recorded parent. The Compensation Author Action names the compensated Forward action. The walk does not compare text.
- **Prior Revision.** For a candidate edit, a Replan, and a Reopen, the prior Revision is the parent of the Revision that the Forward action wrote. For an author withdrawal, the prior Revision is the withdrawn Revision itself, because the withdrawal changes the closure in place.
- **Candidate edit target.** The Undo of a candidate edit needs its edited Revision to be a Proposal Head Lineage Revision of the current head. It restores the candidate of the edited Operation before the edit.
- **Decision target.** The Revision that a Proposal decision wrote must be a Proposal Head Lineage Revision of the current head. It appends a copy of the Revision before the decision. The Undo of a reopen of rejected Operations also needs each reopened Operation to be `pending` and `unresolved`. It puts each one back to `rejected` and `resolved`.
- **Withdrawal target.** The Undo of an author withdrawal compensates when two conditions are true. The current head is withdrawn. The withdrawn Revision is a Proposal Head Lineage Revision of the current head. The Compensation reopens the Proposal.
- **Compensation records.** The Compensation appends its Revision to the current head, as the Chapter rule of ADR 0067 does. Thus the Proposal history stays linear. The Undo Receipt payload records the appended Revision and the current head as its parent. The replay of an exact retry finds the appended Revision through these two ids.
- **Barrier.** These conditions still stop the chain with the reason `barrier`. No new reason code is added.
  - The Frontier has the Barrier disposition of ADR 0044, for example `rejectProposalOperations`, `completeReadyPartialProposal`, `continueProposalGeneration`, or `undoLatestAuthorAction` with `reversal_required`.
  - A writer that the records cannot explain moved the Proposal head. Examples are an append of Worker generation, the reopen Revision of an Acceptance Compensation, and an old Compensation with no Receipt payload.
  - The source is a candidate edit from before migration 0085 on a Block that is not the primary Block. Its prior candidate is not known.
  - The source is an author withdrawal, and the current head is not withdrawn.
  - The source is a reopen of rejected Operations, and a reopened Operation is not `pending` and `unresolved`.
- **Expansion target.** The Draft Compensation of a Refused Edit Draft expansion X finds X with this rule. The Revision of X must be a Proposal Head Lineage Revision of the current head. The validation of the head is not examined. The head must be `open`, and an Operation must be `pending` and `unresolved`. The base of the head must be the Head Lineage Revision of the Chapter head (ADR 0067). The Compensation withdraws the current head and reopens the source Draft.
- **Expansion results.** A failure of this Proposal evidence gives `barrier`, and the editor stays writable. A change on the Draft side gives the same result as before. A changed Draft Revision, digest, or closure gives `conflicted` with `source_binding_changed`. An unavailable Draft gives `source_unavailable`. Only a race can change the Draft side, so the Draft result comes first.
- **Inline rebind Revision.** A prose edit on a Chapter with an open inline Proposal appends a Revision to that Proposal. It keeps the candidate and the base. No Receipt and no Compensation records it, so it is not a Proposal Head Lineage Revision. Thus the Undo of X after a prose edit and its Undo gives `barrier`. A separate ticket owns this chain.
- **Editor.** The Web Client does not change. After each Undo, it reads the next Author Undo Frontier from the Editor Session. The Server accepts that Undo or gives its reason. An unavailable Undo keeps the editor writable (ADR 0067).
- **Storage.** No new column and no new table. Migration 0096 replaces one trigger function. The Receipt trigger of migration 0066 required that the parent of the appended Revision is the source Revision of the Forward action. Migration 0096 replaces only the Proposal Compensation branch of `storyos.require_draft_retry_settlement()`. All other branches stay as migration 0066 wrote them. That branch applies to each Forward action with the result `proposal_revised`, so the three Proposal decisions need no migration.
- **Expansion trigger branch.** The reopen trigger function of migration 0067, `storyos.require_draft_reopen_settlement()`, required that the withdrawn head was the Revision of X with the validation `pending`. Migration 0097 replaces only its expansion branch. The withdrawn head is the Revision of X, or a Revision that a recorded Compensation wrote. A Proposal Compensation records the appended Revision and its parent in its Undo Receipt payload. An Acceptance Compensation records its reopen Revision in its Undo Acceptance Receipt with the outcome `compensated`. The adapter proves the lineage, as "Division of the proof" tells.
- **Trigger branch.** The new branch requires four conditions. The appended Revision is the recorded `proposal_revision_id`. It is in the same Proposal as the Revision that the Forward action wrote. Its parent is the recorded `source_proposal_revision_id`. That parent is a Revision of the same Proposal.
- **Division of the proof.** The trigger proves only that the records agree and that they stay in one Proposal. The adapter proves the lineage. A recursive walk in a trigger function adds a second copy of the lineage rule and a cost to each Undo Receipt write. The adapter walk runs in the same Serializable transaction, and the head compare-and-set stops a concurrent change.
- **Anchor copy.** A Replan changes the base, so the Compensation of a Replan copies the Anchor set of the source Revision (ADR 0063). It copies the set when the base of the source Revision is not the base of the current head. The current head restores the Revision that the decision wrote, so it has the base of that Revision.

## Replaced rules

- ADR 0044 said: "For the three new compensations, a Proposal head that moved after the Forward action makes the Forward action a Barrier." The lineage rule of this ADR now applies to a candidate edit, an author withdrawal, and the three Proposal decisions.
- The Compensation of an author withdrawal appended its Revision to the withdrawn Revision. The Compensation of a Proposal decision appended its Revision to the Revision that the decision wrote. Each one now appends its Revision to the current head.

## Considered options

- **Record the writer on each Proposal Revision.** Add a column for the writer Compensation, as migration 0085 did. Rejected: the Undo Receipt payload already records the two ids. A new column also adds a backfill and an archive export change.
- **Compare the text.** Rejected: this is an inference from content, as ADR 0067 records.
- **Append the Compensation to the source Revision.** Rejected: the Proposal history gets a branch, and the head compare-and-set becomes complex.
- **Prove the lineage in the trigger.** Rejected for the reasons in "Division of the proof".

## Consequences

- The adapter tests in `command_sequence_tests/proposal_undo_chain_tests.rs` prove the chains, the Barriers, and the trigger branch. The adapter tests in `command_sequence_tests/proposal_decision_undo_chain_tests.rs` prove the chains through the three Proposal decisions. The HTTP tests in `undo-latest-author-action-http.integration.test.ts` follow the Editor Session frontier step by step.
- The adapter tests in `command_sequence_tests/draft_expansion_undo_chain_tests.rs` prove the expansion chains, the `barrier` and `conflicted` results, and the expansion trigger branch. The HTTP test in `edit-inline-proposal-http.integration.test.ts` follows the Editor Session frontier through an expansion.
- Structured candidates currently cannot be edited. Each expansion writes a structured candidate, so real commands cannot make the chain through a candidate edit. The adapter test removes the structured candidate with a fixture. Thus it proves the lineage rule and the trigger branch for a Compensation head. Real commands can make the chains through a withdrawal and through a prose edit.
- One chain stays out of scope. It goes through the Compensation of an Acceptance. A separate ticket owns it.
