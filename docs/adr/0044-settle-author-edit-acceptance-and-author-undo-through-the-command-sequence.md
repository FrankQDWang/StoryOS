---
status: accepted
---

# Settle Author Edit, Acceptance, and Author Undo Through the Command Sequence

On 2026-10-05 the author accepted this decision. It extends [ADR 0041](0041-own-the-project-command-sequence-in-one-adapter-module.md) and [ADR 0043](0043-settle-every-project-command-through-declared-profiles-of-one-sequence.md) to the three project commands that ADR 0043 kept out: `applyAuthorEdit`, `acceptProposal`, and `undoLatestAuthorAction`. It records the sequence capabilities that these commands need and the Author Undo Disposition of each Forward command kind. It also resolves [Decide the Author Undo Disposition of Replan and Reopen Proposal Actions](https://github.com/FrankQDWang/StoryOS/issues/933).

## Context

At `main` `5470896d`, the three commands use hand-written transactions:

- `applyAuthorEdit` uses two transactions. The first transaction consumes the Command Challenge, commits the Author Command Admission, and leaves the Command Idempotency Fence `in_progress`. The second transaction locks the target Head, classifies through Core, and writes the Domain Receipt. The Author Edit outcome query finds an Admission that has no settlement. It runs the second transaction again, or it settles the Admission as `RequiresReconfirmation`. The `outcome_unknown` observation store has no product caller. The query derives each unknown state from the existing rows.
- An applied Author Edit and an applied Acceptance write the same authority records: the payload, the Authoritative Revision and its members, the Revision Envelope, the guarded Head update, the Authoritative Commit, the Forward Author Action, one row in the first Project Activity table (`project_activity_events`), the guarded writer-base update, and the canonical Snapshot. The prose and Acceptance compensations of Author Undo write the same records with a Compensation disposition. No ADR 0043 profile writes these records.
- `applyAuthorEdit` has two applied result kinds, `authoritative_applied` and `proposal_revised`, and the zero-authority result kind `refused_to_draft`. `acceptProposal` has the zero-authority result kind `invalid`, and its zero-authority outcomes write effect rows but no Activity record. `undoLatestAuthorAction` has seven compensation shapes, and its `reversal_required` outcome writes a Forward Author Action.
- `acceptProposal` writes a Pre-Admission Refusal Record ([ADR 0013](0013-trust-the-storyos-web-client-for-author-command-admission.md)). The command transaction rolls back. Then a second Serializable transaction proves the refusal facts again under the Challenge and fence locks and inserts the record.
- Author Undo selects its compensation from a pattern of null columns and from the Receipt result kind. Thus the Forward actions of `replanProposal`, `reopenRejectedOperations`, and `reopenWithdrawnProposal` go to the Proposal edit compensation. That compensation appends a Proposal Revision with the parent candidate text only. It does not put a reopened operation back to `rejected`, and it does not withdraw a reopened Proposal again.

## Decision

### The `AuthoritativeRevision` settlement profile

- The sequence adds one settlement profile, `AuthoritativeRevision`. It allocates the Author Action, Authoritative Commit, and Project Activity sequences together. It writes the payload, the Authoritative Revision and its members, the Revision Envelope, the guarded Head update, the Authoritative Commit, the Author Action, the `project_activity_events` row, the writer-base update that the prior Revision guards, and the canonical Snapshot. Its replay decodes these records.
- The applied Author Edit, the applied Acceptance, and the prose and Acceptance compensations of Author Undo use this profile. Each command supplies only the payload and the source of the members.
- The rows and tables do not change. The profile continues to write the first Project Activity table.

### Applied variants and Receipt result kinds

- A command declares a closed set of applied variants. Each variant fixes its settlement profile, its Receipt result kind, and its Author Action disposition: `Forward`, or `Compensation` with the sequence of the Forward action that it settles. ADR 0041 said that a disposition field arrives with the first Compensation command. This is that field.
- Core gives the Receipt result kind of each zero-authority outcome through its reason type, for example `refused_to_draft`, `invalid`, or `unavailable`.
- A zero-authority outcome can declare effect rows without an Activity record. `acceptProposal` uses this for its validation condition rows.
- The command supplies the revision, condition, Proposal, draft, and lifecycle reference arrays of the Domain Receipt. A command can also write one child Receipt row, for example `acceptance_receipts` or `undo_acceptance_receipts`.

### Author Edit: admit first, then settle the admitted command

- The sequence has two steps that run in separate transactions:
  1. **Admit.** This is the admit-only first use that ADR 0043 gives the exports. It consumes the Command Challenge, commits the Admission, and leaves the fence `in_progress`.
  2. **Settle the admitted command.** It reads the Project row without a lock and locks the command facts. It checks again that the Admission is not expired and that its writer generation is current. It classifies through Core, writes the Domain Receipt and the profile records, settles the Admission, and settles the fence.
- `applyAuthorEdit` runs step 1 and then step 2 in one request. The outcome query runs step 2 for an Admission that has no settlement, or settles it as `RequiresReconfirmation` when the recovery rules require that. The recovery rules of the [Author Command Admission contract](../foundation/author-command-admission.md#5-first-invocation-and-recovery-rules) do not change.
- An Admission that is committed and has no terminal settlement is `pending`. When the outcome query cannot yet settle a `pending` Admission, it reports `outcome_unknown`. The Admission row makes this condition durable. The sequence writes no `outcome_unknown` row.
- The transaction that pauses Proposal generation before an Author Edit stays before step 1. It is not part of the sequence.
- The `outcome_unknown` observation store and its tests are removed. The table `author_command_admission_outcome_unknown_observations` stays, with no writer. Removing the table needs a separate decision with a migration.

### Acceptance refusal before Admission

- The command adapter supplies a diagnosis for an Admission insert that inserts no row. The diagnosis gives a typed reason: `stale_writer`, `session_changed`, or `invalid_challenge`. For this command, the diagnosis replaces the fixed `MissingAdmission` error.
- A retained refusal for the same idempotency key is a refusal before Admission in the ADR 0043 sense. The fact load finds it, and the transaction rolls back.
- The `acceptProposal` method calls the sequence. When the sequence returns an invalid Challenge or a typed missing Admission, the transaction is already rolled back. Then the method retains the Pre-Admission Refusal Record in its own Serializable transaction, as ADR 0013 specifies. The sequence does not own this transaction, because only Acceptance writes such a record.

### Compensation adapters

- Each Forward command family has one compensation adapter in the module of its forward command. A compensation adapter loads the Forward evidence under lock and gives the Author Undo Frontier kind. It returns the writes of an applied variant with a `Compensation` disposition. It also decodes its effect on replay.
- The Author Undo command adapter keeps the Author Undo Frontier query and the Core classification. It selects the compensation adapter from the Forward command kind and its applied variant through one exhaustive match over a closed enum. A new Forward command kind does not compile until it declares its Author Undo Disposition.
- Replay selects the decoder from the command kind of the source Receipt, not from the shape of the stored rows.

### Author Undo Disposition of each Forward command kind

| Forward command kind and applied variant | Author Undo Disposition |
| --- | --- |
| `applyAuthorEdit`, `authoritative_applied` | Compensation. It restores the prior Revision. It reopens a Refused Edit Draft that the edit superseded. |
| `applyAuthorEdit`, `proposal_revised` | Compensation. It appends a Proposal Revision with the parent candidate text. |
| `acceptProposal` | Compensation when the Head is a Safe Compensation Head. Otherwise the Undo Acceptance outcome is reversal required or unavailable. |
| `createVolume`, `updateVolume`, `deleteVolume`, `createChapter`, `updateChapter`, `deleteChapter` | Compensation ([ADR 0030](0030-restore-prior-identities-for-structure-compensation.md)). |
| `setCurrentChapter` | Compensation when the prior Chapter is a lawful target. Otherwise Barrier. |
| `withdrawProposal`, author form | Compensation. It reopens the Proposal. |
| `closeEditorFlowDraft`, `expandRefusedEditDraftToProposal` | Compensation. It reopens the Draft. |
| `replanProposal` | Compensation. It appends a Proposal Revision equal to the Revision before the replan: base, candidate text, generation state, closure, and validation. |
| `reopenRejectedOperations` | Compensation. It appends a Proposal Revision equal to the Revision before the reopen, and it puts each reopened operation back to `rejected` and `resolved`. |
| `reopenWithdrawnProposal` | Compensation. It appends a Proposal Revision equal to the Revision before the reopen, and it withdraws the Proposal again. |
| `rejectProposalOperations` | Barrier. |
| `completeReadyPartialProposal`, `continueProposalGeneration` | Barrier. |
| `undoLatestAuthorAction`, `reversal_required` | Barrier. |

- Each existing compensation keeps the binding checks and the outcomes of its family. A moved binding gives the same Barrier, conflict, unavailable, or reversal-required outcome as on `main`. For the three new compensations, a Proposal head that moved after the Forward action makes the Forward action a Barrier.
- The compensations of `replanProposal`, `reopenRejectedOperations`, and `reopenWithdrawnProposal` are the only behavior changes of Author Undo in this decision. The `proposal_replans` and `proposal_operation_reopenings` rows stay, because they are history.
- On 2026-10-07 the author decided two details of these three compensations. The appended Proposal Revision also gets a copy of the validation receipt of the earlier revision. An Acceptance conflict condition stays on the earlier revision, because its identity and its Acceptance Receipt bind it to that revision. Thus after an Undo of a Replan, the Proposal shows the validation of the earlier revision without the conflict. A new Acceptance finds the conflict again.
- After an Undo of a reopen, the author cannot reopen the same withdrawal or rejection again, because the reopen records stay. [Decide if Author Undo of a Proposal Reopen Lets the Author Reopen Again](https://github.com/FrankQDWang/StoryOS/issues/1051) owns this question.
- A Barrier stops Author Undo, and Author Undo never skips it. Thus a Barrier frontier also stops Author Undo of all earlier Forward actions. A later decision can change a Barrier to a Compensation.

### Project setting commands stay outside Author Undo Order

- On 2026-10-09 the author decided this. These four commands are not author-owned Core Transitions: `createProject`, `updateProject`, `archiveProject`, and `updateProjectAssistance`. They write no Author Action, and Author Undo Order does not include them. They are not Forward command kinds, so they declare no Author Undo Disposition.
- Thus Author Undo after a Project rename compensates the latest earlier Forward action, for example a prose edit. The Project title does not change. Author Undo reverses the manuscript and Proposal work of the editor. A Project setting is not part of that work. Similarly, a document title is not in the undo history of a text editor.
- The reference model study reported this behavior as a defect ([issue 1027](https://github.com/FrankQDWang/StoryOS/issues/1027)). This decision makes it the contract.

## Relation to other decisions

- ADR 0041 and ADR 0043 stay in force. ADR 0043 says that Author Edit, `acceptProposal`, and Author Undo need sequence capabilities that a later decision records. This decision records them, and these three commands now settle through the sequence. After this decision, every implemented project command settles through the sequence, except the three operations that ADR 0043 excludes.
- ADR 0043 says that every command inserts its Admission after classification. `applyAuthorEdit` is an exception: it inserts its Admission in the admit step, before classification. `acceptProposal` and `undoLatestAuthorAction` insert their Admission after classification.
- The [Author Command Admission contract](../foundation/author-command-admission.md#4-lifecycle-and-terminal-settlement) now states that the outcome query reports `outcome_unknown` and that no row records it. The [PostgreSQL storage contract](../foundation/postgresql-project-storage-isolation-and-migration-contract.md) states that the observation table has no writer. The lifecycle and the recovery rules do not change.
- ADR 0013 stays in force. The refusal record, its transaction, and its fields do not change.
- [ADR 0029](0029-own-structural-authority-settlement-beside-author-edit.md) and ADR 0030 stay in force. A compensation adapter is next to its forward command, and the inverse public command is not invoked.
- [ADR 0038](0038-separate-author-edit-challenge-admission-from-shared-command-admission.md) stays in force. `applyAuthorEdit` and `undoLatestAuthorAction` stay in the `author_edit` Challenge Rate Class.

## Behavior-equivalence record

Each ticket that moves a command records here every observable difference from `main`. An unlisted difference is a defect.

### Sequence capabilities

- The `AuthoritativeRevision` profile writes the records of the Decision section after the Domain Receipt. The command supplies the payload, the source of the members, the prior Revision, and the Editor Session of the writer base.
- A command declares its applied variant. `AppliedVariant::Forward` names the `ForwardCommand` entry of the command, which gives the Receipt result kind and the Author Undo Disposition. A command that writes no Author Action declares `AppliedVariant::NoAuthorAction`. The sequence does not compile when a Forward variant names the entry of another command kind. The commands of Specifications A, B, and C declare their variant with the result kind that they recorded before, so their rows do not change.
- A reason type can give the Receipt result kind of its outcome. `AcceptProposalInvalid` gives `invalid`.
- A reason value can give its own Receipt result kind, and it can record no reason code. `AuthorEditRefused::RefusedToDraft` gives `refused_to_draft` and records no reason, as on `main`. The other values of `AuthorEditRefused` give `refused` with their reason.
- A command can write one child Receipt row after the Domain Receipt of each outcome, and the Domain Receipt can record condition references.
- A command can declare `MissingAdmission::Diagnosed`. Then its diagnosis gives the error of an Admission insert that inserts no row.
- A command can declare more than one applied variant. The command selects the variant of each applied outcome. `AppliedVariant::Compensation` gives the Receipt result kind of a Compensation. The compensation adapter gives the compensated source sequence.
- A settlement profile can take a selector from the plan of the command. The selector chooses the sequences that the profile allocates. A profile with one write path takes no selector.
- A command can record the time of the Domain Receipt as the transaction time. Author Undo does this when it reopens a Draft, as on `main`. All other Receipts record the clock time, as before.
- The sequence has the admit step and the settle step of a direct editor action (`command_sequence/settle.rs`). The admit step reads the Project row without a lock, inserts the `direct_editor_action` Admission, and commits.
  - The settle step runs in a new transaction. It reads the Project row without a lock. It requires an Admission without settlement that is not expired and whose Editor Session is the current writer. The command locks its own facts when it classifies. Then the step writes the Receipt, the profile records, the Admission settlement, and the fence.
  - A settled Admission replays its Receipt. After a failed settle step, a read-only transaction reads the settlement again, and a settlement of a concurrent settle step replays.
- The `AuthoritativeRevision` profile can write the members of a versioned edit from its Blocks (`RevisionMembers::Blocks`).
- A zero-authority Receipt keeps the observed payload fields also when its reason value records no code. Before, the sequence wrote `{}` for such a value. No earlier command had such a value.

### `acceptProposal`

- `acceptProposal` uses the explicit editor command Admission form, the `AuthoritativeRevision` profile with a Forward Author Action, the Command-response Project, and the `acceptance_receipts` child row. The `invalid` and `conflicted` outcomes write their validation condition row, and they write no Activity record. The `refused` outcome writes no effect row.
- The route uses the generic project command admission with the problem order of `main`. The problem texts use the name "Acceptance", as on `main`.
- The Pre-Admission Refusal Record, its Serializable transaction, its fields, and its problems `422 challenge_invalid` and `409 acceptance_session_ineligible` do not change.
- The application binding self-check, the Store trait, the command type, and the error type of the command are removed.
- These observable changes follow from the sequence:
  - The command locks the Project row first. It inserts its Admission after the Core classification.
  - When the Admission insert fails and the composition of the accepted text also fails, the command gives `503 project_store_unavailable` and retains no refusal record. Before, it retained the refusal record and gave the refusal problem. The composition fails only for an inline Proposal without its exact Anchors or its canonical Block.
  - A missing settled record, an unknown result kind, or an unknown reason gives `409 idempotency_binding_conflict`. Before, it gave `409 historical_acknowledgement_unavailable`.
  - An applied Receipt without its Authoritative Commit, its Revision, its payload, its Author Action, or its `project_activity_events` record gives `503 project_store_unavailable`. An applied Receipt whose Author Action is not Forward gives the same store fault. Before, a missing Commit or Revision stopped the Server process with a panic ([issue 930](https://github.com/FrankQDWang/StoryOS/issues/930)), and replay ignored the disposition. Foreign keys and the receipt relation trigger already prevent such a record.
  - A zero-authority Receipt with an Author Action gives a store fault. Before, replay ignored the Author Action. The receipt relation trigger already prevents such a record.
  - A Receipt `reason` field with the wrong JSON type gives a store fault. Before, replay read the value as text.
- The behavior-equivalence review against `main` found no other difference.

### `undoLatestAuthorAction`

- `undoLatestAuthorAction` uses the explicit editor command Admission form, the `UndoCompensation` profile, and the Command-response Project. The profile sends the write and the replay to the compensation adapter of the Forward family.
- The applied variants are `Compensation` with `authoritative_applied`, `Compensation` with `draft_closure_changed`, and `Forward` with `reversal_required`. The Receipt of a Reversal records the result kind `authoritative_applied`, as on `main`.
- The `conflicted` and `refused` outcomes write only the Domain Receipt. When the frontier is an Acceptance, the `refused` outcome also writes the `undo_acceptance_receipts` child row with the outcome `unavailable`, as on `main`.
- An Acceptance `conflicted` outcome writes no child row, as on `main`. The ticket text of [Settle undoLatestAuthorAction Through the Command Sequence](https://github.com/FrankQDWang/StoryOS/issues/965) says that a conflict writes the child row. But the `outcome` check of `undo_acceptance_receipts` has no conflict value, and the ticket also requires the rows of `main` without a migration. Thus the rows of `main` apply.
- Each compensation adapter writes the same rows as on `main`. A Structure or Current Chapter Compensation writes a canonical Snapshot and no `project_activity_events` record, as on `main`.
- Core gives the outcome as a `TransitionOutcome`. The reason code texts do not change.
- The route uses the generic project command admission with the problem order of `main`.
- The application Store trait, the command type, and the error type of the command are removed.
- These observable changes follow from the sequence:
  - The command locks the Project row before it uses the Command Challenge. It inserts its Admission after the Core classification, as on `main`.
  - An applied Receipt whose Compensation records are missing or damaged gives `503 project_store_unavailable`. Examples are a missing Authoritative Commit, canonical Snapshot, Draft reopen event, or Receipt payload field, and an Author Action that is not a Compensation. Before, most of these gave `409 idempotency_binding_conflict`. Foreign keys and the receipt relation triggers already prevent such records.
  - An exact retry of the Undo of an author Withdrawal returns the Project Activity position of the first acknowledgement. The Receipt payload records this position. Before, the retry returned the position zero ([issue 1031](https://github.com/FrankQDWang/StoryOS/issues/1031)).
- The behavior-equivalence review against `main` found no other difference.

### `applyAuthorEdit`

- `applyAuthorEdit` runs the transaction that pauses Proposal generation, then the admit step, and then the settle step. Its profile `AuthorEditProfile` selects the `AuthoritativeRevision` records for `authoritative_applied` and the `ActionOnly` records for `proposal_revised`. An applied edit of an Inline Proposal also appends its Proposal Revision. Each outcome that supersedes the source Refused Edit Draft writes its close event after the Receipt.
- The zero-authority outcomes write the rows and payloads of `main`. `refused_to_draft` writes the Refused Edit Draft and records no reason. The response record is `NoResponse`, as on `main`.
- The outcome query keeps its code. It completes an open Admission through the settle step, and it replays a settled Admission through the sequence.
- The hand-written settlement transaction and replay query (`author_edit_settlement.rs`, `author_edit_replay.rs`), the application Store trait, and the binding self-check are removed. The Server builds the command and calls the adapter.
- These observable changes follow from the sequence:
  - The admit step and the settle step read the Project row without a lock, as on `main`. When the admit step locked the Project row, the mixed edit of the `production-host` journey waited and did not complete. The settle step locks only the Chapter head with the Admission and its current writer, as on `main`. Thus a writer takeover can still settle while an admitted edit waits for that head.
  - A replay keeps the binding conflicts of `main` for the request facts. These facts are the canonical payload, the digest, the Chapter, the expected Revision, the target references, and the expected head of the Receipt. A missing or unknown reason also gives a binding conflict.
  - Damaged evidence gives `503 author_edit_store_unavailable`. Before, it gave `409 idempotency_binding_conflict`. Examples are a missing Activity record, Author Action, Commit, or Draft lifecycle event, and damaged head arrays. Other examples are a zero-authority Receipt with authority records, a Commit or Envelope of another prior Revision, and an impossible conflict. Foreign keys, CHECK constraints, and the receipt relation triggers already prevent such records.
- The behavior-equivalence review against `main` found no other difference.

## Considered options

- Each of the three commands writes the Authoritative Revision records as its own effect rows. This was rejected. It keeps three copies of the same write path.
- One command adapter for each applied shape, selected after classification, was rejected. It splits one Core classification and one replay.
- One transaction for Author Edit, with the Admission and the Core Transition together, was rejected for this decision. It removes the Admission recovery, but it changes the Author Command Admission lifecycle, the Web Client states, and the premise of [Deliver Host-created Recovery Drafts for admitted author edits](https://github.com/FrankQDWang/StoryOS/issues/881). This is a product contract change, not a migration.
- A refusal record profile in the sequence was rejected. Only Acceptance writes a Pre-Admission Refusal Record, and its transaction runs after the command transaction rolls back.
- One Author Undo adapter with one internal match over the compensation families was rejected. The seven families change independently, and the selection by Receipt result kind caused the defect of issue 933.
- A Barrier for `replanProposal`, `reopenRejectedOperations`, and `reopenWithdrawnProposal` was rejected. One Proposal decision would then stop Author Undo of all earlier prose edits.
- A Barrier for `updateProject` and `archiveProject` was rejected. With it, one rename stops Author Undo of all earlier prose edits.
- A Compensation that restores the prior Project title was rejected. With it, Author Undo in the editor changes the Project title.

## Consequences

- Rows, migrations, persisted formats, isolation levels, reason code texts, HTTP statuses, and problem codes do not change, except for the three compensations above. Each specification that moves these commands lists every other observable difference in its behavior-equivalence review.
- The hand-written transactions, replay queries, Store traits, and binding self-checks of the three commands are removed. Then the shared code that no command uses is removed, and `AGENTS.md` states that a new project command must settle through the sequence.
- A new Forward command kind needs a compensation adapter or a Barrier entry, and its Author Undo Disposition must be recorded.
- Making `rejectProposalOperations` compensable is a separate decision.
